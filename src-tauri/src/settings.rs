//! Non-secret settings, stored as JSON in the app data directory. Secrets (certificate
//! passwords, private keys, Samsung tokens) are never written here.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SavedDevice {
    pub ip: String,
    pub port: u16,
    pub name: String,
    pub duid: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    /// Unix seconds of the last confirmed connection.
    #[serde(default)]
    pub last_connected: Option<i64>,
}

/// Which toolchain created (and can use) a certificate profile.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Engine {
    /// Official Tizen SDK + Samsung CLI (desktop only).
    #[default]
    Sdk,
    /// EasyTizen's built-in SDB client and signer (all platforms; the only one on Android).
    Builtin,
}

/// Public metadata of a signing profile. Keys and passwords live elsewhere: in the Tizen
/// SDK keystore (`Sdk`) or in password-protected `.p12` files in the app data folder
/// (`Builtin`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SavedCertificateProfile {
    pub name: String,
    pub identity: String,
    /// TVs this profile's distributor certificate is valid for.
    #[serde(default)]
    pub duids: Vec<String>,
    /// Legacy single-DUID field written by TV Deploy 0.1; folded into `duids` on load.
    #[serde(default, skip_serializing)]
    duid: Option<String>,
    #[serde(default)]
    pub engine: Engine,
    /// Unix seconds; `None` when unknown (profiles created by older versions).
    #[serde(default)]
    pub created_at: Option<i64>,
    #[serde(default)]
    pub expires_at: Option<i64>,
    #[serde(default)]
    pub imported: bool,
}

impl SavedCertificateProfile {
    pub fn new(name: String, identity: String, duids: Vec<String>, engine: Engine) -> Self {
        Self {
            name,
            identity,
            duids,
            duid: None,
            engine,
            created_at: Some(crate::now_secs()),
            expires_at: None,
            imported: false,
        }
    }
    pub fn covers(&self, duid: &str) -> bool {
        self.duids.iter().any(|d| d.eq_ignore_ascii_case(duid))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// "system" | "light" | "dark"
    pub theme: String,
    /// "auto" | "ru" | "en"
    pub language: String,
    pub engine: Engine,
    pub sdk_path: Option<String>,
    pub default_port: u16,
    pub timeout_ms: u64,
    /// Saved TVs are remembered between launches but are never treated as connected without
    /// a fresh check.
    pub devices: Vec<SavedDevice>,
    pub certificate_profiles: Vec<SavedCertificateProfile>,
    pub onboarding_done: bool,
    /// Ask GitHub for a newer EasyTizen release at start-up.
    pub check_updates: bool,
    /// Folder of an SDK installation that was started but has not finished (resume hint).
    pub pending_sdk_install: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            language: "auto".into(),
            engine: if cfg!(target_os = "android") {
                Engine::Builtin
            } else {
                Engine::Sdk
            },
            sdk_path: None,
            default_port: 26101,
            timeout_ms: 3000,
            devices: vec![],
            certificate_profiles: vec![],
            onboarding_done: false,
            check_updates: true,
            pending_sdk_install: None,
        }
    }
}

fn file(dir: &Path) -> PathBuf {
    dir.join("settings.json")
}

pub fn load(dir: &Path) -> Settings {
    let existed = file(dir).is_file();
    let mut settings: Settings = std::fs::read_to_string(file(dir))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    for profile in &mut settings.certificate_profiles {
        if let Some(duid) = profile.duid.take() {
            if !profile.covers(&duid) {
                profile.duids.push(duid);
            }
        }
    }
    // People upgrading from TV Deploy already went through setup.
    if existed && (settings.sdk_path.is_some() || !settings.devices.is_empty()) {
        settings.onboarding_done = true;
    }
    if cfg!(target_os = "android") {
        settings.engine = Engine::Builtin;
    }
    settings
}

pub fn save(dir: &Path, s: &Settings) -> std::io::Result<()> {
    let tmp = dir.join("settings.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(s)?)?;
    std::fs::rename(tmp, file(dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_defaults_on_garbage() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(load(d.path()).default_port, 26101);
        let mut s = Settings {
            theme: "dark".into(),
            ..Settings::default()
        };
        s.devices.push(SavedDevice {
            ip: "192.168.1.5".into(),
            port: 26101,
            name: "TV".into(),
            duid: None,
            model: None,
            last_connected: None,
        });
        save(d.path(), &s).unwrap();
        let back = load(d.path());
        assert_eq!(back.theme, "dark");
        assert_eq!(back.devices.len(), 1);
        std::fs::write(file(d.path()), "{broken").unwrap();
        assert_eq!(load(d.path()).theme, "system");
    }

    #[test]
    fn tv_deploy_0_1_settings_are_upgraded_without_loss() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(
            file(d.path()),
            r#"{"theme":"dark","language":"ru","sdk_path":"/home/u/tizen-sdk","default_port":26101,"timeout_ms":3000,
               "devices":[{"ip":"192.168.1.5","port":26101,"name":"Гостиная","duid":"ABCD1234EFGH"}],
               "certificate_profiles":[{"name":"TVDeploy-5","identity":"Me","duid":"ABCD1234EFGH"}]}"#,
        )
        .unwrap();
        let s = load(d.path());
        assert_eq!(s.devices[0].name, "Гостиная");
        assert_eq!(s.sdk_path.as_deref(), Some("/home/u/tizen-sdk"));
        let profile = &s.certificate_profiles[0];
        assert_eq!(profile.duids, ["ABCD1234EFGH"]);
        assert_eq!(profile.engine, Engine::Sdk);
        assert!(profile.covers("abcd1234efgh"));
        assert!(
            s.onboarding_done,
            "existing users must not be sent through first-run setup"
        );
        // Saving and loading again keeps the upgraded shape.
        save(d.path(), &s).unwrap();
        assert_eq!(
            load(d.path()).certificate_profiles[0].duids,
            ["ABCD1234EFGH"]
        );
    }

    #[test]
    fn no_secret_like_fields() {
        let json = serde_json::to_string(&Settings::default())
            .unwrap()
            .to_lowercase();
        for bad in ["password", "secret", "token", "private"] {
            assert!(!json.contains(bad));
        }
    }
}
