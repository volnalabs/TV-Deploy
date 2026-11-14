//! Update check: asks GitHub which release is the latest and compares versions.
//! Nothing is downloaded or installed automatically — the user is offered the link.

use serde::Serialize;
use std::time::Duration;

pub const REPOSITORY: &str = "volnalabs/TV-Deploy";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub update_available: bool,
    /// Release page with notes and all installers.
    pub release_url: String,
    /// Stable direct link to this platform's installer.
    pub download_url: String,
}

/// Parses `1.2.3`, `v1.2.3` or `1.2.3-beta.1` into comparable numbers (pre-release tags
/// are ignored for ordering: `1.2.3-beta` is treated as `1.2.3`).
pub fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let core = text.trim().trim_start_matches(['v', 'V']);
    let core = core.split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    parts.next().is_none().then_some((major, minor, patch))
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

/// File name of the installer for the running platform, as published in every release.
pub fn installer_asset(os: &str, arch: &str) -> &'static str {
    match (os, arch) {
        ("android", _) => "EasyTizen-Android.apk",
        ("windows", _) => "EasyTizen-Windows-x64-Setup.exe",
        ("macos", "aarch64") => "EasyTizen-macOS-ARM64.dmg",
        ("macos", _) => "EasyTizen-macOS-Intel.dmg",
        _ => "EasyTizen-Linux-x64.AppImage",
    }
}

pub fn build_info(current: &str, latest_tag: &str, os: &str, arch: &str) -> UpdateInfo {
    UpdateInfo {
        current: current.to_string(),
        latest: latest_tag.trim_start_matches(['v', 'V']).to_string(),
        update_available: is_newer(latest_tag, current),
        release_url: format!("https://github.com/{REPOSITORY}/releases/latest"),
        download_url: format!(
            "https://github.com/{REPOSITORY}/releases/latest/download/{}",
            installer_asset(os, arch)
        ),
    }
}

/// One HTTPS GET to api.github.com. No identifiers are sent besides a generic User-Agent.
pub fn check(current: &str) -> Result<UpdateInfo, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .http_status_as_error(false)
        .build()
        .into();
    let mut response = agent
        .get(format!(
            "https://api.github.com/repos/{REPOSITORY}/releases/latest"
        ))
        .header("User-Agent", "EasyTizen")
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|_| "network".to_string())?;
    match response.status().as_u16() {
        200 => {}
        404 => return Err("no_release".into()),
        _ => return Err("network".into()),
    }
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|_| "network".to_string())?;
    let json: serde_json::Value = serde_json::from_str(&body).map_err(|_| "network".to_string())?;
    let tag = json
        .get("tag_name")
        .and_then(|t| t.as_str())
        .ok_or("no_release")?;
    if parse_version(tag).is_none() {
        return Err("no_release".into());
    }
    Ok(build_info(
        current,
        tag,
        std::env::consts::OS,
        std::env::consts::ARCH,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(parse_version("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("1.2"), Some((1, 2, 0)));
        assert_eq!(parse_version("1.2.3-beta.1"), Some((1, 2, 3)));
        assert_eq!(parse_version("release-1"), None);
        assert_eq!(parse_version("1.2.3.4"), None);
        assert!(is_newer("v1.0.1", "1.0.0"));
        assert!(is_newer("v1.10.0", "1.9.9"));
        assert!(!is_newer("v1.0.0", "1.0.0"));
        assert!(!is_newer("v0.9.0", "1.0.0"));
        assert!(!is_newer("garbage", "1.0.0"));
    }

    #[test]
    fn links_point_at_the_published_file_names() {
        let info = build_info("1.0.0", "v1.1.0", "macos", "aarch64");
        assert!(info.update_available);
        assert_eq!(info.latest, "1.1.0");
        assert_eq!(
            info.download_url,
            "https://github.com/volnalabs/TV-Deploy/releases/latest/download/EasyTizen-macOS-ARM64.dmg"
        );
        assert_eq!(
            installer_asset("android", "aarch64"),
            "EasyTizen-Android.apk"
        );
        assert_eq!(
            installer_asset("windows", "x86_64"),
            "EasyTizen-Windows-x64-Setup.exe"
        );
        assert_eq!(
            installer_asset("macos", "x86_64"),
            "EasyTizen-macOS-Intel.dmg"
        );
        assert_eq!(
            installer_asset("linux", "x86_64"),
            "EasyTizen-Linux-x64.AppImage"
        );
    }
}
