//! Samsung Account sign-in through the system browser.
//!
//! The app listens on a loopback port (4794–4813, as Samsung's tooling does), opens
//! Samsung's sign-in page with `redirect_uri=http://localhost:<port>/signin/callback`, and
//! receives the result as a form POST to that address. Only 127.0.0.1 is listened on, so no
//! other device on the network can deliver a forged answer.

use super::CaError;
use crate::cancel::CancelToken;
use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::time::{Duration, Instant};

const SAMSUNG_OAUTH_URL: &str = "https://account.samsung.com/mobile/account/check.do";
const SERVICE_ID: &str = "v285zxnl3h";
const CALLBACK_PATH: &str = "/signin/callback";
const FIRST_PORT: u16 = 4794;
const PORT_COUNT: u16 = 20;
pub const AUTH_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// A Samsung access token. It lives in memory only and is never written to disk or logs.
#[derive(Clone)]
pub struct SamsungToken {
    access_token: String,
    pub user_id: String,
    pub email: Option<String>,
}

impl SamsungToken {
    pub fn access_token(&self) -> &str {
        &self.access_token
    }
}

impl std::fmt::Debug for SamsungToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SamsungToken")
            .field("access_token", &"[redacted]")
            .finish_non_exhaustive()
    }
}

pub struct OAuthSession {
    listener: TcpListener,
    pub port: u16,
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                let hex = |b: u8| (b as char).to_digit(16);
                match (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                    (Some(high), Some(low)) => {
                        out.push((high * 16 + low) as u8);
                        i += 2;
                    }
                    _ => out.push(b'%'),
                }
            }
            other => out.push(other),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Finds `code=<value>` in a form body or a query string; the value is decoded once.
pub fn extract_auth_code(body: &str, target: &str) -> Option<String> {
    let query = target.split_once('?').map(|(_, q)| q).unwrap_or("");
    for source in [body, query] {
        for pair in source.split('&') {
            if let Some(value) = pair.strip_prefix("code=") {
                return Some(percent_decode(value));
            }
        }
    }
    None
}

pub fn parse_token(code_json: &str) -> Result<SamsungToken, CaError> {
    let value: serde_json::Value =
        serde_json::from_str(code_json).map_err(|_| CaError::AuthInvalid)?;
    let text = |key: &str| value.get(key).and_then(|v| v.as_str()).map(str::to_string);
    let access_token = text("access_token").ok_or(CaError::AuthInvalid)?;
    let user_id = text("userId").ok_or(CaError::AuthInvalid)?;
    let printable = |s: &str, max: usize| {
        !s.is_empty() && s.len() <= max && s.chars().all(|c| c.is_ascii_graphic())
    };
    if !printable(&access_token, 4096) || !printable(&user_id, 256) {
        return Err(CaError::AuthInvalid);
    }
    Ok(SamsungToken {
        access_token,
        user_id,
        email: text("inputEmailID"),
    })
}

const DONE_PAGE: &str = "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>EasyTizen</title></head><body style=\"font-family:system-ui,sans-serif;text-align:center;padding:56px 24px;background:#0f1115;color:#f3f4f6\"><h1 style=\"font-size:22px\">&#10003; EasyTizen</h1><p>Вход выполнен. Вернитесь в приложение EasyTizen.</p><p>Signed in. You can return to EasyTizen now.</p></body></html>";
const FAIL_PAGE: &str = "<!doctype html><html><head><meta charset=\"utf-8\"><title>EasyTizen</title></head><body style=\"font-family:system-ui,sans-serif;text-align:center;padding:56px 24px\"><h1 style=\"font-size:22px\">EasyTizen</h1><p>Не удалось получить ответ Samsung. Повторите попытку в приложении.</p><p>The sign-in answer could not be read. Please try again in the app.</p></body></html>";

fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let _ = stream.write_all(
        format!(
            "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n{body}",
            body.len()
        )
        .as_bytes(),
    );
}

/// Reads one small HTTP request: returns `(request target, body)`.
fn read_request(stream: &mut TcpStream) -> Option<(String, String)> {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .ok()?;
    let mut data = Vec::new();
    let mut buffer = [0u8; 4096];
    let header_end = loop {
        if let Some(pos) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
        if data.len() > 64 * 1024 {
            return None;
        }
        match stream.read(&mut buffer) {
            Ok(0) | Err(_) => return None,
            Ok(n) => data.extend_from_slice(&buffer[..n]),
        }
    };
    let head = String::from_utf8_lossy(&data[..header_end]).to_string();
    let target = head.lines().next()?.split_whitespace().nth(1)?.to_string();
    let length: usize = head
        .lines()
        .find_map(|l| {
            let (name, value) = l.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().ok())?
        })
        .unwrap_or(0);
    if length > 64 * 1024 {
        return None;
    }
    while data.len() < header_end + length {
        match stream.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(n) => data.extend_from_slice(&buffer[..n]),
        }
    }
    let body = String::from_utf8_lossy(&data[header_end..]).to_string();
    Some((target, body))
}

impl OAuthSession {
    /// Reserves a callback port. Fails when all 20 ports are busy.
    pub fn start() -> Result<Self, CaError> {
        for port in FIRST_PORT..FIRST_PORT + PORT_COUNT {
            if let Ok(listener) = TcpListener::bind((Ipv4Addr::LOCALHOST, port)) {
                listener
                    .set_nonblocking(true)
                    .map_err(|_| CaError::PortUnavailable)?;
                return Ok(Self { listener, port });
            }
        }
        Err(CaError::PortUnavailable)
    }

    /// The Samsung page to open in the browser.
    pub fn sign_in_url(&self) -> String {
        format!(
            "{SAMSUNG_OAUTH_URL}?serviceID={SERVICE_ID}&actionID=StartOAuth2&accessToken=Y&redirect_uri=http%3A%2F%2Flocalhost%3A{}%2Fsignin%2Fcallback",
            self.port
        )
    }

    /// Waits for Samsung's redirect. Unrelated requests (favicon probes …) get a 404.
    pub fn wait(self, timeout: Duration, cancel: &CancelToken) -> Result<SamsungToken, CaError> {
        let deadline = Instant::now() + timeout;
        loop {
            if cancel.is_cancelled() {
                return Err(CaError::Cancelled);
            }
            if Instant::now() >= deadline {
                return Err(CaError::AuthTimeout);
            }
            match self.listener.accept() {
                Ok((mut stream, peer)) => {
                    if !peer.ip().is_loopback() {
                        continue;
                    }
                    stream.set_nonblocking(false).ok();
                    let Some((target, body)) = read_request(&mut stream) else {
                        continue;
                    };
                    if !target.starts_with(CALLBACK_PATH) {
                        respond(&mut stream, "404 Not Found", "");
                        continue;
                    }
                    let token = extract_auth_code(&body, &target)
                        .ok_or(CaError::AuthInvalid)
                        .and_then(|code| parse_token(&code));
                    match &token {
                        Ok(_) => respond(&mut stream, "200 OK", DONE_PAGE),
                        Err(_) => respond(&mut stream, "400 Bad Request", FAIL_PAGE),
                    }
                    return token;
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(120))
                }
                Err(_) => std::thread::sleep(Duration::from_millis(120)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_extraction_decodes_exactly_once() {
        let body = "state=1&code=%7B%22access_token%22%3A%22a%3Db%2B%2F%3D%22%7D";
        assert_eq!(
            extract_auth_code(body, "/signin/callback").as_deref(),
            Some("{\"access_token\":\"a=b+/=\"}")
        );
        assert_eq!(
            extract_auth_code("", "/signin/callback?x=1&code=abc%20d").as_deref(),
            Some("abc d")
        );
        assert_eq!(extract_auth_code("nothing=1", "/signin/callback"), None);
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz"), "%zz");
    }

    #[test]
    fn token_parsing_and_redaction() {
        let token = parse_token(r#"{"access_token":"tok123","userId":"u9","inputEmailID":"a@b.c","access_token_expires_in":"3600"}"#).unwrap();
        assert_eq!(
            (token.access_token(), token.user_id.as_str()),
            ("tok123", "u9")
        );
        assert!(!format!("{token:?}").contains("tok123"));
        assert_eq!(parse_token("{}").err(), Some(CaError::AuthInvalid));
        assert_eq!(
            parse_token(r#"{"access_token":"a b","userId":"u"}"#).err(),
            Some(CaError::AuthInvalid)
        );
        assert_eq!(parse_token("not json").err(), Some(CaError::AuthInvalid));
    }

    #[test]
    fn callback_round_trip_over_loopback() {
        let session = OAuthSession::start().unwrap();
        let port = session.port;
        assert!(session
            .sign_in_url()
            .contains(&format!("localhost%3A{port}%2Fsignin%2Fcallback")));
        let browser = std::thread::spawn(move || {
            let send = |request: String| {
                let mut s = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
                s.write_all(request.as_bytes()).unwrap();
                let mut answer = String::new();
                let _ = s.read_to_string(&mut answer);
                answer
            };
            // A stray request first, then the real callback.
            assert!(
                send("GET /favicon.ico HTTP/1.1\r\nHost: localhost\r\n\r\n".into())
                    .starts_with("HTTP/1.1 404")
            );
            let body = "code=%7B%22access_token%22%3A%22T0K%22%2C%22userId%22%3A%22user1%22%7D";
            send(format!(
                "POST /signin/callback HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            ))
        });
        let token = session
            .wait(Duration::from_secs(10), &CancelToken::new())
            .unwrap();
        assert_eq!(token.access_token(), "T0K");
        assert_eq!(token.user_id, "user1");
        assert!(browser.join().unwrap().starts_with("HTTP/1.1 200"));
    }

    #[test]
    fn wait_honours_cancel_and_timeout() {
        let cancel = CancelToken::new();
        cancel.cancel();
        assert_eq!(
            OAuthSession::start()
                .unwrap()
                .wait(Duration::from_secs(5), &cancel)
                .err(),
            Some(CaError::Cancelled)
        );
        assert_eq!(
            OAuthSession::start()
                .unwrap()
                .wait(Duration::from_millis(200), &CancelToken::new())
                .err(),
            Some(CaError::AuthTimeout)
        );
    }
}
