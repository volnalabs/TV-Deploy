//! Non-secret settings, stored as JSON in the app data directory. Secrets (certificate
//! passwords, keys) must never be added here — they go to the OS keychain (iteration 2).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SavedDevice {
    pub ip: String,
    pub port: u16,
    pub name: String,
    pub duid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SavedCertificateProfile {
    pub name: String,
    pub identity: String,
    pub duid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// "system" | "light" | "dark"
    pub theme: String,
    /// "auto" | "ru" | "en"
    pub language: String,
    pub sdk_path: Option<String>,
    pub default_port: u16,
    pub timeout_ms: u64,
    /// Saved TVs are remembered between launches but are never treated as connected without a
    /// fresh check.
    pub devices: Vec<SavedDevice>,
    /// Public metadata only. Private keys and passwords remain in the Tizen SDK's
    /// platform-protected keystore and are never copied into settings.json.
    pub certificate_profiles: Vec<SavedCertificateProfile>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            language: "auto".into(),
            sdk_path: None,
            default_port: 26101,
            timeout_ms: 3000,
            devices: vec![],
            certificate_profiles: vec![],
        }
    }
}

fn file(dir: &Path) -> PathBuf {
    dir.join("settings.json")
}

pub fn load(dir: &Path) -> Settings {
    std::fs::read_to_string(file(dir))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
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
        let d = tempfile_dir();
        assert_eq!(load(&d).default_port, 26101);
        let mut s = Settings::default();
        s.theme = "dark".into();
        s.devices.push(SavedDevice {
            ip: "192.168.1.5".into(),
            port: 26101,
            name: "TV".into(),
            duid: None,
        });
        save(&d, &s).unwrap();
        let back = load(&d);
        assert_eq!(back.theme, "dark");
        assert_eq!(back.devices.len(), 1);
        std::fs::write(file(&d), "{broken").unwrap();
        assert_eq!(load(&d).theme, "system");
        let _ = std::fs::remove_dir_all(&d);
    }

    fn tempfile_dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "tvdeploy-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
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
