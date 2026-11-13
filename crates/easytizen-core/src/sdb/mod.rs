//! Native SDB engine: everything EasyTizen needs from a Samsung TV without the Tizen SDK.
//!
//! The device-side commands mirror what Samsung's own tools send (checked against the
//! official `sdb` 4.2.36 client and the pinned Samsung CLI in `resources/samsung-cli`):
//!
//! | Purpose        | TV with secure protocol            | Other Tizen devices                         |
//! |----------------|------------------------------------|---------------------------------------------|
//! | DUID           | `0 getduid` (`0 getduidgadget` on product 1.0) | `/opt/etc/duid-gadget 2.0 2> /dev/null` |
//! | Install        | `0 vd_appinstall <app-id> <path>`  | `/usr/bin/pkgcmd -i -t wgt -p "<path>" -q`  |
//! | Launch         | `0 was_execute <app-id>`           | `app_launcher -s <app-id>`                  |
//! | Uninstall      | `0 vd_appuninstall <app-id>`       | `/usr/bin/pkgcmd -u -n <package-id> -q`     |
//! | Remove upload  | `0 rmfile "<path>"`                | `/bin/rm -f "<path>"`                       |
//!
//! NOTE: this engine has been exercised against the protocol-level TV emulator in
//! [`mock`], not yet against physical televisions — see docs/KNOWN-LIMITATIONS.md.

pub mod client;
#[cfg(any(test, feature = "mock-tv"))]
pub mod mock;
pub mod protocol;
pub mod scan;

use crate::cancel::CancelToken;
pub use client::{Capability, SdbClient, SdbError};
use serde::Serialize;
use std::io::Read;
use std::net::Ipv4Addr;
use std::time::Duration;

pub const FALLBACK_TMP_DIR: &str = "/home/owner/share/tmp/sdk_tools/tmp";
const SHELL_TIMEOUT: Duration = Duration::from_secs(20);

/// A word that is safe to splice into a device shell command line.
pub fn is_safe_shell_word(word: &str) -> bool {
    !word.is_empty()
        && word.len() <= 128
        && word
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TvIdentity {
    pub duid: Option<String>,
    pub model: Option<String>,
    pub platform_version: Option<String>,
    pub secure_protocol: bool,
}

pub fn duid_command(cap: &Capability) -> &'static str {
    if !cap.secure_protocol() {
        "/opt/etc/duid-gadget 2.0 2> /dev/null"
    } else if cap.get("product_version") == Some("1.0") {
        "0 getduidgadget"
    } else {
        "0 getduid"
    }
}

/// Picks the DUID out of a shell answer (`duid: X`, or a bare value on its own line).
pub fn extract_duid(output: &str) -> Option<String> {
    let valid = |v: &str| {
        (4..=128).contains(&v.len())
            && v.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"#.-:=".contains(&b))
    };
    let clean = |v: &str| v.trim().trim_matches(['"', '\'']).to_ascii_uppercase();
    for line in output.lines() {
        let lower = line.trim().to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("duid") {
            let rest = rest.trim_start();
            if let Some(value) = rest.strip_prefix([':', '=']) {
                let value = clean(value);
                if valid(&value) {
                    return Some(value);
                }
            }
        }
    }
    output
        .lines()
        .map(clean)
        .find(|v| valid(v) && !v.contains("NOT") && v.chars().any(|c| c.is_ascii_digit()))
}

/// Connects, confirms the device speaks SDB and reads its identity (DUID, model).
pub fn query_tv(
    ip: Ipv4Addr,
    port: u16,
    timeout: Duration,
    cancel: &CancelToken,
) -> Result<TvIdentity, SdbError> {
    let mut client = SdbClient::connect(ip, port, timeout, cancel)?;
    let cap = match client.capability(SHELL_TIMEOUT) {
        Ok(cap) => cap,
        Err(SdbError::ServiceRefused(_)) => Capability::default(),
        Err(e) => return Err(e),
    };
    let duid = match client.shell(duid_command(&cap), SHELL_TIMEOUT) {
        Ok(out) => extract_duid(&out),
        Err(SdbError::Cancelled) => return Err(SdbError::Cancelled),
        Err(_) => None,
    };
    Ok(TvIdentity {
        duid,
        model: cap
            .get("device_name")
            .or(cap.get("model_name"))
            .map(str::to_string)
            .filter(|m| !m.is_empty()),
        platform_version: cap.get("platform_version").map(str::to_string),
        secure_protocol: cap.secure_protocol(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallFailure {
    /// The TV does not trust the signature (wrong DUID, expired or foreign certificate).
    CertificateRejected,
    /// The app is already installed and was signed by a different author certificate.
    AuthorMismatch,
    NoSpace,
    /// The TV answered, but not with a recognisable success marker.
    Unrecognised,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallVerdict {
    Installed,
    Failed(InstallFailure),
}

/// Interprets the TV's installer log. Deliberately conservative: anything that is not a
/// recognised success marker is reported as a failure, never as a success.
pub fn classify_install_output(output: &str) -> InstallVerdict {
    let low = output.to_ascii_lowercase();
    let nonzero_ret = low
        .split("cmd_ret:")
        .skip(1)
        .any(|rest| !rest.trim_start().starts_with('0'));
    let failed = low.contains("val[fail]")
        || low.contains("install failed")
        || low.contains("failed[")
        || nonzero_ret
        || low.contains("processing result : fail");
    if failed {
        let kind = if low.contains("author") && (low.contains("match") || low.contains("different"))
        {
            InstallFailure::AuthorMismatch
        } else if low.contains("certificate") || low.contains("signature") {
            InstallFailure::CertificateRejected
        } else if low.contains("space") || low.contains("storage") {
            InstallFailure::NoSpace
        } else {
            InstallFailure::Other
        };
        return InstallVerdict::Failed(kind);
    }
    if low.contains("install completed") || low.contains("cmd_ret:0") || low.contains("val[ok]") {
        InstallVerdict::Installed
    } else {
        InstallVerdict::Failed(InstallFailure::Unrecognised)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallStage {
    Connecting,
    Uploading,
    Installing,
    Launching,
}

#[derive(Debug)]
pub struct InstallOutcome {
    pub verdict: InstallVerdict,
    /// Raw installer log from the TV (shown under "technical details").
    pub log: String,
    /// `Some(true/false)` when a launch was requested.
    pub launched: Option<bool>,
}

fn launch_on(client: &mut SdbClient, secure: bool, app_id: &str) -> Result<bool, SdbError> {
    let command = if secure {
        format!("0 was_execute {app_id}")
    } else {
        format!("app_launcher -s {app_id}")
    };
    let out = client
        .shell(&command, Duration::from_secs(30))?
        .to_ascii_lowercase();
    Ok(out.contains("launched") || out.contains("resumed"))
}

/// Uploads a signed `.wgt` and asks the TV to install it. `progress(stage, fraction)` is
/// called with the upload fraction (0–1) while uploading.
#[allow(clippy::too_many_arguments)]
pub fn install_wgt(
    ip: Ipv4Addr,
    port: u16,
    package: &mut dyn Read,
    package_size: u64,
    app_id: &str,
    run_after_install: bool,
    timeout: Duration,
    cancel: &CancelToken,
    progress: &mut dyn FnMut(InstallStage, f32),
) -> Result<InstallOutcome, SdbError> {
    if !is_safe_shell_word(app_id) {
        return Err(SdbError::Protocol("unsafe application id".into()));
    }
    progress(InstallStage::Connecting, 0.0);
    let mut client = SdbClient::connect(ip, port, timeout, cancel)?;
    let cap = match client.capability(SHELL_TIMEOUT) {
        Ok(cap) => cap,
        Err(SdbError::ServiceRefused(_)) => Capability::default(),
        Err(e) => return Err(e),
    };
    let secure = cap.secure_protocol();
    let dir = cap
        .package_tmp_dir()
        .unwrap_or_else(|| FALLBACK_TMP_DIR.to_string());
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let remote = format!("{dir}/easytizen-{stamp}.wgt");

    progress(InstallStage::Uploading, 0.0);
    client.push(
        package,
        package_size,
        &remote,
        Duration::from_secs(60),
        &mut |sent| {
            let fraction = if package_size == 0 {
                1.0
            } else {
                sent as f32 / package_size as f32
            };
            progress(InstallStage::Uploading, fraction.min(1.0));
        },
    )?;

    progress(InstallStage::Installing, 0.0);
    let install = if secure {
        format!("0 vd_appinstall {app_id} {remote}")
    } else {
        format!("/usr/bin/pkgcmd -i -t wgt -p \"{remote}\" -q")
    };
    let log = client.shell(&install, Duration::from_secs(5 * 60));
    // Best-effort cleanup of the uploaded copy, whatever the installer said.
    let cleanup = if secure {
        format!("0 rmfile \"{remote}\"")
    } else {
        format!("/bin/rm -f \"{remote}\"")
    };
    let _ = client.shell(&cleanup, SHELL_TIMEOUT);
    let log = log?;
    let verdict = classify_install_output(&log);

    let mut launched = None;
    if run_after_install && verdict == InstallVerdict::Installed {
        progress(InstallStage::Launching, 0.0);
        launched = Some(launch_on(&mut client, secure, app_id).unwrap_or(false));
    }
    Ok(InstallOutcome {
        verdict,
        log,
        launched,
    })
}

/// Removes an installed app (needed when the TV holds a copy signed by another author).
pub fn uninstall_app(
    ip: Ipv4Addr,
    port: u16,
    app_id: &str,
    timeout: Duration,
    cancel: &CancelToken,
) -> Result<String, SdbError> {
    if !is_safe_shell_word(app_id) {
        return Err(SdbError::Protocol("unsafe application id".into()));
    }
    let mut client = SdbClient::connect(ip, port, timeout, cancel)?;
    let secure = client
        .capability(SHELL_TIMEOUT)
        .map(|c| c.secure_protocol())
        .unwrap_or(true);
    let command = if secure {
        format!("0 vd_appuninstall {app_id}")
    } else {
        let package = app_id.split('.').next().unwrap_or(app_id);
        format!("/usr/bin/pkgcmd -u -n {package} -q")
    };
    client.shell(&command, Duration::from_secs(120))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duid_extraction() {
        assert_eq!(
            extract_duid("ABCDEF1234567890\n").as_deref(),
            Some("ABCDEF1234567890")
        );
        assert_eq!(
            extract_duid("duid: abcd1234efgh\n").as_deref(),
            Some("ABCD1234EFGH")
        );
        assert_eq!(
            extract_duid("\n  \"2.0#QWER1234\"  \n").as_deref(),
            Some("2.0#QWER1234")
        );
        assert_eq!(extract_duid(""), None);
        assert_eq!(extract_duid("command not found\n"), None);
        assert_eq!(extract_duid("$(reboot) 1234\n"), None);
    }

    #[test]
    fn duid_command_selection_matches_samsung_cli() {
        let cap = |s: &str| Capability::parse(s.as_bytes());
        assert_eq!(
            duid_command(&cap("secure_protocol:disabled\n")),
            "/opt/etc/duid-gadget 2.0 2> /dev/null"
        );
        assert_eq!(
            duid_command(&cap("secure_protocol:enabled\nproduct_version:1.0\n")),
            "0 getduidgadget"
        );
        assert_eq!(
            duid_command(&cap("secure_protocol:enabled\nproduct_version:5.0\n")),
            "0 getduid"
        );
    }

    #[test]
    fn install_log_classification_is_conservative() {
        let ok = "install abc.App\npackage_path /x.wgt\napp_id[abc.App] install start\napp_id[abc.App] install completed\nspend time for wascmd is [3210]ms\ncmd_ret:0\n";
        assert_eq!(classify_install_output(ok), InstallVerdict::Installed);
        let cert = "app_id[abc.App] install failed[118, -12], reason: Check certificate error : :Invalid certificate chain with certificate in signature\ncmd_ret:1\n";
        assert_eq!(
            classify_install_output(cert),
            InstallVerdict::Failed(InstallFailure::CertificateRejected)
        );
        let author = "install failed[118, -11], reason: Author certificate not match : :\n";
        assert_eq!(
            classify_install_output(author),
            InstallVerdict::Failed(InstallFailure::AuthorMismatch)
        );
        assert_eq!(
            classify_install_output("key[end] val[ok]\n"),
            InstallVerdict::Installed
        );
        assert_eq!(
            classify_install_output("key[end] val[fail]\n"),
            InstallVerdict::Failed(InstallFailure::Other)
        );
        assert_eq!(
            classify_install_output(""),
            InstallVerdict::Failed(InstallFailure::Unrecognised)
        );
        assert_eq!(
            classify_install_output("closed"),
            InstallVerdict::Failed(InstallFailure::Unrecognised)
        );
    }

    #[test]
    fn shell_words() {
        assert!(is_safe_shell_word("AbCdE12345.My-App_1"));
        for bad in ["", "a b", "a;b", "$(x)", "a/b", "a\"b", "`x`"] {
            assert!(!is_safe_shell_word(bad), "{bad}");
        }
    }
}
