//! Typed Samsung CLI invocations. Verified against the pinned sources:
//!
//! * `remote-device-cli.js  scan [subnet] [--port N] [--timeout MS]`
//! * `remote-device-cli.js  connect|disconnect <ip[:port]> [--port N]`
//! * `remote-device-cli.js  list`
//! * `cert-manager-cli.js   acquire-duid [--serial S]`
//! * `check-node-cli.js`
//! * `sdk-init-cli.js [sdkPath]`
//! * `project-manager-cli.js install --package P [--device-serial S] [--run]` (iteration 3)

use crate::validate::*;
use std::net::Ipv4Addr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Script {
    CheckNode,
    SdkInit,
    RemoteDevice,
    CertManager,
    ProjectManager,
}

impl Script {
    pub fn file_name(self) -> &'static str {
        match self {
            Script::CheckNode => "check-node-cli.js",
            Script::SdkInit => "sdk-init-cli.js",
            Script::RemoteDevice => "remote-device-cli.js",
            Script::CertManager => "cert-manager-cli.js",
            Script::ProjectManager => "project-manager-cli.js",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    pub script: Script,
    pub args: Vec<String>,
}

impl Invocation {
    /// `sdk-init-cli.js <sdkPath>` — writes `~/.tizen.sdk.path.config` (inside TV Deploy's
    /// private HOME, see `runner`). `TIZEN_SDK_PATH` is deliberately ignored by the Samsung
    /// scripts (core/sdk.js `readSdkPath`), so this is the only supported way.
    pub fn sdk_init(sdk_path: &str) -> Result<Self, ValidationError> {
        check_cli_path(sdk_path)?;
        Ok(Self {
            script: Script::SdkInit,
            args: vec![sdk_path.to_string()],
        })
    }

    pub fn check_node() -> Self {
        Self {
            script: Script::CheckNode,
            args: vec![],
        }
    }

    /// LAN scan. With `subnet = None` the Samsung CLI decides which local subnet to sweep.
    pub fn scan(subnet: Option<&str>, port: u16, timeout_ms: u64) -> Result<Self, ValidationError> {
        let mut args = vec!["scan".to_string()];
        if let Some(s) = subnet {
            args.push(parse_subnet_prefix(s)?);
        }
        args.push("--port".into());
        args.push(port.to_string());
        args.push("--timeout".into());
        args.push(clamp_timeout_ms(timeout_ms)?.to_string());
        Ok(Self {
            script: Script::RemoteDevice,
            args,
        })
    }

    pub fn connect(ip: Ipv4Addr, port: u16) -> Self {
        Self {
            script: Script::RemoteDevice,
            args: vec![
                "connect".into(),
                ip.to_string(),
                "--port".into(),
                port.to_string(),
            ],
        }
    }

    pub fn disconnect(ip: Ipv4Addr, port: u16) -> Self {
        Self {
            script: Script::RemoteDevice,
            args: vec![
                "disconnect".into(),
                ip.to_string(),
                "--port".into(),
                port.to_string(),
            ],
        }
    }

    pub fn list_connected() -> Self {
        Self {
            script: Script::RemoteDevice,
            args: vec!["list".into()],
        }
    }

    pub fn acquire_duid(ip: Ipv4Addr, port: u16) -> Self {
        Self {
            script: Script::CertManager,
            args: vec![
                "acquire-duid".into(),
                "--serial".into(),
                format!("{ip}:{port}"),
            ],
        }
    }

    /// Samsung online-CA author certificate. The password is supplied through the
    /// documented hidden-prompt environment variable by runner::run_with_env,
    /// never as a command-line argument.
    pub fn generate_samsung_author(profile: &str, identity: &str) -> Result<Self, ValidationError> {
        validate_profile(profile)?;
        validate_identity(identity)?;
        Ok(Self {
            script: Script::CertManager,
            args: vec![
                "generate-samsung-author".into(),
                "--profile-name".into(),
                profile.to_string(),
                "--identity".into(),
                identity.trim().to_string(),
                "--prompt-password".into(),
            ],
        })
    }

    /// Samsung online-CA distributor certificate bound to one or more DUIDs.
    pub fn generate_samsung_distributor(profile: &str, duid: &str) -> Result<Self, ValidationError> {
        validate_profile(profile)?;
        if duid.is_empty()
            || duid.len() > 256
            || duid.chars().any(|c| c.is_control())
            || !duid.bytes().all(|b| b.is_ascii_alphanumeric() || b"#.-:_".contains(&b))
        {
            return Err(ValidationError::BadPath);
        }
        Ok(Self {
            script: Script::CertManager,
            args: vec![
                "generate-samsung-distributor".into(),
                "--profile-name".into(),
                profile.to_string(),
                "--duid-list".into(),
                duid.to_string(),
                "--privilege".into(),
                "Public".into(),
                "--prompt-password".into(),
            ],
        })
    }

    pub fn create_samsung_profile(profile: &str) -> Result<Self, ValidationError> {
        validate_profile(profile)?;
        Ok(Self {
            script: Script::CertManager,
            args: vec![
                "create-samsung-profile".into(),
                "--profile-name".into(),
                profile.to_string(),
                "--active".into(),
            ],
        })
    }

    pub fn remove_samsung_profile(profile: &str) -> Result<Self, ValidationError> {
        validate_profile(profile)?;
        Ok(Self {
            script: Script::CertManager,
            args: vec![
                "remove-profile".into(),
                "--profile-name".into(),
                profile.to_string(),
            ],
        })
    }

    pub fn install_package(path: &str, serial: &str, run_after_install: bool) -> Result<Self, ValidationError> {
        check_cli_path(path)?;
        parse_serial(serial)?;
        let mut args = vec![
            "install".into(),
            "--package".into(),
            path.to_string(),
            "--device-serial".into(),
            serial.to_string(),
        ];
        if run_after_install {
            args.push("--run".into());
        }
        Ok(Self { script: Script::ProjectManager, args })
    }
}

fn validate_profile(profile: &str) -> Result<(), ValidationError> {
    let valid = (3..=64).contains(&profile.len())
        && profile.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    if valid { Ok(()) } else { Err(ValidationError::BadPath) }
}

fn validate_identity(identity: &str) -> Result<(), ValidationError> {
    let value = identity.trim();
    if (2..=120).contains(&value.chars().count())
        && !value.chars().any(|c| c.is_control())
    {
        Ok(())
    } else {
        Err(ValidationError::BadPath)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_args() {
        let i = Invocation::scan(Some("192.168.1"), 26101, 3000).unwrap();
        assert_eq!(
            i.args,
            ["scan", "192.168.1", "--port", "26101", "--timeout", "3000"]
        );
        assert!(Invocation::scan(Some("192.168.1; ls"), 26101, 3000).is_err());
        assert!(Invocation::scan(None, 26101, 5).is_err());
    }

    #[test]
    fn connect_args_are_separate_argv_items() {
        let i = Invocation::connect("192.168.1.5".parse().unwrap(), 26101);
        assert_eq!(i.args, ["connect", "192.168.1.5", "--port", "26101"]);
        assert_eq!(
            Invocation::acquire_duid("192.168.1.5".parse().unwrap(), 26101).args[2],
            "192.168.1.5:26101"
        );
    }
}
