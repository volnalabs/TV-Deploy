//! Issuing Samsung developer certificates without the Tizen SDK.
//!
//! This is a Rust port of the flow implemented by Samsung's own open-source CLI (vendored in
//! `resources/samsung-cli/lib/core/samsung-{auth,api,cert}.js`, Apache-2.0):
//!
//! 1. the user signs in to their Samsung Account in the system browser; Samsung redirects
//!    the result to a loopback address this app listens on ([`oauth`]);
//! 2. an RSA-2048 key and a CSR are generated locally ([`csr`]) — private keys never leave
//!    the device;
//! 3. Samsung's certificate authority signs the CSRs ([`api`]);
//! 4. key + certificate + Samsung CA are packaged into `author.p12` / `distributor.p12`.
//!
//! Unlike the reference implementation, TLS certificates are always verified.
//! The exchange with Samsung's servers has not been exercised in automated tests (it needs a
//! real Samsung Account); the request shapes are tested against a local stand-in.

pub mod api;
pub mod csr;
pub mod oauth;

use crate::sign::identity::{write_p12, IdentityError};
use thiserror::Error;

pub const AUTHOR_CA_PEM: &str = include_str!(
    "../../../../resources/samsung-cli/assets/samsung-tv-ca/vd_tizen_dev_author_ca.cer"
);
pub const PUBLIC_DISTRIBUTOR_CA_PEM: &str =
    include_str!("../../../../resources/samsung-cli/assets/samsung-tv-ca/vd_tizen_dev_public2.crt");

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CaError {
    #[error("no free local port for the Samsung sign-in callback (4794–4813)")]
    PortUnavailable,
    #[error("Samsung sign-in was not completed in time")]
    AuthTimeout,
    #[error("Samsung sign-in returned an unexpected answer")]
    AuthInvalid,
    #[error("cancelled")]
    Cancelled,
    #[error("could not reach Samsung's certificate service: {0}")]
    Network(String),
    #[error(
        "Samsung is limiting requests for this account right now (HTTP 403). Try again later."
    )]
    RateLimited,
    #[error("Samsung's certificate service answered HTTP {0}")]
    Http(u16),
    #[error("Samsung returned something that is not a certificate")]
    BadCertificate,
    #[error("could not generate a key")]
    KeyGeneration,
    #[error("invalid certificate request data")]
    InvalidInput,
    #[error("keystore error: {0}")]
    Keystore(#[from] IdentityError),
}

/// Result of a completed issuance: two password-protected keystores.
pub struct IssuedCertificates {
    pub author_p12: Vec<u8>,
    pub distributor_p12: Vec<u8>,
}

fn pem_chain_to_der(pem: &str) -> Result<Vec<Vec<u8>>, CaError> {
    use x509_cert::der::Encode;
    let certs = x509_cert::Certificate::load_pem_chain(pem.as_bytes())
        .map_err(|_| CaError::BadCertificate)?;
    if certs.is_empty() {
        return Err(CaError::BadCertificate);
    }
    certs
        .iter()
        .map(|c| c.to_der().map_err(|_| CaError::BadCertificate))
        .collect()
}

/// Requests an author certificate and packages it as `author.p12`.
pub fn issue_author(
    client: &api::CaClient,
    token: &oauth::SamsungToken,
    author_name: &str,
    password: &str,
) -> Result<Vec<u8>, CaError> {
    let request = csr::author_csr(author_name)?;
    let pem = client.sign_author(token, &request.csr_pem)?;
    let mut chain = pem_chain_to_der(&pem)?;
    chain.truncate(1);
    chain.extend(pem_chain_to_der(AUTHOR_CA_PEM)?);
    Ok(write_p12(&request.key, &chain, "author", password)?)
}

/// Requests a distributor certificate bound to `duids` and packages it as `distributor.p12`.
pub fn issue_distributor(
    client: &api::CaClient,
    token: &oauth::SamsungToken,
    duids: &[String],
    password: &str,
) -> Result<Vec<u8>, CaError> {
    let request = csr::distributor_csr(duids)?;
    let pem = client.sign_distributor(token, &request.csr_pem)?;
    let mut chain = pem_chain_to_der(&pem)?;
    chain.truncate(1);
    chain.extend(pem_chain_to_der(PUBLIC_DISTRIBUTOR_CA_PEM)?);
    Ok(write_p12(&request.key, &chain, "distributor", password)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_samsung_ca_certificates_parse() {
        assert_eq!(pem_chain_to_der(AUTHOR_CA_PEM).unwrap().len(), 1);
        assert_eq!(
            pem_chain_to_der(PUBLIC_DISTRIBUTOR_CA_PEM).unwrap().len(),
            1
        );
        assert_eq!(
            pem_chain_to_der("<html>error</html>"),
            Err(CaError::BadCertificate)
        );
    }
}

#[cfg(all(test, unix))]
mod issuance_tests {
    //! Certificate issuance against a local stand-in for Samsung's CA: every CSR we send is
    //! checked and signed by the OpenSSL CLI, so the request format, the CSR contents and the
    //! keystore packaging are validated by an independent implementation. The real Samsung
    //! service is NOT contacted (it requires a Samsung Account).
    use super::*;
    use crate::cancel::CancelToken;
    use crate::sign::{sign_wgt, SigningIdentity};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::{Arc, Mutex};

    fn openssl(dir: &Path, args: &[&str]) -> Vec<u8> {
        let out = Command::new("openssl")
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "openssl {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        out.stdout
    }

    type Seen = Arc<Mutex<Vec<(String, String)>>>;

    /// Serves `count` requests; returns `(path, body)` of each.
    fn stand_in_ca(dir: PathBuf, count: usize, status: u16) -> (String, Seen) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        std::thread::spawn(move || {
            for index in 0..count {
                let (mut stream, _) = listener.accept().unwrap();
                let mut data = Vec::new();
                let mut buffer = [0u8; 8192];
                let (head_end, length) = loop {
                    let n = stream.read(&mut buffer).unwrap();
                    data.extend_from_slice(&buffer[..n]);
                    if let Some(pos) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&data[..pos]).to_ascii_lowercase();
                        let length: usize = head
                            .lines()
                            .find_map(|l| {
                                l.strip_prefix("content-length:")
                                    .map(|v| v.trim().parse().unwrap())
                            })
                            .unwrap();
                        break (pos + 4, length);
                    }
                };
                while data.len() < head_end + length {
                    let n = stream.read(&mut buffer).unwrap();
                    data.extend_from_slice(&buffer[..n]);
                }
                let head = String::from_utf8_lossy(&data[..head_end]).to_string();
                let path = head.split_whitespace().nth(1).unwrap().to_string();
                let body = String::from_utf8_lossy(&data[head_end..]).to_string();
                let answer = if status == 200 {
                    let start = body.find("-----BEGIN CERTIFICATE REQUEST-----").unwrap();
                    let end = body.find("-----END CERTIFICATE REQUEST-----").unwrap() + 33;
                    let csr = format!("req{index}.csr");
                    std::fs::write(dir.join(&csr), &body[start..end]).unwrap();
                    // OpenSSL must accept the CSR's self-signature…
                    openssl(&dir, &["req", "-in", &csr, "-verify", "-noout"]);
                    // …and then plays the certificate authority.
                    String::from_utf8(openssl(
                        &dir,
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
                            "-days",
                            "30",
                            "-copy_extensions",
                            "copy",
                        ],
                    ))
                    .unwrap()
                } else {
                    "denied".to_string()
                };
                log.lock().unwrap().push((path, body));
                let reason = if status == 200 { "OK" } else { "Error" };
                stream
                    .write_all(format!("HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}", answer.len()).as_bytes())
                    .unwrap();
            }
        });
        (url, seen)
    }

    fn token() -> oauth::SamsungToken {
        oauth::parse_token(r#"{"access_token":"SECRET-TOKEN","userId":"user-42"}"#).unwrap()
    }

    #[test]
    fn issue_author_and_distributor_then_sign_and_install() {
        if Command::new("openssl").arg("version").output().is_err() {
            assert!(
                std::env::var_os("EASYTIZEN_REQUIRE_INTEROP").is_none(),
                "openssl is required"
            );
            eprintln!("SKIPPED: openssl not installed");
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        openssl(
            dir,
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
                "/CN=Stand-in CA",
                "-days",
                "30",
            ],
        );
        let (url, seen) = stand_in_ca(dir.to_path_buf(), 3, 200);
        let client = api::CaClient::new(&url);
        let duid = "EMULATEDTV0001DUID".to_string();

        let author_p12 = issue_author(&client, &token(), "Иван, Petrov+Co", "Passw0rdX").unwrap();
        let distributor_p12 =
            issue_distributor(&client, &token(), std::slice::from_ref(&duid), "Passw0rdX").unwrap();

        let requests = seen.lock().unwrap().clone();
        let paths: Vec<&str> = requests.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(
            paths,
            [
                "/apis/v3/authors",
                "/apis/v1/distributors",
                "/apis/v3/distributors"
            ]
        );
        assert!(requests[0]
            .1
            .contains("name=\"access_token\"\r\n\r\nSECRET-TOKEN\r\n"));
        assert!(requests[0]
            .1
            .contains("name=\"user_id\"\r\n\r\nuser-42\r\n"));
        assert!(requests[0].1.contains("name=\"platform\"\r\n\r\nVD\r\n"));
        assert!(requests[1]
            .1
            .contains("name=\"privilege_level\"\r\n\r\nPublic\r\n"));
        assert!(requests[1]
            .1
            .contains("name=\"developer_type\"\r\n\r\nIndividual\r\n"));
        // The CSR OpenSSL received names the TV.
        let text = String::from_utf8(openssl(dir, &["req", "-in", "req2.csr", "-noout", "-text"]))
            .unwrap();
        assert!(
            text.contains("URI:URN:tizen:packageid=, URI:URN:tizen:deviceid=EMULATEDTV0001DUID"),
            "{text}"
        );
        assert!(text.contains("CN = TizenSDK") || text.contains("CN=TizenSDK"));
        assert!(text.contains("Public-Key: (2048 bit)"));
        assert!(text.contains("sha512WithRSAEncryption"));

        let author = SigningIdentity::from_p12(&author_p12, "Passw0rdX").unwrap();
        let distributor = SigningIdentity::from_p12(&distributor_p12, "Passw0rdX").unwrap();
        assert_eq!(author.info.subject, "Иван, Petrov+Co");
        assert_eq!(distributor.info.duids, std::slice::from_ref(&duid));
        assert!(SigningIdentity::from_p12(&author_p12, "wrong").is_err());

        // The issued pair signs a package that the emulated TV with that DUID installs.
        let source = dir.join("app.wgt");
        {
            use zip::write::SimpleFileOptions;
            let mut w = zip::ZipWriter::new(std::fs::File::create(&source).unwrap());
            w.start_file("config.xml", SimpleFileOptions::default())
                .unwrap();
            w.write_all(br#"<widget xmlns="http://www.w3.org/ns/widgets" xmlns:tizen="http://tizen.org/ns/widgets" version="1.0"><tizen:application id="abcde12345.Demo" package="abcde12345" required_version="6.0"/><name>Demo</name></widget>"#).unwrap();
            w.start_file("index.html", SimpleFileOptions::default())
                .unwrap();
            w.write_all(b"<html/>").unwrap();
            w.finish().unwrap();
        }
        let signed = dir.join("signed.wgt");
        sign_wgt(
            &source,
            &signed,
            &author,
            &distributor,
            &CancelToken::new(),
            &mut |_| {},
        )
        .unwrap();
        let tv = crate::sdb::mock::MockTv::start(std::net::Ipv4Addr::LOCALHOST, 0, &duid).unwrap();
        let bytes = std::fs::read(&signed).unwrap();
        let outcome = crate::sdb::install_wgt(
            std::net::Ipv4Addr::LOCALHOST,
            tv.port,
            &mut bytes.as_slice(),
            bytes.len() as u64,
            "abcde12345.Demo",
            false,
            std::time::Duration::from_secs(5),
            &CancelToken::new(),
            &mut |_, _| {},
        )
        .unwrap();
        assert_eq!(
            outcome.verdict,
            crate::sdb::InstallVerdict::Installed,
            "{}",
            outcome.log
        );
    }

    #[test]
    fn service_errors_are_reported_without_leaking_the_token() {
        let tmp = tempfile::tempdir().unwrap();
        let (url, _) = stand_in_ca(tmp.path().to_path_buf(), 1, 403);
        let err = issue_author(&api::CaClient::new(&url), &token(), "Author", "Passw0rdX")
            .err()
            .unwrap();
        assert_eq!(err, CaError::RateLimited);
        assert!(!err.to_string().contains("SECRET-TOKEN"));
        // Nothing listens here: a network error, still without the token.
        let err = issue_author(
            &api::CaClient::new("http://127.0.0.1:9"),
            &token(),
            "Author",
            "Passw0rdX",
        )
        .err()
        .unwrap();
        assert!(matches!(err, CaError::Network(_)));
        assert!(!err.to_string().contains("SECRET-TOKEN"));
    }
}
