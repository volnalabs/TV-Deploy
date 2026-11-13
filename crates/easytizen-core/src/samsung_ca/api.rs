//! HTTPS client for Samsung's online certificate authority.
//!
//! Endpoints and form fields are those used by the pinned Samsung CLI (`samsung-api.js`).

use super::oauth::SamsungToken;
use super::CaError;
use std::time::Duration;

pub const SAMSUNG_CA_BASE_URL: &str = "https://svdca.samsungqbe.com";
const AUTHOR_PATH: &str = "/apis/v3/authors";
const DISTRIBUTOR_V1_PATH: &str = "/apis/v1/distributors";
const DISTRIBUTOR_V3_PATH: &str = "/apis/v3/distributors";

pub struct CaClient {
    base_url: String,
    agent: ureq::Agent,
}

impl Default for CaClient {
    fn default() -> Self {
        Self::new(SAMSUNG_CA_BASE_URL)
    }
}

enum Part<'a> {
    Field(&'a str, &'a str),
    File(&'a str, &'a str, &'a str),
}

fn multipart(boundary: &str, parts: &[Part]) -> Vec<u8> {
    let mut body = Vec::new();
    for part in parts {
        match part {
            Part::Field(name, value) => body.extend_from_slice(
                format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes(),
            ),
            Part::File(name, filename, content) => body.extend_from_slice(
                format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\nContent-Type: application/octet-stream\r\n\r\n{content}\r\n").as_bytes(),
            ),
        }
    }
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    body
}

impl CaClient {
    /// `base_url` is overridable so tests can point the client at a local stand-in.
    pub fn new(base_url: &str) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(60)))
            .http_status_as_error(false)
            .build()
            .into();
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            agent,
        }
    }

    fn post(&self, path: &str, parts: &[Part]) -> Result<String, CaError> {
        let boundary = format!(
            "----EasyTizen{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let body = multipart(&boundary, parts);
        let mut response = self
            .agent
            .post(format!("{}{path}", self.base_url))
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .send(&body[..])
            // Never include the request (it carries the access token) in an error.
            .map_err(|e| CaError::Network(network_reason(&e)))?;
        let status = response.status().as_u16();
        if status == 403 {
            return Err(CaError::RateLimited);
        }
        if status != 200 {
            return Err(CaError::Http(status));
        }
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| CaError::Network(network_reason(&e)))?;
        if text.trim().is_empty() {
            return Err(CaError::BadCertificate);
        }
        Ok(text)
    }

    pub fn sign_author(&self, token: &SamsungToken, csr_pem: &str) -> Result<String, CaError> {
        self.post(
            AUTHOR_PATH,
            &[
                Part::Field("access_token", token.access_token()),
                Part::Field("user_id", &token.user_id),
                Part::Field("platform", "VD"),
                Part::File("csr", "author.csr", csr_pem),
            ],
        )
    }

    /// Public-privilege distributor certificate for TVs. As in the reference flow, the v1
    /// endpoint is called first and the v3 endpoint returns the certificate that is used.
    pub fn sign_distributor(&self, token: &SamsungToken, csr_pem: &str) -> Result<String, CaError> {
        self.post(
            DISTRIBUTOR_V1_PATH,
            &[
                Part::Field("access_token", token.access_token()),
                Part::Field("user_id", &token.user_id),
                Part::Field("privilege_level", "Public"),
                Part::Field("developer_type", "Individual"),
                Part::Field("platform", "VD"),
                Part::File("csr", "distributor.csr", csr_pem),
            ],
        )?;
        self.post(
            DISTRIBUTOR_V3_PATH,
            &[
                Part::Field("access_token", token.access_token()),
                Part::Field("user_id", &token.user_id),
                Part::Field("privilege_level", "Public"),
                Part::Field("developer_type", "Individual"),
                Part::File("csr", "distributor.csr", csr_pem),
            ],
        )
    }
}

fn network_reason(error: &ureq::Error) -> String {
    match error {
        ureq::Error::Timeout(_) => "timed out".into(),
        ureq::Error::HostNotFound => "host not found (no internet connection?)".into(),
        ureq::Error::Io(e) => format!("{:?}", e.kind()),
        ureq::Error::Tls(_) | ureq::Error::Rustls(_) => "secure connection failed".into(),
        _ => "request failed".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multipart_layout() {
        let body = multipart(
            "B",
            &[
                Part::Field("user_id", "u1"),
                Part::File("csr", "author.csr", "PEM"),
            ],
        );
        let text = String::from_utf8(body).unwrap();
        assert_eq!(
            text,
            "--B\r\nContent-Disposition: form-data; name=\"user_id\"\r\n\r\nu1\r\n--B\r\nContent-Disposition: form-data; name=\"csr\"; filename=\"author.csr\"\r\nContent-Type: application/octet-stream\r\n\r\nPEM\r\n\r\n--B--\r\n"
        );
    }
}
