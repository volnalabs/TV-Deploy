//! Host-side network pre-checks, so errors can be explained precisely instead of
//! "failed to connect". These probes only open a TCP socket to the address the user chose.

use serde::Serialize;
use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr, TcpStream, UdpSocket};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Probe {
    Open,
    /// Host answered but nothing listens on the port (typical: Developer Mode off, or the
    /// TV rejected this PC's IP).
    Refused,
    /// No answer at all (typical: wrong IP, different network, TV off / in deep standby).
    TimedOut,
    /// No route to the address from this computer.
    NetworkUnreachable,
    Other,
}

pub fn probe_tcp(ip: Ipv4Addr, port: u16, timeout: Duration) -> Probe {
    let addr = SocketAddr::from((ip, port));
    match TcpStream::connect_timeout(&addr, timeout) {
        Ok(_) => Probe::Open,
        Err(e) => match e.kind() {
            ErrorKind::ConnectionRefused => Probe::Refused,
            ErrorKind::TimedOut | ErrorKind::WouldBlock => Probe::TimedOut,
            _ => {
                // `NetworkUnreachable`/`HostUnreachable` are not all stable ErrorKinds on every
                // supported toolchain; match on the debug name to stay portable.
                let k = format!("{:?}", e.kind());
                if k.contains("Unreachable") {
                    Probe::NetworkUnreachable
                } else {
                    Probe::Other
                }
            }
        },
    }
}

/// The local IPv4 address this computer would use to reach `target` — i.e. what must be
/// entered as "Host PC IP" in the TV's Developer Mode dialog. Uses a connected UDP socket;
/// no packet is sent.
pub fn local_ip_towards(target: Ipv4Addr) -> Option<Ipv4Addr> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect(SocketAddr::from((target, 9))).ok()?;
    match s.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(v4) if !v4.is_unspecified() => Some(v4),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn open_and_refused_on_loopback() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        assert_eq!(
            probe_tcp(Ipv4Addr::LOCALHOST, port, Duration::from_secs(1)),
            Probe::Open
        );
        drop(l);
        assert_eq!(
            probe_tcp(Ipv4Addr::LOCALHOST, port, Duration::from_secs(1)),
            Probe::Refused
        );
    }

    #[test]
    fn local_ip_for_loopback_is_loopback() {
        assert_eq!(
            local_ip_towards(Ipv4Addr::LOCALHOST),
            Some(Ipv4Addr::LOCALHOST)
        );
    }
}
