//! A small, synchronous SDB client: one TCP connection, one stream at a time.
//!
//! This is what lets EasyTizen talk to a Samsung TV without the Tizen SDK — in particular on
//! Android, where the official `sdb` binary does not exist. It implements only what an
//! installer needs:
//!
//! * connection handshake (`CNXN` / `host::`),
//! * `capability:` — device properties (secure protocol, SDK tool path, model …),
//! * `shell:<command>` — run one command and collect its output,
//! * `sync:` `SEND`/`DATA`/`DONE` — push one file.
//!
//! Every read honours a deadline and a [`CancelToken`], so a silent TV can never hang the UI.

use super::protocol::*;
use crate::cancel::CancelToken;
use std::collections::BTreeMap;
use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpStream};
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SdbError {
    #[error("the TV did not answer in time")]
    Timeout,
    #[error("the TV refused the connection")]
    Refused,
    #[error("no route to the TV")]
    Unreachable,
    #[error("network error: {0}")]
    Io(String),
    #[error("unexpected answer from the device: {0}")]
    Protocol(String),
    #[error("this is an Android (ADB) device, not a Samsung TV")]
    NotTizen,
    #[error("the TV rejected the request: {0}")]
    ServiceRefused(String),
    #[error("file transfer failed: {0}")]
    SyncFailed(String),
    #[error("cancelled")]
    Cancelled,
}

impl From<std::io::Error> for SdbError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            ErrorKind::TimedOut | ErrorKind::WouldBlock => SdbError::Timeout,
            ErrorKind::ConnectionRefused => SdbError::Refused,
            _ => {
                let kind = format!("{:?}", e.kind());
                if kind.contains("Unreachable") {
                    SdbError::Unreachable
                } else {
                    SdbError::Io(kind)
                }
            }
        }
    }
}

/// Parsed `capability:` answer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Capability {
    pub raw: BTreeMap<String, String>,
}

impl Capability {
    /// Tolerant parser: the payload starts with a binary length prefix and then lists
    /// `key:value` lines.
    pub fn parse(bytes: &[u8]) -> Self {
        let text = String::from_utf8_lossy(bytes);
        let mut raw = BTreeMap::new();
        for line in text.split(['\n', '\r']) {
            let Some(colon) = line.find(':') else {
                continue;
            };
            let key: String = line[..colon]
                .chars()
                .rev()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            let value = line[colon + 1..].trim();
            if !key.is_empty() && value.chars().all(|c| !c.is_control()) {
                raw.insert(key.to_ascii_lowercase(), value.to_string());
            }
        }
        Self { raw }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.raw.get(key).map(String::as_str)
    }

    pub fn secure_protocol(&self) -> bool {
        matches!(
            self.get("secure_protocol")
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("enabled" | "true" | "yes" | "1")
        )
    }

    /// Directory the TV wants packages uploaded to (`<sdk_toolpath>/tmp`).
    pub fn package_tmp_dir(&self) -> Option<String> {
        let base = self.get("sdk_toolpath")?.trim_end_matches('/');
        let safe = base.starts_with('/')
            && base
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "/_-.".contains(c))
            && !base.split('/').any(|part| part == "..");
        safe.then(|| format!("{base}/tmp"))
    }
}

pub struct SdbClient {
    stream: TcpStream,
    /// Largest WRTE payload the device accepts.
    max_payload: usize,
    next_local_id: u32,
    pub banner: String,
    cancel: CancelToken,
}

struct OpenStream {
    local: u32,
    remote: u32,
}

const POLL: Duration = Duration::from_millis(150);

impl SdbClient {
    pub fn connect(
        ip: Ipv4Addr,
        port: u16,
        timeout: Duration,
        cancel: &CancelToken,
    ) -> Result<Self, SdbError> {
        if cancel.is_cancelled() {
            return Err(SdbError::Cancelled);
        }
        let stream = TcpStream::connect_timeout(&SocketAddr::from((ip, port)), timeout)?;
        stream.set_nodelay(true).ok();
        stream.set_read_timeout(Some(POLL))?;
        stream.set_write_timeout(Some(timeout.max(Duration::from_secs(5))))?;
        let mut client = Self {
            stream,
            max_payload: ANNOUNCED_MAX_PAYLOAD as usize,
            next_local_id: 1,
            banner: String::new(),
            cancel: cancel.clone(),
        };
        client.send(&Message::new(
            A_CNXN,
            SDB_VERSION,
            ANNOUNCED_MAX_PAYLOAD,
            b"host::\0".to_vec(),
        ))?;
        let deadline = Instant::now() + timeout;
        loop {
            let msg = client.recv(deadline)?;
            match msg.command {
                A_CNXN => {
                    client.max_payload = (msg.arg1 as usize).clamp(1024, SEND_PAYLOAD_CAP);
                    client.banner = String::from_utf8_lossy(&msg.data)
                        .trim_end_matches('\0')
                        .to_string();
                    return Ok(client);
                }
                A_AUTH => return Err(SdbError::NotTizen),
                // Status frames some firmwares send before the banner are ignored.
                _ => continue,
            }
        }
    }

    fn send(&mut self, msg: &Message) -> Result<(), SdbError> {
        self.stream.write_all(&msg.encode())?;
        Ok(())
    }

    fn read_exact(&mut self, buf: &mut [u8], deadline: Instant) -> Result<(), SdbError> {
        let mut filled = 0;
        while filled < buf.len() {
            if self.cancel.is_cancelled() {
                return Err(SdbError::Cancelled);
            }
            match self.stream.read(&mut buf[filled..]) {
                Ok(0) => return Err(SdbError::Io("connection closed by the TV".into())),
                Ok(n) => filled += n,
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                    if Instant::now() >= deadline {
                        return Err(SdbError::Timeout);
                    }
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }

    fn recv(&mut self, deadline: Instant) -> Result<Message, SdbError> {
        let mut raw = [0u8; HEADER_LEN];
        self.read_exact(&mut raw, deadline)?;
        let header = decode_header(&raw).map_err(|e| SdbError::Protocol(format!("{e:?}")))?;
        let mut data = vec![0u8; header.data_length as usize];
        self.read_exact(&mut data, deadline)?;
        Ok(Message {
            command: header.command,
            arg0: header.arg0,
            arg1: header.arg1,
            data,
        })
    }

    fn open(&mut self, service: &str, deadline: Instant) -> Result<OpenStream, SdbError> {
        let local = self.next_local_id;
        self.next_local_id += 1;
        let mut name = service.as_bytes().to_vec();
        name.push(0);
        if name.len() > self.max_payload {
            return Err(SdbError::Protocol("service name is too long".into()));
        }
        self.send(&Message::new(A_OPEN, local, 0, name))?;
        loop {
            let msg = self.recv(deadline)?;
            match msg.command {
                A_OKAY if msg.arg1 == local => {
                    return Ok(OpenStream {
                        local,
                        remote: msg.arg0,
                    })
                }
                A_CLSE if msg.arg1 == local => {
                    let service_name = service.split(':').next().unwrap_or(service);
                    return Err(SdbError::ServiceRefused(service_name.to_string()));
                }
                _ => continue, // stale frames of an earlier stream
            }
        }
    }

    /// Reads a stream until the device closes it; returns everything it wrote (capped).
    fn read_to_close(
        &mut self,
        s: &OpenStream,
        deadline: Instant,
        cap: usize,
    ) -> Result<Vec<u8>, SdbError> {
        let mut out = Vec::new();
        loop {
            let msg = self.recv(deadline)?;
            match msg.command {
                A_WRTE if msg.arg1 == s.local => {
                    if out.len() < cap {
                        let take = msg.data.len().min(cap - out.len());
                        out.extend_from_slice(&msg.data[..take]);
                    }
                    self.send(&Message::new(A_OKAY, s.local, s.remote, vec![]))?;
                }
                A_CLSE if msg.arg1 == s.local => {
                    self.send(&Message::new(A_CLSE, s.local, s.remote, vec![]))
                        .ok();
                    return Ok(out);
                }
                _ => continue,
            }
        }
    }

    /// Runs one command through the TV's `shell:` service and returns its output.
    ///
    /// The command is interpreted by the *device*. Callers must build it from validated
    /// values only (see [`super::is_safe_shell_word`]).
    pub fn shell(&mut self, command: &str, timeout: Duration) -> Result<String, SdbError> {
        if command.contains(['\0', '\n', '\r']) {
            return Err(SdbError::Protocol("unsafe shell command".into()));
        }
        let deadline = Instant::now() + timeout;
        let s = self.open(&format!("shell:{command}"), deadline)?;
        let out = self.read_to_close(&s, deadline, 512 * 1024)?;
        Ok(String::from_utf8_lossy(&out).replace('\r', ""))
    }

    pub fn capability(&mut self, timeout: Duration) -> Result<Capability, SdbError> {
        let deadline = Instant::now() + timeout;
        let s = self.open("capability:", deadline)?;
        let out = self.read_to_close(&s, deadline, 64 * 1024)?;
        Ok(Capability::parse(&out))
    }

    /// Sends bytes on a stream, split into WRTE frames, waiting for the device's OKAY after
    /// each one (the protocol's flow control). Data the device writes meanwhile is collected.
    fn write_stream(
        &mut self,
        s: &OpenStream,
        mut data: &[u8],
        incoming: &mut Vec<u8>,
        deadline: Instant,
    ) -> Result<(), SdbError> {
        while !data.is_empty() {
            let n = data.len().min(self.max_payload);
            self.send(&Message::new(A_WRTE, s.local, s.remote, data[..n].to_vec()))?;
            data = &data[n..];
            loop {
                let msg = self.recv(deadline)?;
                match msg.command {
                    A_OKAY if msg.arg1 == s.local => break,
                    A_WRTE if msg.arg1 == s.local => {
                        incoming.extend_from_slice(&msg.data);
                        self.send(&Message::new(A_OKAY, s.local, s.remote, vec![]))?;
                    }
                    A_CLSE if msg.arg1 == s.local => {
                        return Err(SdbError::SyncFailed(sync_failure_text(incoming)));
                    }
                    _ => continue,
                }
            }
        }
        Ok(())
    }

    /// Uploads `size` bytes from `source` to `remote_path` on the TV (`sdb push`).
    /// `progress` receives the number of bytes sent so far.
    pub fn push(
        &mut self,
        source: &mut dyn Read,
        size: u64,
        remote_path: &str,
        idle_timeout: Duration,
        progress: &mut dyn FnMut(u64),
    ) -> Result<(), SdbError> {
        if !remote_path.starts_with('/') || remote_path.contains([',', '\0', '\n']) {
            return Err(SdbError::Protocol("unsafe remote path".into()));
        }
        let mut deadline = Instant::now() + idle_timeout;
        let s = self.open("sync:", deadline)?;
        let mut incoming = Vec::new();

        // 0644 regular file, as the official client sends for packages.
        let target = format!("{remote_path},{}", 0o100644);
        let mut head = b"SEND".to_vec();
        head.extend_from_slice(&(target.len() as u32).to_le_bytes());
        head.extend_from_slice(target.as_bytes());
        self.write_stream(&s, &head, &mut incoming, deadline)?;

        let mut sent = 0u64;
        let mut chunk = vec![0u8; 32 * 1024];
        loop {
            let n = source.read(&mut chunk).map_err(|e| {
                SdbError::SyncFailed(format!("cannot read the package: {:?}", e.kind()))
            })?;
            if n == 0 {
                break;
            }
            let mut frame = b"DATA".to_vec();
            frame.extend_from_slice(&(n as u32).to_le_bytes());
            frame.extend_from_slice(&chunk[..n]);
            deadline = Instant::now() + idle_timeout;
            self.write_stream(&s, &frame, &mut incoming, deadline)?;
            sent += n as u64;
            progress(sent);
        }
        if sent != size {
            return Err(SdbError::SyncFailed(
                "the package changed while it was being sent".into(),
            ));
        }

        let mtime = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as u32)
            .unwrap_or(0);
        let mut done = b"DONE".to_vec();
        done.extend_from_slice(&mtime.to_le_bytes());
        deadline = Instant::now() + idle_timeout;
        self.write_stream(&s, &done, &mut incoming, deadline)?;

        // Wait for the 8-byte status: "OKAY" + 0, or "FAIL" + length + message.
        while incoming.len() < 8 {
            let msg = self.recv(deadline)?;
            match msg.command {
                A_WRTE if msg.arg1 == s.local => {
                    incoming.extend_from_slice(&msg.data);
                    self.send(&Message::new(A_OKAY, s.local, s.remote, vec![]))?;
                }
                A_CLSE if msg.arg1 == s.local => break,
                _ => continue,
            }
        }
        let ok = incoming.starts_with(b"OKAY");
        if ok {
            let mut quit = b"QUIT".to_vec();
            quit.extend_from_slice(&0u32.to_le_bytes());
            self.write_stream(&s, &quit, &mut incoming, deadline).ok();
        }
        self.send(&Message::new(A_CLSE, s.local, s.remote, vec![]))
            .ok();
        if ok {
            Ok(())
        } else {
            Err(SdbError::SyncFailed(sync_failure_text(&incoming)))
        }
    }
}

impl Drop for SdbClient {
    fn drop(&mut self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

fn sync_failure_text(incoming: &[u8]) -> String {
    if incoming.len() >= 8 && incoming.starts_with(b"FAIL") {
        let text = String::from_utf8_lossy(&incoming[8..]);
        let text: String = text.chars().filter(|c| !c.is_control()).take(300).collect();
        if !text.is_empty() {
            return text;
        }
    }
    "the TV closed the transfer".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_parser_handles_length_prefix_and_real_shape() {
        let mut payload = vec![0xbc, 0x02];
        payload.extend_from_slice(
            b"secure_protocol:enabled\nintershell_support:disabled\nsdk_toolpath:/home/owner/share/tmp/sdk_tools\nprofile_name:tv\nplatform_version:6.5\nproduct_version:5.0\ndevice_name:Tizen\n",
        );
        let cap = Capability::parse(&payload);
        assert!(cap.secure_protocol());
        assert_eq!(cap.get("product_version"), Some("5.0"));
        assert_eq!(
            cap.package_tmp_dir().as_deref(),
            Some("/home/owner/share/tmp/sdk_tools/tmp")
        );
    }

    #[test]
    fn unsafe_tool_paths_are_not_used() {
        for bad in [
            "relative/path",
            "/tmp/../etc",
            "/tmp/a b",
            "/tmp/$(reboot)",
            "/tmp/a;b",
        ] {
            let cap = Capability::parse(format!("sdk_toolpath:{bad}\n").as_bytes());
            assert_eq!(cap.package_tmp_dir(), None, "{bad}");
        }
    }

    #[test]
    fn connect_reports_refused_and_cancelled() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let err = SdbClient::connect(
            Ipv4Addr::LOCALHOST,
            port,
            Duration::from_secs(3),
            &CancelToken::new(),
        )
        .err()
        .unwrap();
        assert_eq!(err, SdbError::Refused);
        let cancelled = CancelToken::new();
        cancelled.cancel();
        assert_eq!(
            SdbClient::connect(
                Ipv4Addr::LOCALHOST,
                port,
                Duration::from_secs(1),
                &cancelled
            )
            .err(),
            Some(SdbError::Cancelled)
        );
    }
}
