//! Typed extraction of `result` payloads. Field names verified by running the pinned
//! scripts (see tests/fixtures; the `*_mock_sdb` fixtures were produced by the real Samsung
//! scripts talking to a mock `sdb`, so they confirm JSON shapes only — not TV behaviour).

use super::envelope::Envelope;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectedDevice {
    pub serial: String,
    pub ip: String,
    pub port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuidInfo {
    pub duid: String,
    pub serial: String,
}

pub fn connect_result(env: &Envelope) -> Option<ConnectedDevice> {
    let r = env.result.as_ref()?;
    if r.get("status")?.as_str()? != "connected" {
        return None;
    }
    Some(ConnectedDevice {
        serial: r.get("device_serial")?.as_str()?.to_string(),
        ip: r.get("ip")?.as_str()?.to_string(),
        port: u16::try_from(r.get("port")?.as_u64()?).ok()?,
    })
}

pub fn duid_result(env: &Envelope) -> Option<DuidInfo> {
    let r = env.result.as_ref()?;
    let duid = r.get("duid")?.as_str()?.trim().to_string();
    if duid.is_empty()
        || !duid
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"#.-:=".contains(&b))
    {
        return None;
    }
    Some(DuidInfo {
        duid,
        serial: r.get("serial")?.as_str()?.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::samsung::envelope::parse_envelope;

    #[test]
    fn connect_shape() {
        let e = parse_envelope(include_str!(
            "../../tests/fixtures/connect_success_mock_sdb.out"
        ))
        .unwrap();
        let c = connect_result(&e).unwrap();
        assert_eq!(
            (c.ip.as_str(), c.port, c.serial.as_str()),
            ("192.168.1.5", 26101, "192.168.1.5:26101")
        );
    }

    #[test]
    fn duid_shape() {
        let e = parse_envelope(include_str!(
            "../../tests/fixtures/duid_success_mock_sdb.out"
        ))
        .unwrap();
        assert_eq!(duid_result(&e).unwrap().duid, "MOCKDUID1234ABCD");
    }

    #[test]
    fn failures_yield_none() {
        let e = parse_envelope(include_str!(
            "../../tests/fixtures/connect_failure_no_sdk.out"
        ))
        .unwrap();
        assert!(connect_result(&e).is_none() && duid_result(&e).is_none());
    }
}
