//! Safe, read-only analysis of a `.wgt` (Tizen web widget) archive.
//!
//! Nothing from the archive is ever extracted to disk or executed: we only read the central
//! directory plus `config.xml` and the signature XML files into memory, with hard size caps.
//! The user's original file is opened read-only.

use serde::Serialize;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use thiserror::Error;

/// Hard limits (archive-bomb / DoS protection).
pub const MAX_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_ENTRIES: usize = 20_000;
pub const MAX_TOTAL_UNCOMPRESSED: u64 = 2 * 1024 * 1024 * 1024;
pub const MAX_COMPRESSION_RATIO: u64 = 1_000;
pub const MAX_XML_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Error)]
pub enum PackageError {
    #[error("file not found or unreadable")]
    Unreadable,
    #[error("not a file")]
    NotAFile,
    #[error("unsupported file type (expected .wgt)")]
    UnsupportedExtension,
    #[error("file is empty")]
    Empty,
    #[error("file is too large")]
    TooLarge,
    #[error("not a valid ZIP archive")]
    NotAZip,
    #[error("archive looks malicious or unreasonable: {0}")]
    Unsafe(String),
    #[error("config.xml is missing")]
    MissingConfig,
    #[error("config.xml is not valid: {0}")]
    BadConfig(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SignatureState {
    /// No signature files at all.
    Unsigned,
    /// Author and distributor signature files are both present and look structurally valid.
    /// Cryptographic validity / matching the TV's DUID is NOT verified here.
    SignedStructure,
    /// Only some of the expected signature files exist (e.g. author-signature.xml without signature1.xml).
    Partial,
    /// Signature files exist but are not well-formed XML signatures.
    Malformed,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Finding {
    /// Stable machine key, translated in the UI.
    pub code: &'static str,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PackageReport {
    pub file_name: String,
    pub size_bytes: u64,
    pub entry_count: usize,
    pub uncompressed_bytes: u64,
    /// `<name>` from config.xml, if present.
    pub app_name: Option<String>,
    /// `tizen:application/@id`, e.g. `abcde12345.MyApp`.
    pub app_id: Option<String>,
    /// `tizen:application/@package` (10-char package id).
    pub package_id: Option<String>,
    /// `widget/@version`.
    pub version: Option<String>,
    pub required_api_version: Option<String>,
    pub signature: SignatureState,
    pub has_author_signature: bool,
    pub has_distributor_signature: bool,
    pub has_index_html: bool,
    pub warnings: Vec<Finding>,
}

fn is_unsafe_name(name: &str) -> Option<&'static str> {
    if name.contains('\0') {
        return Some("nul_in_name");
    }
    if name.starts_with('/') || name.starts_with('\\') {
        return Some("absolute_path");
    }
    let b = name.as_bytes();
    if b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic() {
        return Some("drive_letter_path");
    }
    if name.split(['/', '\\']).any(|c| c == "..") {
        return Some("path_traversal");
    }
    None
}

fn read_capped<R: Read>(r: R, cap: u64) -> Result<Vec<u8>, PackageError> {
    let mut buf = Vec::new();
    r.take(cap + 1)
        .read_to_end(&mut buf)
        .map_err(|_| PackageError::NotAZip)?;
    if buf.len() as u64 > cap {
        return Err(PackageError::Unsafe("xml_too_large".into()));
    }
    Ok(buf)
}

pub fn analyze_wgt(path: &Path) -> Result<PackageReport, PackageError> {
    let meta = std::fs::metadata(path).map_err(|_| PackageError::Unreadable)?;
    if !meta.is_file() {
        return Err(PackageError::NotAFile);
    }
    let ext_ok = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("wgt"))
        .unwrap_or(false);
    if !ext_ok {
        return Err(PackageError::UnsupportedExtension);
    }
    let size = meta.len();
    if size == 0 {
        return Err(PackageError::Empty);
    }
    if size > MAX_ARCHIVE_BYTES {
        return Err(PackageError::TooLarge);
    }

    let file = File::open(path).map_err(|_| PackageError::Unreadable)?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| PackageError::NotAZip)?;
    if zip.len() > MAX_ENTRIES {
        return Err(PackageError::Unsafe("too_many_entries".into()));
    }

    let mut total: u64 = 0;
    let mut has_config = false;
    let mut has_index = false;
    let mut has_author = false;
    let mut has_dist = false;
    let mut warnings: Vec<Finding> = Vec::new();

    for i in 0..zip.len() {
        let entry = zip.by_index_raw(i).map_err(|_| PackageError::NotAZip)?;
        let name = entry.name().to_string();
        if let Some(why) = is_unsafe_name(&name) {
            return Err(PackageError::Unsafe(format!("{why}: {name}")));
        }
        let usize_ = entry.size();
        let csize = entry.compressed_size();
        total = total.saturating_add(usize_);
        if total > MAX_TOTAL_UNCOMPRESSED {
            return Err(PackageError::Unsafe("uncompressed_size_limit".into()));
        }
        if csize > 0 && usize_ / csize > MAX_COMPRESSION_RATIO && usize_ > 10 * 1024 * 1024 {
            return Err(PackageError::Unsafe("suspicious_compression_ratio".into()));
        }
        match name.as_str() {
            "config.xml" => has_config = true,
            "index.html" => has_index = true,
            "author-signature.xml" => has_author = true,
            "signature1.xml" => has_dist = true,
            _ => {}
        }
    }
    if !has_config {
        return Err(PackageError::MissingConfig);
    }
    if !has_index {
        warnings.push(Finding {
            code: "no_index_html",
            detail: None,
        });
    }

    // config.xml
    let cfg_bytes = {
        let f = zip
            .by_name("config.xml")
            .map_err(|_| PackageError::MissingConfig)?;
        read_capped(f, MAX_XML_BYTES)?
    };
    let cfg_text =
        String::from_utf8(cfg_bytes).map_err(|_| PackageError::BadConfig("not UTF-8".into()))?;
    // DTDs are rejected by roxmltree by default (no entity expansion attacks).
    let doc = roxmltree::Document::parse(&cfg_text)
        .map_err(|e| PackageError::BadConfig(e.to_string()))?;
    let root = doc.root_element();
    if root.tag_name().name() != "widget" {
        return Err(PackageError::BadConfig(
            "root element is not <widget>".into(),
        ));
    }
    let version = root
        .attribute("version")
        .map(str::to_string)
        .filter(|s| !s.is_empty());
    let mut app_name = None;
    let mut app_id = None;
    let mut package_id = None;
    let mut required = None;
    for n in root.children().filter(|n| n.is_element()) {
        match n.tag_name().name() {
            "name" if app_name.is_none() => {
                let t: String = n
                    .descendants()
                    .filter(|d| d.is_text())
                    .filter_map(|d| d.text())
                    .collect();
                let t = t.trim();
                if !t.is_empty() {
                    app_name = Some(t.chars().take(200).collect());
                }
            }
            "application" => {
                app_id = n.attribute("id").map(str::to_string);
                package_id = n.attribute("package").map(str::to_string);
                required = n.attribute("required_version").map(str::to_string);
            }
            _ => {}
        }
    }
    if app_id.is_none() {
        warnings.push(Finding {
            code: "no_tizen_application",
            detail: None,
        });
    }
    if let (Some(id), Some(pkg)) = (&app_id, &package_id) {
        if !id.starts_with(&format!("{pkg}.")) {
            warnings.push(Finding {
                code: "app_id_package_mismatch",
                detail: Some(format!("{id} / {pkg}")),
            });
        }
    }

    // Signature structure
    let mut malformed = false;
    for (present, name) in [
        (has_author, "author-signature.xml"),
        (has_dist, "signature1.xml"),
    ] {
        if !present {
            continue;
        }
        let ok = match zip
            .by_name(name)
            .ok()
            .and_then(|f| read_capped(f, MAX_XML_BYTES).ok())
        {
            Some(bytes) => std::str::from_utf8(&bytes)
                .ok()
                .and_then(|t| roxmltree::Document::parse(t).ok())
                .map(|d| d.root_element().tag_name().name() == "Signature")
                .unwrap_or(false),
            None => false,
        };
        if !ok {
            malformed = true;
        }
    }
    let signature = if malformed {
        SignatureState::Malformed
    } else if has_author && has_dist {
        SignatureState::SignedStructure
    } else if has_author || has_dist {
        SignatureState::Partial
    } else {
        SignatureState::Unsigned
    };

    Ok(PackageReport {
        file_name: path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
        size_bytes: size,
        entry_count: zip.len(),
        uncompressed_bytes: total,
        app_name,
        app_id,
        package_id,
        version,
        required_api_version: required,
        signature,
        has_author_signature: has_author,
        has_distributor_signature: has_dist,
        has_index_html: has_index,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    const CONFIG: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<widget xmlns="http://www.w3.org/ns/widgets" xmlns:tizen="http://tizen.org/ns/widgets" id="http://example.org/Demo" version="1.2.3" viewmodes="maximized">
  <tizen:application id="abcde12345.Demo" package="abcde12345" required_version="6.0"/>
  <name>Demo App</name>
</widget>"#;
    const SIG: &str =
        r#"<Signature xmlns="http://www.w3.org/2000/09/xmldsig#"><SignedInfo/></Signature>"#;

    fn make(entries: &[(&str, &[u8])]) -> tempfile::NamedTempFile {
        let f = tempfile::Builder::new().suffix(".wgt").tempfile().unwrap();
        let mut w = zip::ZipWriter::new(f.reopen().unwrap());
        for (n, d) in entries {
            w.start_file(*n, SimpleFileOptions::default()).unwrap();
            w.write_all(d).unwrap();
        }
        w.finish().unwrap();
        f
    }

    #[test]
    fn reads_metadata_unsigned() {
        let f = make(&[
            ("config.xml", CONFIG.as_bytes()),
            ("index.html", b"<html/>"),
        ]);
        let r = analyze_wgt(f.path()).unwrap();
        assert_eq!(r.app_name.as_deref(), Some("Demo App"));
        assert_eq!(r.package_id.as_deref(), Some("abcde12345"));
        assert_eq!(r.app_id.as_deref(), Some("abcde12345.Demo"));
        assert_eq!(r.version.as_deref(), Some("1.2.3"));
        assert_eq!(r.required_api_version.as_deref(), Some("6.0"));
        assert_eq!(r.signature, SignatureState::Unsigned);
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn signature_states() {
        let both = make(&[
            ("config.xml", CONFIG.as_bytes()),
            ("index.html", b"x"),
            ("author-signature.xml", SIG.as_bytes()),
            ("signature1.xml", SIG.as_bytes()),
        ]);
        assert_eq!(
            analyze_wgt(both.path()).unwrap().signature,
            SignatureState::SignedStructure
        );
        let part = make(&[
            ("config.xml", CONFIG.as_bytes()),
            ("index.html", b"x"),
            ("author-signature.xml", SIG.as_bytes()),
        ]);
        assert_eq!(
            analyze_wgt(part.path()).unwrap().signature,
            SignatureState::Partial
        );
        let bad = make(&[
            ("config.xml", CONFIG.as_bytes()),
            ("index.html", b"x"),
            ("author-signature.xml", b"garbage"),
            ("signature1.xml", SIG.as_bytes()),
        ]);
        assert_eq!(
            analyze_wgt(bad.path()).unwrap().signature,
            SignatureState::Malformed
        );
    }

    #[test]
    fn rejects_zip_slip_names() {
        for evil in [
            "../evil.txt",
            "a/../../b",
            "/etc/passwd",
            "C:\\x",
            "a\\..\\b",
        ] {
            let f = make(&[("config.xml", CONFIG.as_bytes()), (evil, b"x")]);
            assert!(
                matches!(analyze_wgt(f.path()), Err(PackageError::Unsafe(_))),
                "{evil}"
            );
        }
    }

    #[test]
    fn missing_or_broken_config() {
        let f = make(&[("index.html", b"x")]);
        assert!(matches!(
            analyze_wgt(f.path()),
            Err(PackageError::MissingConfig)
        ));
        let f = make(&[("config.xml", b"<widget><name>"), ("index.html", b"x")]);
        assert!(matches!(
            analyze_wgt(f.path()),
            Err(PackageError::BadConfig(_))
        ));
        let f = make(&[("config.xml", b"<html/>"), ("index.html", b"x")]);
        assert!(matches!(
            analyze_wgt(f.path()),
            Err(PackageError::BadConfig(_))
        ));
    }

    #[test]
    fn rejects_dtd_entity_bombs() {
        let bomb = r#"<?xml version="1.0"?><!DOCTYPE widget [<!ENTITY a "aaaa"><!ENTITY b "&a;&a;&a;&a;">]><widget version="1">&b;</widget>"#;
        let f = make(&[("config.xml", bomb.as_bytes()), ("index.html", b"x")]);
        assert!(matches!(
            analyze_wgt(f.path()),
            Err(PackageError::BadConfig(_))
        ));
    }

    #[test]
    fn rejects_non_zip_and_wrong_ext() {
        let f = tempfile::Builder::new().suffix(".wgt").tempfile().unwrap();
        std::fs::write(f.path(), b"not a zip at all").unwrap();
        assert!(matches!(analyze_wgt(f.path()), Err(PackageError::NotAZip)));
        let g = tempfile::Builder::new().suffix(".zip").tempfile().unwrap();
        std::fs::write(g.path(), b"x").unwrap();
        assert!(matches!(
            analyze_wgt(g.path()),
            Err(PackageError::UnsupportedExtension)
        ));
        let e = tempfile::Builder::new().suffix(".wgt").tempfile().unwrap();
        assert!(matches!(analyze_wgt(e.path()), Err(PackageError::Empty)));
    }

    #[test]
    fn oversized_config_is_refused() {
        let big = vec![b' '; (MAX_XML_BYTES + 10) as usize];
        let f = make(&[("config.xml", &big), ("index.html", b"x")]);
        assert!(matches!(
            analyze_wgt(f.path()),
            Err(PackageError::Unsafe(_))
        ));
    }

    #[test]
    fn original_file_is_not_modified() {
        let f = make(&[("config.xml", CONFIG.as_bytes()), ("index.html", b"x")]);
        let before = std::fs::read(f.path()).unwrap();
        analyze_wgt(f.path()).unwrap();
        assert_eq!(before, std::fs::read(f.path()).unwrap());
    }
}
