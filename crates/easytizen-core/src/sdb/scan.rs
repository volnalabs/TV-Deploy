//! Finding TVs on the local network: a TCP sweep of one /24 for the SDB port, followed by a
//! real SDB handshake so that unrelated services on the same port are not listed.

use super::client::SdbClient;
use crate::cancel::CancelToken;
use serde::Serialize;
use std::net::{Ipv4Addr, SocketAddr, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FoundDevice {
    pub ip: String,
    pub port: u16,
    pub name: Option<String>,
}

/// The IPv4 address of the interface used for the default route (the Wi-Fi address on a
/// phone). A connected UDP socket is used; no packet is sent.
pub fn primary_local_ip() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:9").ok()?;
    match socket.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(ip) if !ip.is_unspecified() && !ip.is_loopback() => Some(ip),
        _ => None,
    }
}

pub fn subnet_prefix_of(ip: Ipv4Addr) -> String {
    let o = ip.octets();
    format!("{}.{}.{}", o[0], o[1], o[2])
}

/// Sweeps `<prefix>.1 … <prefix>.254`. `on_progress` receives the fraction scanned.
pub fn scan_subnet(
    prefix: &str,
    port: u16,
    connect_timeout: Duration,
    cancel: &CancelToken,
    on_progress: &(dyn Fn(f32) + Sync),
) -> Vec<FoundDevice> {
    let octets: Vec<u8> = prefix.split('.').filter_map(|p| p.parse().ok()).collect();
    if octets.len() != 3 {
        return vec![];
    }
    let next = AtomicUsize::new(1);
    let done = AtomicUsize::new(0);
    let open: Mutex<Vec<Ipv4Addr>> = Mutex::new(Vec::new());
    thread::scope(|scope| {
        for _ in 0..48 {
            scope.spawn(|| loop {
                let host = next.fetch_add(1, Ordering::SeqCst);
                if host > 254 || cancel.is_cancelled() {
                    break;
                }
                let ip = Ipv4Addr::new(octets[0], octets[1], octets[2], host as u8);
                if TcpStream::connect_timeout(&SocketAddr::from((ip, port)), connect_timeout)
                    .is_ok()
                {
                    open.lock().unwrap().push(ip);
                }
                let finished = done.fetch_add(1, Ordering::SeqCst) + 1;
                on_progress(finished as f32 / 254.0);
            });
        }
    });
    let mut candidates = open.into_inner().unwrap();
    candidates.sort();

    let found = Arc::new(Mutex::new(Vec::new()));
    thread::scope(|scope| {
        for ip in candidates {
            let found = found.clone();
            scope.spawn(move || {
                if cancel.is_cancelled() {
                    return;
                }
                let Ok(mut client) = SdbClient::connect(ip, port, Duration::from_secs(3), cancel)
                else {
                    return;
                };
                let name = client
                    .capability(Duration::from_secs(4))
                    .ok()
                    .and_then(|cap| cap.get("device_name").map(str::to_string))
                    .filter(|n| !n.is_empty());
                found.lock().unwrap().push(FoundDevice {
                    ip: ip.to_string(),
                    port,
                    name,
                });
            });
        }
    });
    let mut list = Arc::try_unwrap(found)
        .map(|m| m.into_inner().unwrap())
        .unwrap_or_default();
    list.sort_by_key(|d: &FoundDevice| d.ip.parse::<Ipv4Addr>().map(u32::from).unwrap_or(0));
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix() {
        assert_eq!(
            subnet_prefix_of(Ipv4Addr::new(192, 168, 7, 23)),
            "192.168.7"
        );
    }

    #[test]
    fn bad_prefix_and_cancel_return_nothing() {
        let cancel = CancelToken::new();
        assert!(scan_subnet("nope", 26101, Duration::from_millis(50), &cancel, &|_| {}).is_empty());
        cancel.cancel();
        assert!(scan_subnet("127.0.0", 9, Duration::from_millis(50), &cancel, &|_| {}).is_empty());
    }
}
