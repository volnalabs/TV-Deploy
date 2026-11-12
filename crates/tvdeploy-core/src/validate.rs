//! Strict validation of everything that ends up as a CLI argument.

use std::net::{IpAddr, Ipv4Addr};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("invalid IPv4 address")]
    BadIp,
    #[error("this address cannot be a TV (loopback, multicast, broadcast or unspecified)")]
    ForbiddenIp,
    #[error("port must be between 1 and 65535")]
    BadPort,
    #[error("invalid subnet prefix (expected three octets like 192.168.1)")]
    BadSubnet,
    #[error("invalid device serial (expected ip:port)")]
    BadSerial,
    #[error("invalid file path")]
    BadPath,
    #[error("timeout out of range")]
    BadTimeout,
}

pub const DEFAULT_SDB_PORT: u16 = 26101;

/// Parses a dotted-quad IPv4 address. Rejects anything that is not exactly four decimal
/// octets (no hostnames, no leading option-like text, no whitespace) and addresses that can
/// never be a TV.
pub fn parse_tv_ip(input: &str) -> Result<Ipv4Addr, ValidationError> {
    let s = input.trim();
    if s.len() > 15 || !s.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return Err(ValidationError::BadIp);
    }
    let ip: Ipv4Addr = s.parse().map_err(|_| ValidationError::BadIp)?;
    if ip.is_loopback() || ip.is_multicast() || ip.is_broadcast() || ip.is_unspecified() {
        return Err(ValidationError::ForbiddenIp);
    }
    Ok(ip)
}

pub fn parse_port(input: u32) -> Result<u16, ValidationError> {
    u16::try_from(input)
        .ok()
        .filter(|p| *p != 0)
        .ok_or(ValidationError::BadPort)
}

/// "192.168.1" -> ok. Used for the optional restricted LAN scan.
pub fn parse_subnet_prefix(input: &str) -> Result<String, ValidationError> {
    let s = input.trim();
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 3 || parts.iter().any(|p| p.is_empty() || p.len() > 3 || !p.bytes().all(|b| b.is_ascii_digit())) {
        return Err(ValidationError::BadSubnet);
    }
    let probe = format!("{s}.1");
    let ip: Ipv4Addr = probe.parse().map_err(|_| ValidationError::BadSubnet)?;
    if ip.is_loopback() || ip.is_multicast() {
        return Err(ValidationError::BadSubnet);
    }
    Ok(s.to_string())
}

/// "ip:port" as used by `sdb devices`.
pub fn parse_serial(input: &str) -> Result<(Ipv4Addr, u16), ValidationError> {
    let (ip, port) = input.split_once(':').ok_or(ValidationError::BadSerial)?;
    let ip = parse_tv_ip(ip).map_err(|_| ValidationError::BadSerial)?;
    let port: u32 = port.parse().map_err(|_| ValidationError::BadSerial)?;
    Ok((ip, parse_port(port).map_err(|_| ValidationError::BadSerial)?))
}

pub fn clamp_timeout_ms(ms: u64) -> Result<u64, ValidationError> {
    if (100..=30_000).contains(&ms) {
        Ok(ms)
    } else {
        Err(ValidationError::BadTimeout)
    }
}

/// Paths passed to the Samsung CLI must be absolute, free of NUL/newlines and must not start
/// with `-` (argument injection).
pub fn check_cli_path(path: &str) -> Result<(), ValidationError> {
    let p = std::path::Path::new(path);
    if path.is_empty() || path.contains('\0') || path.contains('\n') || path.contains('\r') || path.starts_with('-') || !p.is_absolute() {
        return Err(ValidationError::BadPath);
    }
    Ok(())
}

pub fn is_private_or_link_local(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_link_local(),
        IpAddr::V6(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_normal_lan_ip() {
        assert_eq!(parse_tv_ip(" 192.168.1.50 ").unwrap(), Ipv4Addr::new(192, 168, 1, 50));
    }

    #[test]
    fn rejects_garbage_and_injection() {
        for bad in ["", "tv.local", "192.168.1", "192.168.1.256", "1.2.3.4; rm -rf /", "-1.2.3.4", "０.0.0.1", "192.168.1.5\n--x"] {
            assert!(parse_tv_ip(bad).is_err(), "{bad:?} should fail");
        }
    }

    #[test]
    fn rejects_non_tv_addresses() {
        for bad in ["127.0.0.1", "0.0.0.0", "255.255.255.255", "224.0.0.1"] {
            assert_eq!(parse_tv_ip(bad), Err(ValidationError::ForbiddenIp));
        }
    }

    #[test]
    fn ports() {
        assert_eq!(parse_port(26101), Ok(26101));
        assert!(parse_port(0).is_err());
        assert!(parse_port(65536).is_err());
    }

    #[test]
    fn subnet_prefix() {
        assert_eq!(parse_subnet_prefix("192.168.1").unwrap(), "192.168.1");
        for bad in ["192.168", "192.168.1.1", "a.b.c", "1..2", "127.0.0", "192.168.1; ls"] {
            assert!(parse_subnet_prefix(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn serial() {
        assert_eq!(parse_serial("192.168.1.5:26101").unwrap().1, 26101);
        assert!(parse_serial("192.168.1.5").is_err());
        assert!(parse_serial("emulator-26101").is_err());
    }

    #[test]
    fn cli_paths() {
        assert!(check_cli_path("-rf").is_err());
        assert!(check_cli_path("relative/app.wgt").is_err());
        assert!(check_cli_path("/tmp/a\nb.wgt").is_err());
        #[cfg(unix)]
        assert!(check_cli_path("/tmp/app.wgt").is_ok());
    }
}
