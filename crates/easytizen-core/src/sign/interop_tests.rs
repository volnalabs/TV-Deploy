//! End-to-end check of the native signer against independent implementations:
//!
//! * keys, certificates and `.p12` files are produced by the OpenSSL CLI with the same
//!   flags the Samsung CLI uses, so the reader is tested on foreign keystores;
//! * the signatures are verified by `xmlsec1`;
//! * the signed package is then installed on the protocol-level TV emulator.
//!
//! Skipped when `openssl`/`xmlsec1` are not installed, unless `EASYTIZEN_REQUIRE_INTEROP=1`
//! (set in CI) — then a missing tool is a failure, never a silent pass.

use super::identity::{write_p12, SigningIdentity};
use super::wgt::sign_wgt;
use crate::cancel::CancelToken;
use crate::package::{analyze_wgt, SignatureState};
use crate::sdb::mock::MockTv;
use crate::sdb::{install_wgt, InstallFailure, InstallVerdict};
use std::io::Write;
use std::net::Ipv4Addr;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const PASSWORD: &str = "Passw0rdX";
const DUID: &str = "MOCKDUID1234ABCD";

fn tool(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
        || Command::new(name)
            .arg("version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
}

fn run(dir: &Path, program: &str, args: &[&str]) {
    let out = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{program} {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// CA + author + distributor material, the way Samsung's online CA and CLI would shape it.
fn make_keystores(dir: &Path) {
    run(
        dir,
        "openssl",
        &[
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-keyout",
            "ca.key",
            "-out",
            "ca.pem",
            "-subj",
            "/CN=Test VD CA",
            "-days",
            "30",
            "-addext",
            "basicConstraints=critical,CA:TRUE",
        ],
    );
    std::fs::write(
        dir.join("dist.cnf"),
        format!("[req]\nprompt = no\ndistinguished_name = dn\nreq_extensions = v3_req\n\n[dn]\nCN = TizenSDK\n\n[v3_req]\nsubjectAltName = @alt_names\n\n[alt_names]\nURI.1 = URN:tizen:packageid=\nURI.2 = URN:tizen:deviceid={DUID}\n"),
    )
    .unwrap();
    for (name, extra) in [
        ("author", vec!["-subj", "/CN=Test Author"]),
        ("distributor", vec!["-config", "dist.cnf"]),
    ] {
        let key = format!("{name}.key");
        let csr = format!("{name}.csr");
        let crt = format!("{name}.crt");
        let p12 = format!("{name}.p12");
        run(dir, "openssl", &["genrsa", "-out", &key, "2048"]);
        let mut req = vec!["req", "-new", "-key", &key, "-out", &csr, "-sha512"];
        req.extend(extra);
        run(dir, "openssl", &req);
        run(
            dir,
            "openssl",
            &[
                "x509",
                "-req",
                "-in",
                &csr,
                "-CA",
                "ca.pem",
                "-CAkey",
                "ca.key",
                "-CAcreateserial",
                "-out",
                &crt,
                "-days",
                "30",
                "-copy_extensions",
                "copy",
            ],
        );
        run(
            dir,
            "openssl",
            &[
                "pkcs12",
                "-export",
                "-in",
                &crt,
                "-inkey",
                &key,
                "-certfile",
                "ca.pem",
                "-name",
                name,
                "-certpbe",
                "PBE-SHA1-3DES",
                "-keypbe",
                "PBE-SHA1-3DES",
                "-macalg",
                "sha1",
                "-passout",
                &format!("pass:{PASSWORD}"),
                "-out",
                &p12,
            ],
        );
    }
}

fn make_wgt(path: &Path) {
    use zip::write::SimpleFileOptions;
    let mut w = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let files: [(&str, &[u8]); 5] = [
        ("config.xml", br#"<?xml version="1.0" encoding="UTF-8"?><widget xmlns="http://www.w3.org/ns/widgets" xmlns:tizen="http://tizen.org/ns/widgets" id="http://example.org/Demo" version="1.0.0"><tizen:application id="abcde12345.Demo" package="abcde12345" required_version="6.0"/><name>Demo</name></widget>"#),
        ("index.html", b"<html><body>demo</body></html>"),
        ("css/my style (1).css", b"body{}"),
        ("данные.json", b"{}"),
        ("signature1.xml", b"<stale/>"),
    ];
    for (name, data) in files {
        w.start_file(name, SimpleFileOptions::default()).unwrap();
        w.write_all(data).unwrap();
    }
    w.finish().unwrap();
}

fn xmlsec_verifies(dir: &Path, file: &str) -> bool {
    Command::new("xmlsec1")
        .args([
            "--verify",
            "--trusted-pem",
            "ca.pem",
            "--id-attr:Id",
            "http://www.w3.org/2000/09/xmldsig#:Object",
            file,
        ])
        .current_dir(dir)
        .output()
        .map(|o| o.status.success())
        .unwrap()
}

#[test]
fn sign_verify_with_xmlsec_and_install_on_emulated_tv() {
    if !(tool("openssl") && tool("xmlsec1")) {
        assert!(
            std::env::var_os("EASYTIZEN_REQUIRE_INTEROP").is_none(),
            "openssl and xmlsec1 are required for the signing interop test"
        );
        eprintln!("SKIPPED: openssl/xmlsec1 not installed");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    make_keystores(dir);

    let author =
        SigningIdentity::from_p12(&std::fs::read(dir.join("author.p12")).unwrap(), PASSWORD)
            .unwrap();
    let distributor = SigningIdentity::from_p12(
        &std::fs::read(dir.join("distributor.p12")).unwrap(),
        PASSWORD,
    )
    .unwrap();
    assert_eq!(author.info.subject, "Test Author");
    assert_eq!(author.info.issuer, "Test VD CA");
    assert!(author.info.duids.is_empty());
    assert_eq!(distributor.info.duids, [DUID]);
    assert_eq!(
        author.chain.len(),
        2,
        "signer certificate followed by its CA"
    );
    assert!(author.info.not_after > author.info.not_before);
    assert!(
        SigningIdentity::from_p12(&std::fs::read(dir.join("author.p12")).unwrap(), "wrong")
            .is_err()
    );
    assert!(SigningIdentity::from_p12(b"not a keystore", PASSWORD).is_err());

    let source = dir.join("app.wgt");
    make_wgt(&source);
    let original = std::fs::read(&source).unwrap();
    let signed = dir.join("signed.wgt");
    let mut last = 0.0;
    sign_wgt(
        &source,
        &signed,
        &author,
        &distributor,
        &CancelToken::new(),
        &mut |p| last = p,
    )
    .unwrap();
    assert_eq!(last, 1.0);
    assert_eq!(
        std::fs::read(&source).unwrap(),
        original,
        "the original must never change"
    );
    let report = analyze_wgt(&signed).unwrap();
    assert_eq!(report.signature, SignatureState::SignedStructure);
    assert_eq!(report.app_id.as_deref(), Some("abcde12345.Demo"));

    // Independent cryptographic verification of both documents.
    let unpacked = dir.join("unpacked");
    std::fs::create_dir(&unpacked).unwrap();
    let mut archive = zip::ZipArchive::new(std::fs::File::open(&signed).unwrap()).unwrap();
    assert_eq!(
        archive.len(),
        6,
        "4 content files + 2 fresh signatures; the stale one is replaced"
    );
    archive.extract(&unpacked).unwrap();
    std::fs::copy(dir.join("ca.pem"), unpacked.join("ca.pem")).unwrap();
    assert!(
        xmlsec_verifies(&unpacked, "author-signature.xml"),
        "xmlsec1 rejected the author signature"
    );
    assert!(
        xmlsec_verifies(&unpacked, "signature1.xml"),
        "xmlsec1 rejected the distributor signature"
    );
    // …and it really checks content: a modified file must break both.
    std::fs::write(unpacked.join("index.html"), b"tampered").unwrap();
    assert!(!xmlsec_verifies(&unpacked, "author-signature.xml"));
    assert!(!xmlsec_verifies(&unpacked, "signature1.xml"));

    // Install on the emulated TV whose DUID the distributor certificate names.
    let install = |tv: &MockTv| {
        let bytes = std::fs::read(&signed).unwrap();
        install_wgt(
            Ipv4Addr::LOCALHOST,
            tv.port,
            &mut bytes.as_slice(),
            bytes.len() as u64,
            "abcde12345.Demo",
            true,
            Duration::from_secs(5),
            &CancelToken::new(),
            &mut |_, _| {},
        )
        .unwrap()
    };
    let tv = MockTv::start(Ipv4Addr::LOCALHOST, 0, DUID).unwrap();
    let outcome = install(&tv);
    assert_eq!(
        outcome.verdict,
        InstallVerdict::Installed,
        "{}",
        outcome.log
    );
    assert_eq!(outcome.launched, Some(true));
    assert_eq!(tv.state.lock().unwrap().installed, ["abcde12345.Demo"]);
    // A different TV must refuse the same package.
    let other = MockTv::start(Ipv4Addr::LOCALHOST, 0, "ANOTHERTV00000001").unwrap();
    assert_eq!(
        install(&other).verdict,
        InstallVerdict::Failed(InstallFailure::CertificateRejected)
    );

    // Keystores written by EasyTizen are readable by EasyTizen and by OpenSSL.
    let ours = write_p12(&author.key, &author.chain, "author", PASSWORD).unwrap();
    let reread = SigningIdentity::from_p12(&ours, PASSWORD).unwrap();
    assert_eq!(reread.info, author.info);
    assert_eq!(reread.chain, author.chain);
    std::fs::write(dir.join("ours.p12"), &ours).unwrap();
    run(
        dir,
        "openssl",
        &[
            "pkcs12",
            "-in",
            "ours.p12",
            "-passin",
            &format!("pass:{PASSWORD}"),
            "-noout",
        ],
    );
}
