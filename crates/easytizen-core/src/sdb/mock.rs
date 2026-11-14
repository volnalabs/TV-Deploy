//! A protocol-level emulator of a Samsung TV's `sdbd`, for automated tests and for trying the
//! app without a television (`cargo run -p easytizen-core --example mock_tv --features mock-tv`).
//!
//! It answers the same frames and device commands the real engine sends and applies the two
//! rules a real TV enforces on a package: both signature files must be present, and the
//! distributor certificate must name this TV's DUID. It is an emulator written from the
//! protocol, NOT a substitute for testing on physical hardware.

use super::protocol::*;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Debug, Default)]
pub struct MockState {
    pub files: HashMap<String, Vec<u8>>,
    pub installed: Vec<String>,
    pub launched: Vec<String>,
    pub shell_commands: Vec<String>,
}

#[derive(Clone)]
pub struct MockTv {
    pub port: u16,
    pub duid: String,
    pub state: Arc<Mutex<MockState>>,
}

fn capability_text() -> String {
    [
        "secure_protocol:enabled",
        "intershell_support:disabled",
        "filesync_support:pushpull",
        "sockproto_support:enabled",
        "rootonoff_support:disabled",
        "encryption_support:disabled",
        "cpu_arch:armv7",
        "sdk_toolpath:/home/owner/share/tmp/sdk_tools",
        "profile_name:tv",
        "vendor_name:Samsung",
        "can_launch:tv-samsung",
        "device_name:Tizen TV (emulated)",
        "platform_version:8.0",
        "product_version:5.0",
        "sdbd_version:2.2.31",
    ]
    .join("\n")
        + "\n"
}

impl MockTv {
    /// Starts the emulator on `bind_ip:port` (port 0 picks a free one).
    pub fn start(bind_ip: Ipv4Addr, port: u16, duid: &str) -> std::io::Result<Self> {
        let listener = TcpListener::bind((bind_ip, port))?;
        let tv = MockTv {
            port: listener.local_addr()?.port(),
            duid: duid.to_string(),
            state: Arc::new(Mutex::new(MockState::default())),
        };
        let accept = tv.clone();
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let tv = accept.clone();
                thread::spawn(move || {
                    let _ = tv.serve(stream);
                });
            }
        });
        Ok(tv)
    }

    fn read_message(stream: &mut TcpStream) -> std::io::Result<Message> {
        let mut raw = [0u8; HEADER_LEN];
        stream.read_exact(&mut raw)?;
        let header = decode_header(&raw)
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "bad header"))?;
        let mut data = vec![0u8; header.data_length as usize];
        stream.read_exact(&mut data)?;
        if checksum(&data) != header.data_check {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "bad checksum",
            ));
        }
        Ok(Message {
            command: header.command,
            arg0: header.arg0,
            arg1: header.arg1,
            data,
        })
    }

    /// Writes device → host data and waits for the host's OKAY, like a real sdbd.
    fn write_data(stream: &mut TcpStream, dev: u32, host: u32, data: &[u8]) -> std::io::Result<()> {
        for chunk in data.chunks(ANNOUNCED_MAX_PAYLOAD as usize) {
            stream.write_all(&Message::new(A_WRTE, dev, host, chunk.to_vec()).encode())?;
            loop {
                let reply = Self::read_message(stream)?;
                if reply.command == A_OKAY {
                    break;
                }
            }
        }
        Ok(())
    }

    fn serve(&self, mut stream: TcpStream) -> std::io::Result<()> {
        let hello = Self::read_message(&mut stream)?;
        if hello.command != A_CNXN || !hello.data.starts_with(b"host::") {
            return Ok(());
        }
        stream.write_all(
            &Message::new(
                A_CNXN,
                SDB_VERSION,
                ANNOUNCED_MAX_PAYLOAD,
                b"device::mock-tv\0".to_vec(),
            )
            .encode(),
        )?;
        let mut next_id = 100u32;
        loop {
            let msg = Self::read_message(&mut stream)?;
            if msg.command != A_OPEN {
                continue;
            }
            let host = msg.arg0;
            let dev = next_id;
            next_id += 1;
            let service = String::from_utf8_lossy(&msg.data)
                .trim_end_matches('\0')
                .to_string();
            if service == "capability:" {
                stream.write_all(&Message::new(A_OKAY, dev, host, vec![]).encode())?;
                let text = capability_text();
                let mut payload = (text.len() as u16).to_le_bytes().to_vec();
                payload.extend_from_slice(text.as_bytes());
                Self::write_data(&mut stream, dev, host, &payload)?;
                stream.write_all(&Message::new(A_CLSE, dev, host, vec![]).encode())?;
            } else if let Some(command) = service.strip_prefix("shell:") {
                stream.write_all(&Message::new(A_OKAY, dev, host, vec![]).encode())?;
                let out = self.run_shell(command);
                if !out.is_empty() {
                    Self::write_data(&mut stream, dev, host, out.as_bytes())?;
                }
                stream.write_all(&Message::new(A_CLSE, dev, host, vec![]).encode())?;
            } else if service == "sync:" {
                stream.write_all(&Message::new(A_OKAY, dev, host, vec![]).encode())?;
                self.serve_sync(&mut stream, dev, host)?;
            } else {
                // Unknown service: refuse, as sdbd does.
                stream.write_all(&Message::new(A_CLSE, 0, host, vec![]).encode())?;
            }
        }
    }

    fn serve_sync(&self, stream: &mut TcpStream, dev: u32, host: u32) -> std::io::Result<()> {
        let mut buffer: Vec<u8> = Vec::new();
        let mut path: Option<String> = None;
        let mut file: Vec<u8> = Vec::new();
        loop {
            // Consume every complete sync request already buffered.
            loop {
                if buffer.len() < 8 {
                    break;
                }
                let id = [buffer[0], buffer[1], buffer[2], buffer[3]];
                let arg = u32::from_le_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]) as usize;
                match &id {
                    b"SEND" | b"DATA" => {
                        if buffer.len() < 8 + arg {
                            break;
                        }
                        let body: Vec<u8> = buffer.drain(..8 + arg).skip(8).collect();
                        if &id == b"SEND" {
                            let spec = String::from_utf8_lossy(&body).to_string();
                            path = Some(
                                spec.rsplit_once(',')
                                    .map(|(p, _)| p.to_string())
                                    .unwrap_or(spec),
                            );
                            file.clear();
                        } else {
                            file.extend_from_slice(&body);
                        }
                    }
                    b"DONE" => {
                        buffer.drain(..8);
                        let reply = match path.take() {
                            Some(p) if p.starts_with("/home/owner/share/tmp/sdk_tools/tmp/") => {
                                self.state
                                    .lock()
                                    .unwrap()
                                    .files
                                    .insert(p, std::mem::take(&mut file));
                                let mut ok = b"OKAY".to_vec();
                                ok.extend_from_slice(&0u32.to_le_bytes());
                                ok
                            }
                            _ => {
                                let text = b"permission denied";
                                let mut fail = b"FAIL".to_vec();
                                fail.extend_from_slice(&(text.len() as u32).to_le_bytes());
                                fail.extend_from_slice(text);
                                fail
                            }
                        };
                        Self::write_data(stream, dev, host, &reply)?;
                    }
                    b"QUIT" => {
                        stream.write_all(&Message::new(A_CLSE, dev, host, vec![]).encode())?;
                        return Ok(());
                    }
                    _ => {
                        stream.write_all(&Message::new(A_CLSE, dev, host, vec![]).encode())?;
                        return Ok(());
                    }
                }
            }
            let msg = Self::read_message(stream)?;
            match msg.command {
                A_WRTE => {
                    buffer.extend_from_slice(&msg.data);
                    stream.write_all(&Message::new(A_OKAY, dev, host, vec![]).encode())?;
                }
                A_CLSE => return Ok(()),
                _ => {}
            }
        }
    }

    fn run_shell(&self, command: &str) -> String {
        let mut state = self.state.lock().unwrap();
        state.shell_commands.push(command.to_string());
        let words: Vec<&str> = command.split_whitespace().collect();
        match words.as_slice() {
            ["0", "getduid"] => format!("{}\n", self.duid),
            ["0", "vd_appinstall", app_id, path] => {
                let Some(bytes) = state.files.get(*path).cloned() else {
                    return format!("install {app_id}\npackage_path {path}\napp_id[{app_id}] install failed[118, -3], reason: package not found\ncmd_ret:1\n");
                };
                match check_package(&bytes, &self.duid) {
                    Ok(()) => {
                        state.installed.push(app_id.to_string());
                        format!("install {app_id}\npackage_path {path}\napp in-house install\napp_id[{app_id}] install start\napp_id[{app_id}] install completed\nspend time for wascmd is [1200]ms\ncmd_ret:0\n")
                    }
                    Err(reason) => format!("install {app_id}\npackage_path {path}\napp_id[{app_id}] install failed[118, -12], reason: Check certificate error : :{reason}\ncmd_ret:1\n"),
                }
            }
            ["0", "vd_appuninstall", app_id] => {
                state.installed.retain(|a| a != app_id);
                format!("uninstall {app_id}\napp_id[{app_id}] uninstall completed\ncmd_ret:0\n")
            }
            ["0", "was_execute", app_id] => {
                if state.installed.iter().any(|a| a == app_id) {
                    state.launched.push(app_id.to_string());
                    format!("app_id[{app_id}] launched\n")
                } else {
                    format!("app_id[{app_id}] launch failed\n")
                }
            }
            ["0", "rmfile", path] => {
                state.files.remove(path.trim_matches('"'));
                String::new()
            }
            _ => "closed\n".to_string(),
        }
    }
}

/// The emulated TV's acceptance rule: a ZIP with both signature documents whose distributor
/// certificate carries `URN:tizen:deviceid=<this TV's DUID>`.
fn check_package(bytes: &[u8], duid: &str) -> Result<(), String> {
    use base64::Engine;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|_| "package is not a valid archive".to_string())?;
    if zip.by_name("author-signature.xml").is_err() {
        return Err("author-signature.xml is missing".into());
    }
    let mut xml = String::new();
    zip.by_name("signature1.xml")
        .map_err(|_| "signature1.xml is missing".to_string())?
        .read_to_string(&mut xml)
        .map_err(|_| "signature1.xml is unreadable".to_string())?;
    let needle = format!("URN:tizen:deviceid={duid}");
    let mut rest = xml.as_str();
    while let Some(start) = rest.find("<X509Certificate>") {
        let body = &rest[start + 17..];
        let Some(end) = body.find("</X509Certificate>") else {
            break;
        };
        let b64: String = body[..end].chars().filter(|c| !c.is_whitespace()).collect();
        if let Ok(der) = base64::engine::general_purpose::STANDARD.decode(b64) {
            if der.windows(needle.len()).any(|w| w == needle.as_bytes()) {
                return Ok(());
            }
        }
        rest = &body[end..];
    }
    Err(
        "Invalid certificate chain with certificate in signature (DUID does not match this TV)"
            .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cancel::CancelToken;
    use crate::sdb::{
        install_wgt, query_tv, uninstall_app, InstallFailure, InstallStage, InstallVerdict,
        SdbClient, SdbError,
    };
    use std::time::Duration;

    const DUID: &str = "MOCKDUID1234ABCD";

    fn tv() -> MockTv {
        MockTv::start(Ipv4Addr::LOCALHOST, 0, DUID).unwrap()
    }

    #[test]
    fn handshake_capability_and_duid() {
        let tv = tv();
        let id = query_tv(
            Ipv4Addr::LOCALHOST,
            tv.port,
            Duration::from_secs(5),
            &CancelToken::new(),
        )
        .unwrap();
        assert_eq!(id.duid.as_deref(), Some(DUID));
        assert_eq!(id.model.as_deref(), Some("Tizen TV (emulated)"));
        assert!(id.secure_protocol);
        assert_eq!(tv.state.lock().unwrap().shell_commands, ["0 getduid"]);
    }

    #[test]
    fn push_transfers_large_files_byte_for_byte() {
        let tv = tv();
        let data: Vec<u8> = (0..300_000u32).map(|i| (i * 31 % 251) as u8).collect();
        let mut client = SdbClient::connect(
            Ipv4Addr::LOCALHOST,
            tv.port,
            Duration::from_secs(5),
            &CancelToken::new(),
        )
        .unwrap();
        let mut last = 0;
        client
            .push(
                &mut data.as_slice(),
                data.len() as u64,
                "/home/owner/share/tmp/sdk_tools/tmp/a.wgt",
                Duration::from_secs(10),
                &mut |sent| last = sent,
            )
            .unwrap();
        assert_eq!(last, data.len() as u64);
        assert_eq!(
            tv.state.lock().unwrap().files["/home/owner/share/tmp/sdk_tools/tmp/a.wgt"],
            data
        );
        // A refused destination surfaces the device's own reason.
        let err = client
            .push(
                &mut &b"x"[..],
                1,
                "/etc/passwd",
                Duration::from_secs(10),
                &mut |_| {},
            )
            .unwrap_err();
        assert_eq!(err, SdbError::SyncFailed("permission denied".into()));
    }

    #[test]
    fn unsigned_package_is_rejected_and_cleaned_up() {
        let tv = tv();
        let mut wgt = Vec::new();
        {
            use zip::write::SimpleFileOptions;
            let mut w = zip::ZipWriter::new(std::io::Cursor::new(&mut wgt));
            w.start_file("config.xml", SimpleFileOptions::default())
                .unwrap();
            w.write_all(b"<widget/>").unwrap();
            w.finish().unwrap();
        }
        let mut stages = Vec::new();
        let outcome = install_wgt(
            Ipv4Addr::LOCALHOST,
            tv.port,
            &mut wgt.as_slice(),
            wgt.len() as u64,
            "abcde12345.Demo",
            true,
            Duration::from_secs(5),
            &CancelToken::new(),
            &mut |stage, _| {
                if stages.last() != Some(&stage) {
                    stages.push(stage)
                }
            },
        )
        .unwrap();
        assert_eq!(
            outcome.verdict,
            InstallVerdict::Failed(InstallFailure::CertificateRejected)
        );
        assert_eq!(outcome.launched, None);
        assert_eq!(
            stages,
            [
                InstallStage::Connecting,
                InstallStage::Uploading,
                InstallStage::Installing
            ]
        );
        let state = tv.state.lock().unwrap();
        assert!(state.files.is_empty(), "uploaded copy must be removed");
        assert!(state.installed.is_empty());
    }

    #[test]
    fn unsafe_ids_never_reach_the_device() {
        let tv = tv();
        let err = uninstall_app(
            Ipv4Addr::LOCALHOST,
            tv.port,
            "a; reboot",
            Duration::from_secs(5),
            &CancelToken::new(),
        )
        .unwrap_err();
        assert!(matches!(err, SdbError::Protocol(_)));
        assert!(tv.state.lock().unwrap().shell_commands.is_empty());
    }

    #[test]
    fn android_adb_endpoints_are_recognised_as_not_tizen() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            if let Ok((mut s, _)) = listener.accept() {
                let _ = MockTv::read_message(&mut s);
                let _ = s.write_all(&Message::new(A_AUTH, 1, 0, vec![0u8; 20]).encode());
                thread::sleep(Duration::from_millis(300));
            }
        });
        let err = SdbClient::connect(
            Ipv4Addr::LOCALHOST,
            port,
            Duration::from_secs(5),
            &CancelToken::new(),
        )
        .err()
        .unwrap();
        assert_eq!(err, SdbError::NotTizen);
    }
}
