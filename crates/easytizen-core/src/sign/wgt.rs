//! Produces a signed copy of a `.wgt`. The user's original file is only ever read.

use super::identity::SigningIdentity;
use crate::cancel::CancelToken;
use crate::package::{analyze_wgt, PackageError, MAX_TOTAL_UNCOMPRESSED};
use base64::Engine;
use rsa::pkcs1v15::SigningKey;
use rsa::sha2::{Digest, Sha512};
use rsa::signature::{SignatureEncoding, Signer};
use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use thiserror::Error;
use zip::write::SimpleFileOptions;

pub const AUTHOR_SIGNATURE: &str = "author-signature.xml";
pub const DISTRIBUTOR_SIGNATURE: &str = "signature1.xml";

#[derive(Debug, Error)]
pub enum SignError {
    #[error("the package is not valid: {0}")]
    Package(#[from] PackageError),
    #[error("the package contains two files with the same name")]
    DuplicateEntry,
    #[error("could not read the package")]
    Read,
    #[error("could not write the signed copy")]
    Write,
    #[error("signing failed")]
    Crypto,
    #[error("cancelled")]
    Cancelled,
}

#[derive(Clone, Copy)]
enum Role {
    Author,
    Distributor,
}

impl Role {
    fn id(self) -> &'static str {
        match self {
            Role::Author => "AuthorSignature",
            Role::Distributor => "DistributorSignature",
        }
    }
    fn uri(self) -> &'static str {
        match self {
            Role::Author => "http://www.w3.org/ns/widgets-digsig#role-author",
            Role::Distributor => "http://www.w3.org/ns/widgets-digsig#role-distributor",
        }
    }
}

const DSIG_NS: &str = "http://www.w3.org/2000/09/xmldsig#";

fn b64(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

/// Base64 wrapped at 76 columns, as the Tizen signer writes it.
fn b64_wrapped(data: &[u8]) -> String {
    let text = b64(data);
    let mut out = String::with_capacity(text.len() + text.len() / 76 + 1);
    for (i, ch) in text.chars().enumerate() {
        if i > 0 && i % 76 == 0 {
            out.push('\n');
        }
        out.push(ch);
    }
    out
}

/// Percent-encodes one archive path for a `Reference URI`, keeping `/` between segments.
pub(crate) fn encode_uri(path: &str) -> String {
    let mut out = String::new();
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._*/".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn xml_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('"', "&quot;")
}

/// The `<Object Id="prop">` element exactly as embedded in the document.
fn object_element(role: Role) -> String {
    let id = role.id();
    format!(
        "<Object Id=\"prop\"><SignatureProperties xmlns:dsp=\"http://www.w3.org/2009/xmldsig-properties\">\
<SignatureProperty Id=\"profile\" Target=\"#{id}\"><dsp:Profile URI=\"http://www.w3.org/ns/widgets-digsig#profile\"></dsp:Profile></SignatureProperty>\
<SignatureProperty Id=\"role\" Target=\"#{id}\"><dsp:Role URI=\"{}\"></dsp:Role></SignatureProperty>\
<SignatureProperty Id=\"identifier\" Target=\"#{id}\"><dsp:Identifier></dsp:Identifier></SignatureProperty>\
</SignatureProperties></Object>",
        role.uri()
    )
}

/// Builds one signature document. `references` are `(archive path, SHA-512 digest)`.
///
/// The text is generated directly in canonical form (explicit end tags, sorted attributes,
/// no insignificant attribute whitespace), so the canonicalised `SignedInfo` and `Object`
/// differ from the embedded text only by the inherited `xmlns` declaration.
fn build_signature(
    role: Role,
    references: &[(String, Vec<u8>)],
    identity: &SigningIdentity,
) -> Result<String, SignError> {
    let object = object_element(role);
    // C14N 1.1 of the Object subtree: the in-scope default namespace is rendered on the apex.
    let object_canonical = object.replacen("<Object ", &format!("<Object xmlns=\"{DSIG_NS}\" "), 1);
    let object_digest = Sha512::digest(object_canonical.as_bytes());

    let mut body = String::new();
    body.push_str("<CanonicalizationMethod Algorithm=\"http://www.w3.org/2001/10/xml-exc-c14n#\"></CanonicalizationMethod>\n");
    body.push_str("<SignatureMethod Algorithm=\"http://www.w3.org/2001/04/xmldsig-more#rsa-sha512\"></SignatureMethod>\n");
    for (path, digest) in references {
        body.push_str(&format!(
            "<Reference URI=\"{}\">\n<DigestMethod Algorithm=\"http://www.w3.org/2001/04/xmlenc#sha512\"></DigestMethod>\n<DigestValue>{}</DigestValue>\n</Reference>\n",
            xml_attr(&encode_uri(path)),
            b64(digest)
        ));
    }
    body.push_str(&format!(
        "<Reference URI=\"#prop\">\n<Transforms>\n<Transform Algorithm=\"http://www.w3.org/2006/12/xml-c14n11\"></Transform>\n</Transforms>\n<DigestMethod Algorithm=\"http://www.w3.org/2001/04/xmlenc#sha512\"></DigestMethod>\n<DigestValue>{}</DigestValue>\n</Reference>\n",
        b64(&object_digest)
    ));

    let signed_info_canonical = format!("<SignedInfo xmlns=\"{DSIG_NS}\">\n{body}</SignedInfo>");
    let signer = SigningKey::<Sha512>::new(identity.key.clone());
    let signature = signer
        .try_sign(signed_info_canonical.as_bytes())
        .map_err(|_| SignError::Crypto)?
        .to_vec();

    let mut xml = format!("<Signature xmlns=\"{DSIG_NS}\" Id=\"{}\">\n", role.id());
    xml.push_str(&format!("<SignedInfo>\n{body}</SignedInfo>\n"));
    xml.push_str(&format!(
        "<SignatureValue>\n{}\n</SignatureValue>\n",
        b64_wrapped(&signature)
    ));
    xml.push_str("<KeyInfo>\n<X509Data>\n");
    for cert in &identity.chain {
        xml.push_str(&format!(
            "<X509Certificate>\n{}\n</X509Certificate>\n",
            b64_wrapped(cert)
        ));
    }
    xml.push_str("</X509Data>\n</KeyInfo>\n");
    xml.push_str(&object);
    xml.push_str("\n</Signature>\n");
    Ok(xml)
}

fn is_signature_file(name: &str) -> bool {
    name == AUTHOR_SIGNATURE
        || (name.starts_with("signature")
            && name.ends_with(".xml")
            && !name.contains('/')
            && name["signature".len()..name.len() - 4]
                .bytes()
                .all(|b| b.is_ascii_digit())
            && name.len() > "signature.xml".len())
}

/// Signs `source` into a new file at `destination`. Existing signatures are replaced.
/// `progress` receives a fraction between 0 and 1.
pub fn sign_wgt(
    source: &Path,
    destination: &Path,
    author: &SigningIdentity,
    distributor: &SigningIdentity,
    cancel: &CancelToken,
    progress: &mut dyn FnMut(f32),
) -> Result<(), SignError> {
    // Same safety gate as the UI analysis: archive bombs, zip-slip names, broken config.
    analyze_wgt(source)?;
    let mut archive = zip::ZipArchive::new(File::open(source).map_err(|_| SignError::Read)?)
        .map_err(|_| SignError::Read)?;

    let mut seen = HashSet::new();
    let mut references: Vec<(String, Vec<u8>)> = Vec::new();
    let mut total: u64 = 0;
    let count = archive.len();
    for index in 0..count {
        if cancel.is_cancelled() {
            return Err(SignError::Cancelled);
        }
        let mut entry = archive.by_index(index).map_err(|_| SignError::Read)?;
        let name = entry.name().to_string();
        if !seen.insert(name.clone()) {
            return Err(SignError::DuplicateEntry);
        }
        if entry.is_dir() || is_signature_file(&name) {
            continue;
        }
        let mut hasher = Sha512::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let n = entry.read(&mut buffer).map_err(|_| SignError::Read)?;
            if n == 0 {
                break;
            }
            total += n as u64;
            if total > MAX_TOTAL_UNCOMPRESSED {
                return Err(SignError::Package(PackageError::Unsafe(
                    "uncompressed_size_limit".into(),
                )));
            }
            hasher.update(&buffer[..n]);
        }
        references.push((name, hasher.finalize().to_vec()));
        progress(0.8 * (index + 1) as f32 / count.max(1) as f32);
    }

    let author_xml = build_signature(Role::Author, &references, author)?;
    let mut distributor_refs = references.clone();
    distributor_refs.push((
        AUTHOR_SIGNATURE.to_string(),
        Sha512::digest(author_xml.as_bytes()).to_vec(),
    ));
    let distributor_xml = build_signature(Role::Distributor, &distributor_refs, distributor)?;
    progress(0.9);

    let out = File::create(destination).map_err(|_| SignError::Write)?;
    let mut writer = zip::ZipWriter::new(out);
    for index in 0..count {
        if cancel.is_cancelled() {
            drop(writer);
            let _ = std::fs::remove_file(destination);
            return Err(SignError::Cancelled);
        }
        let entry = archive.by_index_raw(index).map_err(|_| SignError::Read)?;
        if is_signature_file(entry.name()) {
            continue;
        }
        writer.raw_copy_file(entry).map_err(|_| SignError::Write)?;
    }
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, xml) in [
        (AUTHOR_SIGNATURE, &author_xml),
        (DISTRIBUTOR_SIGNATURE, &distributor_xml),
    ] {
        writer
            .start_file(name, options)
            .map_err(|_| SignError::Write)?;
        writer
            .write_all(xml.as_bytes())
            .map_err(|_| SignError::Write)?;
    }
    writer.finish().map_err(|_| SignError::Write)?;
    progress(1.0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_encoding() {
        assert_eq!(encode_uri("css/style.css"), "css/style.css");
        assert_eq!(
            encode_uri("img/my file (1).png"),
            "img/my%20file%20%281%29.png"
        );
        assert_eq!(
            encode_uri("данные.json"),
            "%D0%B4%D0%B0%D0%BD%D0%BD%D1%8B%D0%B5.json"
        );
    }

    #[test]
    fn signature_file_names() {
        assert!(is_signature_file("author-signature.xml"));
        assert!(is_signature_file("signature1.xml"));
        assert!(is_signature_file("signature22.xml"));
        assert!(!is_signature_file("signature.xml"));
        assert!(!is_signature_file("js/signature1.xml"));
        assert!(!is_signature_file("signatureX.xml"));
    }
}
