//! Maps technical failures to stable, translatable diagnosis keys.
//! The UI turns each key into plain-language text and a list of things to check.

use crate::connectivity::Probe;
use crate::samsung::envelope::Envelope;
use crate::samsung::runner::RunError;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Diagnosis {
    /// Tizen SDK / sdb not found at the configured location.
    ToolsMissing,
    NodeProblem,
    NotEnoughDiskSpace,
    /// Nothing answered at the TV's address.
    TvUnreachable,
    /// TV answered but SDB port is closed.
    PortClosed,
    /// Computer has no route to the address.
    NetworkUnreachable,
    /// sdb said the device is not there / rejected the connection after TCP succeeded.
    ConnectionRejected,
    OperationTimedOut,
    Cancelled,
    /// Samsung CLI crashed or printed nothing parseable.
    CliFailure,
    Unknown,
}

pub fn from_probe(p: Probe) -> Option<Diagnosis> {
    match p {
        Probe::Open => None,
        Probe::Refused => Some(Diagnosis::PortClosed),
        Probe::TimedOut => Some(Diagnosis::TvUnreachable),
        Probe::NetworkUnreachable => Some(Diagnosis::NetworkUnreachable),
        Probe::Other => Some(Diagnosis::TvUnreachable),
    }
}

pub fn from_run_error(e: &RunError) -> Diagnosis {
    match e {
        RunError::Timeout(_) => Diagnosis::OperationTimedOut,
        RunError::Cancelled => Diagnosis::Cancelled,
        RunError::Spawn(_) | RunError::ScriptMissing => Diagnosis::NodeProblem,
    }
}

pub fn from_envelope(env: &Envelope) -> Option<Diagnosis> {
    if env.is_success() {
        return None;
    }
    let cat = env
        .errors
        .first()
        .map(|e| e.category())
        .unwrap_or("unknown_error");
    Some(match cat {
        "sdk_path_not_set"
        | "sdk_path_invalid"
        | "sdk_path_not_accessible"
        | "install_required"
        | "dependency_missing" => Diagnosis::ToolsMissing,
        "node_not_found"
        | "node_version_unsupported"
        | "runner_not_found"
        | "runner_failed"
        | "script_not_found" => Diagnosis::NodeProblem,
        "insufficient_disk_space" => Diagnosis::NotEnoughDiskSpace,
        "device_not_found" | "multiple_devices" => Diagnosis::ConnectionRejected,
        "execution_error" => {
            // cert-manager legacy shape wraps the underlying message; classify by text.
            let msg = env
                .errors
                .first()
                .and_then(|e| e.message.as_deref())
                .unwrap_or("")
                .to_ascii_lowercase();
            if msg.contains("sdb not found") || msg.contains("sdb not available") {
                Diagnosis::ToolsMissing
            } else if msg.contains("timed out") || msg.contains("timeout") {
                Diagnosis::OperationTimedOut
            } else {
                Diagnosis::Unknown
            }
        }
        _ => Diagnosis::Unknown,
    })
}

/// A message safe to show in an expandable "technical details" area. Strips the user's home
/// directory and anything resembling a secret.
pub fn technical_detail(env: &Envelope, home: Option<&str>) -> String {
    let mut s = env
        .errors
        .iter()
        .filter_map(|e| e.message.clone())
        .collect::<Vec<_>>()
        .join("\n");
    if let Some(h) = home.filter(|h| !h.is_empty()) {
        s = s.replace(h, "~");
    }
    redact(&s)
}

pub fn redact(s: &str) -> String {
    s.lines()
        .map(|l| {
            let low = l.to_ascii_lowercase();
            if low.contains("password")
                || low.contains("passwd")
                || low.contains("token")
                || low.contains("secret")
            {
                "[redacted line]".to_string()
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::samsung::envelope::parse_envelope;

    #[test]
    fn real_failures_map_to_tools_missing() {
        let e =
            parse_envelope(include_str!("../tests/fixtures/connect_failure_no_sdk.out")).unwrap();
        assert_eq!(from_envelope(&e), Some(Diagnosis::ToolsMissing));
        let e = parse_envelope(include_str!(
            "../tests/fixtures/acquire_duid_error_no_sdk.out"
        ))
        .unwrap();
        assert_eq!(from_envelope(&e), Some(Diagnosis::ToolsMissing));
    }

    #[test]
    fn success_has_no_diagnosis() {
        let e = parse_envelope(include_str!("../tests/fixtures/scan_success_no_sdb.out")).unwrap();
        assert_eq!(from_envelope(&e), None);
    }

    #[test]
    fn probes() {
        assert_eq!(from_probe(Probe::Open), None);
        assert_eq!(from_probe(Probe::Refused), Some(Diagnosis::PortClosed));
        assert_eq!(from_probe(Probe::TimedOut), Some(Diagnosis::TvUnreachable));
    }

    #[test]
    fn redaction_and_home_stripping() {
        assert_eq!(redact("ok\nmy password is x"), "ok\n[redacted line]");
        let e =
            parse_envelope(include_str!("../tests/fixtures/connect_failure_no_sdk.out")).unwrap();
        let d = technical_detail(&e, Some("<HOME>"));
        assert!(!d.contains("<HOME>"));
        assert!(d.contains("~/tizen-sdk"));
    }
}
