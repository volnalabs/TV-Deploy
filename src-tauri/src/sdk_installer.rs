//! Cross-platform bootstrap for the official Samsung Tizen SDK and TV Extension.
//! The exact PowerShell/Bash installers are vendored from the pinned Samsung commit
//! declared in resources/samsung-cli/PINNED.json. No SDK packages are mirrored by TV Deploy.

use fs2::available_space;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;
use tauri::{AppHandle, Manager};

const REQUIRED_FREE_BYTES: u64 = 15 * 1024 * 1024 * 1024;
const DOWNLOAD_JOBS: &str = "4";

fn resource_script(app: &AppHandle, relative: &str) -> Result<PathBuf, String> {
    let bundled = app
        .path()
        .resource_dir()
        .map_err(|e| e.to_string())?
        .join("samsung-cli")
        .join("scripts")
        .join(relative);
    if bundled.is_file() {
        return Ok(bundled);
    }

    // Development fallback. Production bundles must carry these resources.
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../resources/samsung-cli/scripts")
        .join(relative);
    if source.is_file() {
        return Ok(source);
    }
    Err(format!("Samsung installer resource is missing: {relative}"))
}

fn home_dir() -> Result<PathBuf, String> {
    let key = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(key)
        .map(PathBuf::from)
        .ok_or_else(|| format!("{key} is not set"))
}

fn sdk_root(path: Option<&str>) -> Result<PathBuf, String> {
    let root = match path {
        Some(value) if !value.trim().is_empty() => PathBuf::from(value),
        _ => home_dir()?.join("tizen-sdk"),
    };
    if !root.is_absolute() {
        return Err("The SDK folder must be an absolute path.".into());
    }
    Ok(root)
}

fn run_checked(mut command: Command, step: &str, timeout: Duration) -> Result<Output, String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }

    let output = command
        .output()
        .map_err(|e| format!("{step}: could not start installer ({})", e.kind()))?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = tail_text(&format!("{stdout}\n{stderr}"), 3000);
        return Err(format!(
            "{step} failed with exit code {:?}. {}",
            output.status.code(),
            if detail.is_empty() { "No diagnostic output was returned.".into() } else { detail }
        ));
    }
    let _ = timeout; // Installer scripts have their own durable status and resume markers.
    Ok(output)
}

fn tail_text(value: &str, max_chars: usize) -> String {
    let mut chars: Vec<char> = value.trim().chars().collect();
    if chars.len() > max_chars {
        chars.drain(..chars.len() - max_chars);
    }
    chars.into_iter().collect()
}

#[cfg(not(windows))]
fn bash4() -> Result<PathBuf, String> {
    let mut candidates = Vec::new();
    if let Some(paths) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&paths).map(|p| p.join("bash")));
    }
    #[cfg(target_os = "macos")]
    {
        candidates.push(PathBuf::from("/opt/homebrew/bin/bash"));
        candidates.push(PathBuf::from("/usr/local/bin/bash"));
    }
    candidates.push(PathBuf::from("/bin/bash"));
    candidates.push(PathBuf::from("/usr/bin/bash"));

    for candidate in candidates {
        if !candidate.is_file() {
            continue;
        }
        let output = Command::new(&candidate).arg("--version").output();
        if let Ok(output) = output {
            let text = String::from_utf8_lossy(&output.stdout);
            let major = text
                .split_whitespace()
                .find_map(|part| {
                    let clean = part.trim_start_matches("version").trim_matches(|c: char| !c.is_ascii_digit() && c != '.');
                    clean.split('.').next()?.parse::<u32>().ok()
                });
            if major.map(|v| v >= 4).unwrap_or(false) {
                return Ok(candidate);
            }
        }
    }

    #[cfg(target_os = "macos")]
    return Err(
        "Samsung's TV SDK installer requires Bash 4 or newer. macOS includes Bash 3.2. Install a current Bash with Homebrew (brew install bash), then restart TV Deploy.".into()
    );
    #[cfg(not(target_os = "macos"))]
    Err("Samsung's SDK installer requires Bash 4 or newer. Install Bash 4+ and retry.".into())
}

fn run_sdk_install(script: &Path, sdk: &Path, home: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        let mut command = Command::new("powershell.exe");
        command.args([
            "-NoLogo",
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ]);
        command.arg(script).args(["-Path"]).arg(sdk).args(["-DownloadJobs", DOWNLOAD_JOBS]);
        command.env("USERPROFILE", home).env("HOME", home);
        run_checked(command, "Tizen SDK installation", Duration::from_secs(90 * 60))?;
    }
    #[cfg(not(windows))]
    {
        let bash = bash4()?;
        let mut command = Command::new(bash);
        command.arg(script).args(["--path"]).arg(sdk).args(["--download-jobs", DOWNLOAD_JOBS]);
        command.env("HOME", home).env("USERPROFILE", home);
        run_checked(command, "Tizen SDK installation", Duration::from_secs(90 * 60))?;
    }
    Ok(())
}

fn run_tv_extension_install(script: &Path, sdk: &Path, home: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        let mut command = Command::new("powershell.exe");
        command.args([
            "-NoLogo",
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ]);
        command.arg(script).args(["-SdkPath"]).arg(sdk).args(["-DownloadJobs", DOWNLOAD_JOBS]);
        command.env("USERPROFILE", home).env("HOME", home);
        run_checked(command, "Samsung TV Extension installation", Duration::from_secs(60 * 60))?;
    }
    #[cfg(not(windows))]
    {
        let bash = bash4()?;
        let mut command = Command::new(bash);
        command.arg(script).arg(format!("--sdk-path={}", sdk.display()))
            .args(["--download-jobs", DOWNLOAD_JOBS]);
        command.env("HOME", home).env("USERPROFILE", home);
        run_checked(command, "Samsung TV Extension installation", Duration::from_secs(60 * 60))?;
    }
    Ok(())
}

fn base_sdk_ready(path: &Path) -> bool {
    path.join("sdk.info").is_file()
        && path.join("tools").join(if cfg!(windows) { "sdb.exe" } else { "sdb" }).is_file()
        && path.join("tools").join("tizen-core").join(if cfg!(windows) { "tz.exe" } else { "tz" }).is_file()
}

fn tv_extension_ready(path: &Path) -> bool {
    if path.join(".tv-sdk-installed").is_file() {
        return true;
    }
    std::fs::read_to_string(path.join(".platform-installed"))
        .map(|content| content.contains("TV-SAMSUNG-Public"))
        .unwrap_or(false)
}

/// Install the base Tizen SDK and Samsung TV Extension into a user-owned directory.
/// Existing SDK installations are reused; the extension is installed only when missing.
pub fn install(app: &AppHandle, requested_path: Option<&str>) -> Result<PathBuf, String> {
    let sdk = sdk_root(requested_path)?;
    let home = home_dir()?;
    let parent = sdk.parent().unwrap_or(&home);
    std::fs::create_dir_all(parent).map_err(|e| format!("Cannot access SDK folder: {e}"))?;

    if !base_sdk_ready(&sdk) {
        let free = available_space(parent).map_err(|e| format!("Cannot check free disk space: {e}"))?;
        if free < REQUIRED_FREE_BYTES {
            let free_gb = free as f64 / 1024_f64.powi(3);
            return Err(format!(
                "A first-time Tizen SDK installation requires about 15 GB of free disk space. Only {free_gb:.1} GB is available on this drive."
            ));
        }
        let script = resource_script(app, if cfg!(windows) {
            "tizen-sdk-install/tizen-sdk-install.ps1"
        } else {
            "tizen-sdk-install/tizen-sdk-install.sh"
        })?;
        run_sdk_install(&script, &sdk, &home)?;
        if !base_sdk_ready(&sdk) {
            return Err(format!(
                "The Samsung SDK installer returned success, but required files are missing from {} (sdk.info, tools/sdb, or tools/tizen-core/tz).",
                sdk.display()
            ));
        }
    }

    if !tv_extension_ready(&sdk) {
        let script = resource_script(app, if cfg!(windows) {
            "tizen-tv-sdk-install/tizen-tv-sdk-install.ps1"
        } else {
            "tizen-tv-sdk-install/tizen-tv-sdk-install.sh"
        })?;
        run_tv_extension_install(&script, &sdk, &home)?;
        if !tv_extension_ready(&sdk) {
            return Err(format!(
                "The TV Extension installer returned success, but its installation marker was not found in {}.",
                sdk.display()
            ));
        }
    }

    Ok(sdk)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sdk_readiness_requires_sdk_tools() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!base_sdk_ready(dir.path()));
        std::fs::create_dir_all(dir.path().join("tools/tizen-core")).unwrap();
        std::fs::write(dir.path().join("sdk.info"), "sdk").unwrap();
        std::fs::write(dir.path().join("tools/sdb"), "").unwrap();
        std::fs::write(dir.path().join("tools/tizen-core/tz"), "").unwrap();
        assert!(base_sdk_ready(dir.path()));
    }

    #[test]
    fn tv_extension_marker_is_required() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!tv_extension_ready(dir.path()));
        std::fs::write(dir.path().join(".tv-sdk-installed"), "").unwrap();
        assert!(tv_extension_ready(dir.path()));
    }
}
