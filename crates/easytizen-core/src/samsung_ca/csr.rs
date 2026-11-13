//! Local key generation and certificate signing requests.
//!
//! Shapes match the Samsung CLI: RSA-2048, SHA-512; the author request carries the author's
//! name as CN; the distributor request is `CN=TizenSDK` with the TV DUIDs as
//! `URN:tizen:deviceid=<duid>` subject alternative names.

use super::CaError;
use rsa::pkcs1v15::SigningKey;
use rsa::sha2::Sha512;
use rsa::PrivateKey;
use std::str::FromStr;
use x509_cert::builder::{Builder, RequestBuilder};
use x509_cert::der::asn1::Ia5String;
use x509_cert::der::pem::LineEnding;
use x509_cert::der::EncodePem;
use x509_cert::ext::pkix::name::GeneralName;
use x509_cert::ext::pkix::SubjectAltName;
use x509_cert::name::Name;

pub struct CertRequest {
    pub key: PrivateKey,
    pub csr_pem: String,
}

pub const MAX_DUIDS: usize = 50;

pub fn is_valid_duid(duid: &str) -> bool {
    (4..=128).contains(&duid.len())
        && duid
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"#.-:=".contains(&b))
}

/// Escapes a value for an RFC 4514 distinguished-name string.
fn escape_dn_value(value: &str) -> String {
    let mut out = String::new();
    for (i, ch) in value.chars().enumerate() {
        if matches!(ch, ',' | '+' | '"' | '\\' | '<' | '>' | ';' | '=')
            || (i == 0 && matches!(ch, '#' | ' '))
        {
            out.push('\\');
        }
        out.push(ch);
    }
    if out.ends_with(' ') {
        out.insert(out.len() - 1, '\\');
    }
    out
}

fn generate_key() -> Result<PrivateKey, CaError> {
    PrivateKey::new(&mut getrandom::SysRng, 2048).map_err(|_| CaError::KeyGeneration)
}

fn build(subject: &str, san: Option<SubjectAltName>) -> Result<CertRequest, CaError> {
    let key = generate_key()?;
    let signer = SigningKey::<Sha512>::new(key.clone());
    let subject = Name::from_str(subject).map_err(|_| CaError::InvalidInput)?;
    let mut builder = RequestBuilder::new(subject).map_err(|_| CaError::InvalidInput)?;
    if let Some(san) = san {
        builder
            .add_extension(&san)
            .map_err(|_| CaError::InvalidInput)?;
    }
    let request = builder
        .build::<_, rsa::pkcs1v15::Signature>(&signer)
        .map_err(|_| CaError::KeyGeneration)?;
    let csr_pem = request
        .to_pem(LineEnding::LF)
        .map_err(|_| CaError::KeyGeneration)?;
    Ok(CertRequest { key, csr_pem })
}

pub fn author_csr(author_name: &str) -> Result<CertRequest, CaError> {
    let name = author_name.trim();
    if !(2..=64).contains(&name.chars().count()) || name.chars().any(|c| c.is_control()) {
        return Err(CaError::InvalidInput);
    }
    build(&format!("CN={}", escape_dn_value(name)), None)
}

pub fn distributor_csr(duids: &[String]) -> Result<CertRequest, CaError> {
    if duids.is_empty() || duids.len() > MAX_DUIDS || !duids.iter().all(|d| is_valid_duid(d)) {
        return Err(CaError::InvalidInput);
    }
    let uri = |value: String| -> Result<GeneralName, CaError> {
        Ok(GeneralName::UniformResourceIdentifier(
            Ia5String::new(&value).map_err(|_| CaError::InvalidInput)?,
        ))
    };
    let mut names = vec![uri("URN:tizen:packageid=".to_string())?];
    for duid in duids {
        names.push(uri(format!("URN:tizen:deviceid={duid}"))?);
    }
    build("CN=TizenSDK", Some(SubjectAltName(names)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dn_escaping() {
        assert_eq!(escape_dn_value("Ivan Petrov"), "Ivan Petrov");
        assert_eq!(escape_dn_value("A, B+C=D"), "A\\, B\\+C\\=D");
        assert_eq!(escape_dn_value("#lead"), "\\#lead");
    }

    #[test]
    fn rejects_bad_input_before_generating_keys() {
        assert_eq!(author_csr("x").err(), Some(CaError::InvalidInput));
        assert_eq!(author_csr("a\nb").err(), Some(CaError::InvalidInput));
        assert_eq!(distributor_csr(&[]).err(), Some(CaError::InvalidInput));
        assert_eq!(
            distributor_csr(&["bad duid".into()]).err(),
            Some(CaError::InvalidInput)
        );
    }
}
