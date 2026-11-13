//! Loading and creating PKCS#12 keystores (`author.p12`, `distributor.p12`).

use p12_keystore::{
    Certificate as P12Certificate, EncryptionAlgorithm, KeyStore, KeyStoreEntry, MacAlgorithm,
    Pkcs12Archive, PrivateKey as P12PrivateKey, PrivateKeyChain,
};
use rsa::pkcs8::{DecodePrivateKey, EncodePrivateKey, EncodePublicKey};
use rsa::PrivateKey as RsaPrivateKey;
use serde::Serialize;
use thiserror::Error;
use x509_cert::der::{Decode, Encode};
use x509_cert::ext::pkix::name::GeneralName;
use x509_cert::ext::pkix::SubjectAltName;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum IdentityError {
    #[error("wrong password, or the file is not a PKCS#12 keystore")]
    BadPasswordOrFormat,
    #[error("the keystore has no private key")]
    NoPrivateKey,
    #[error("the keystore has no certificate for its private key")]
    NoCertificate,
    #[error("only RSA keys are supported")]
    UnsupportedKey,
    #[error("certificate could not be read")]
    BadCertificate,
    #[error("could not write the keystore")]
    WriteFailed,
}

/// Public facts about a certificate, safe to store and show.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CertInfo {
    pub subject: String,
    pub issuer: String,
    /// Unix seconds.
    pub not_before: i64,
    pub not_after: i64,
    /// TVs (DUIDs) a distributor certificate is valid for; empty for author certificates.
    pub duids: Vec<String>,
}

pub struct SigningIdentity {
    pub(crate) key: RsaPrivateKey,
    /// DER certificates, signer first, then its issuers.
    pub(crate) chain: Vec<Vec<u8>>,
    pub info: CertInfo,
}

impl std::fmt::Debug for SigningIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print key material.
        f.debug_struct("SigningIdentity")
            .field("info", &self.info)
            .finish_non_exhaustive()
    }
}

fn common_name_or_full(name: &x509_cert::name::Name) -> String {
    name.common_name()
        .ok()
        .flatten()
        .map(|cn| cn.value().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| name.to_string())
}

pub fn cert_info(der: &[u8]) -> Result<CertInfo, IdentityError> {
    let cert = x509_cert::Certificate::from_der(der).map_err(|_| IdentityError::BadCertificate)?;
    let tbs = cert.tbs_certificate();
    let mut duids = Vec::new();
    if let Ok(Some((_, san))) = tbs.get_extension::<SubjectAltName>() {
        for name in san.0 {
            if let GeneralName::UniformResourceIdentifier(uri) = name {
                if let Some(duid) = uri.as_str().strip_prefix("URN:tizen:deviceid=") {
                    if !duid.is_empty() {
                        duids.push(duid.to_string());
                    }
                }
            }
        }
    }
    let secs = |t: &x509_cert::time::Time| t.to_unix_duration().as_secs() as i64;
    Ok(CertInfo {
        subject: common_name_or_full(tbs.subject()),
        issuer: common_name_or_full(tbs.issuer()),
        not_before: secs(&tbs.validity().not_before),
        not_after: secs(&tbs.validity().not_after),
        duids,
    })
}

fn spki_of_cert(der: &[u8]) -> Option<Vec<u8>> {
    x509_cert::Certificate::from_der(der)
        .ok()?
        .tbs_certificate()
        .subject_public_key_info()
        .to_der()
        .ok()
}

impl SigningIdentity {
    /// Opens a `.p12` produced by Tizen Studio, the Samsung CLI, OpenSSL or EasyTizen itself.
    pub fn from_p12(data: &[u8], password: &str) -> Result<Self, IdentityError> {
        let archive = Pkcs12Archive::from_pkcs12(data, password)
            .map_err(|_| IdentityError::BadPasswordOrFormat)?;
        let key_bag = archive.keys.first().ok_or(IdentityError::NoPrivateKey)?;
        let key = RsaPrivateKey::from_pkcs8_der(key_bag.key.as_der())
            .map_err(|_| IdentityError::UnsupportedKey)?;
        let key_spki = key
            .to_public_key()
            .to_public_key_der()
            .map_err(|_| IdentityError::UnsupportedKey)?
            .as_bytes()
            .to_vec();

        // The signer certificate is the one carrying this key's public half.
        let leaf = archive
            .certs
            .iter()
            .find(|c| spki_of_cert(c.cert.as_der()).as_deref() == Some(key_spki.as_slice()))
            .ok_or(IdentityError::NoCertificate)?;
        let mut chain = vec![leaf.cert.as_der().to_vec()];
        let mut current = &leaf.cert;
        while chain.len() < 6 {
            let Some(issuer) = archive.certs.iter().find(|c| {
                c.cert.subject() == current.issuer() && c.cert.as_der() != current.as_der()
            }) else {
                break;
            };
            if chain.iter().any(|c| c == issuer.cert.as_der()) {
                break;
            }
            chain.push(issuer.cert.as_der().to_vec());
            current = &issuer.cert;
        }
        let info = cert_info(&chain[0])?;
        Ok(Self { key, chain, info })
    }
}

/// Packages a key, its certificate and the issuing CA into a password-protected PKCS#12
/// file. The encryption (PBE-SHA1-3DES, HMAC-SHA1) matches what the Samsung CLI writes, so
/// the result can also be imported into Tizen Studio.
pub fn write_p12(
    key: &RsaPrivateKey,
    certs_der: &[Vec<u8>],
    alias: &str,
    password: &str,
) -> Result<Vec<u8>, IdentityError> {
    let pkcs8 = key.to_pkcs8_der().map_err(|_| IdentityError::WriteFailed)?;
    let p12_key =
        P12PrivateKey::from_der(pkcs8.as_bytes()).map_err(|_| IdentityError::WriteFailed)?;
    let certs = certs_der
        .iter()
        .map(|der| P12Certificate::from_der(der))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| IdentityError::BadCertificate)?;
    if certs.is_empty() {
        return Err(IdentityError::NoCertificate);
    }
    let mut store = KeyStore::new();
    store.add_entry(
        alias,
        KeyStoreEntry::PrivateKeyChain(PrivateKeyChain::new(
            alias.as_bytes().to_vec(),
            p12_key,
            certs,
        )),
    );
    store
        .writer(password)
        .encryption_algorithm(EncryptionAlgorithm::PbeWithShaAnd3KeyTripleDesCbc)
        .mac_algorithm(MacAlgorithm::HmacSha1)
        .write()
        .map_err(|_| IdentityError::WriteFailed)
}
