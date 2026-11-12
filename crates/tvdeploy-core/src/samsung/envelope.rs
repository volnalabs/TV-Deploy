//! Parsing of the Samsung CLI "Standard JSON Envelope".
//!
//! Verified against `resources/samsung-cli/lib/envelope/envelope.js` (commit pinned in
//! `PINNED.json`) and against real output captured from the scripts (see tests/fixtures).
//! Note that most scripts use `status: success|failure|needs_action`, while
//! `cert-manager-cli.js` additionally emits `status: "error"` with `errors[{code,message}]`.

use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Success,
    Failure,
    NeedsAction,
    /// Legacy shape used by some runners.
    Error,
    Other(String),
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SuggestedFix {
    pub command: Option<String>,
    pub guide_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct EnvError {
    pub error_code: Option<String>,
    pub error_category: Option<String>,
    /// Legacy field (`cert-manager-cli.js` "error" envelopes).
    pub code: Option<String>,
    pub message: Option<String>,
    pub suggested_fix: Option<SuggestedFix>,
}

impl EnvError {
    /// Best available stable category.
    pub fn category(&self) -> &str {
        self.error_category.as_deref().or(self.code.as_deref()).unwrap_or("unknown_error")
    }
}

#[derive(Debug, Clone)]
pub struct Envelope {
    pub status: Status,
    pub result: Option<Value>,
    pub warnings: Vec<String>,
    pub errors: Vec<EnvError>,
    pub command: Option<String>,
    pub duration_ms: Option<u64>,
}

impl Envelope {
    pub fn is_success(&self) -> bool {
        self.status == Status::Success
    }
}

#[derive(Deserialize)]
struct Raw {
    status: Option<String>,
    result: Option<Value>,
    #[serde(default)]
    warnings: Vec<Value>,
    #[serde(default)]
    errors: Vec<EnvError>,
    command: Option<String>,
    duration_ms: Option<u64>,
}

/// Extracts the envelope from stdout. The scripts print progress lines before the JSON
/// (usually on stderr, but we tolerate them on stdout too): we try every line that begins an
/// object and take the last one that parses into something with a `status`.
pub fn parse_envelope(stdout: &str) -> Option<Envelope> {
    let mut found = None;
    let mut offset = 0;
    for line in stdout.split_inclusive('\n') {
        if line.trim_end() == "{" || line.starts_with('{') {
            let slice = &stdout[offset..];
            let mut de = serde_json::Deserializer::from_str(slice).into_iter::<Raw>();
            if let Some(Ok(raw)) = de.next() {
                if raw.status.is_some() {
                    found = Some(raw);
                }
            }
        }
        offset += line.len();
    }
    let raw = found?;
    let status = match raw.status.as_deref().unwrap_or("") {
        "success" => Status::Success,
        "failure" => Status::Failure,
        "needs_action" => Status::NeedsAction,
        "error" => Status::Error,
        other => Status::Other(other.to_string()),
    };
    Some(Envelope {
        status,
        result: raw.result,
        warnings: raw.warnings.into_iter().map(|w| w.as_str().map(str::to_string).unwrap_or_else(|| w.to_string())).collect(),
        errors: raw.errors,
        command: raw.command,
        duration_ms: raw.duration_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_success_with_noise() {
        let s = include_str!("../../tests/fixtures/scan_success_no_sdb.out");
        let e = parse_envelope(s).expect("envelope");
        assert!(e.is_success());
        assert_eq!(e.result.as_ref().unwrap()["device_count"], 0);
        assert_eq!(e.warnings.len(), 1);
    }

    #[test]
    fn parses_real_failure() {
        let e = parse_envelope(include_str!("../../tests/fixtures/connect_failure_no_sdk.out")).unwrap();
        assert_eq!(e.status, Status::Failure);
        assert_eq!(e.errors[0].category(), "sdk_path_not_set");
    }

    #[test]
    fn parses_legacy_error_shape() {
        let e = parse_envelope(include_str!("../../tests/fixtures/acquire_duid_error_no_sdk.out")).unwrap();
        assert_eq!(e.status, Status::Error);
        assert_eq!(e.errors[0].category(), "execution_error");
    }

    #[test]
    fn garbage_is_none() {
        assert!(parse_envelope("hello\nworld").is_none());
        assert!(parse_envelope("{ not json").is_none());
        assert!(parse_envelope("{\"foo\":1}").is_none());
    }
}
