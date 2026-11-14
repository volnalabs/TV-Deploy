//! Cross-platform bootstrap for the official Samsung Tizen SDK and TV Extension.
//! The exact PowerShell/Bash installers are vendored from the pinned Samsung commit
//! declared in resources/samsung-cli/PINNED.json. No SDK packages are mirrored by EasyTizen.

use easytizen_core::cancel::CancelToken;
use fs2::available_space;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
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

/// One line of installer output, interpreted for the UI.
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct InstallProgress {
    /// "sdk" (base Tizen SDK) or "tv_extension" (Samsung TV Extension).
    pub phase: &'static str,
    /// Current installer step, e.g. "Installing Tizen platform packages".
    pub step: Option<String>,
    /// Package counter printed by the installer as `[current/total]`.
    pub current: Option<u32>,
    pub total: Option<u32>,
    pub line: String,
}

/// Extracts `=== Step ===` headings and `[n/total]` counters from an installer line.
pub fn parse_progress_line(line: &str) -> (Option<String>, Option<(u32, u32)>) {
    let trimmed = line.trim();
    let step = trimmed
        .find("=== ")
        .and_then(|start| {
            let rest = &trimmed[start + 4..];
            rest.find(" ===").map(|end| rest[..end].trim().to_string())
        })
        .filter(|s| !s.is_empty());
    let mut counter = None;
    let mut rest = trimmed;
    while let Some(start) = rest.find('[') {
        let after = &rest[start + 1..];
        let Some(end) = after.find(']') else { break };
        if let Some((current, total)) = after[..end].split_once('/') {
            if let (Ok(current), Ok(total)) =
                (current.trim().parse::<u32>(), total.trim().parse::<u32>())
            {
                if total > 0 && current <= total {
                    counter = Some((current, total));
                    break;
                }
            }
        }
        rest = &after[end + 1..];
    }
    (step, counter)
}

/// Stops the installer and everything it started (downloads, extraction).
fn kill_tree(child: &mut std::process::Child) {
    let pid = child.id();
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .creation_flags(0x0800_0000)
            .output();
    }
    #[cfg(not(windows))]
    {
        // The installer runs in its own process group (see run_checked).
        let _ = Command::new("kill")
            .args(["-s", "KILL", "--", &format!("-{pid}")])
            .output();
    }
    let _ = child.kill();
    let _ = child.wait();
}

pub const CANCELLED: &str = "cancelled";

fn run_checked(
    mut command: Command,
    step: &str,
    phase: &'static str,
    timeout: Duration,
    cancel: &CancelToken,
    on_progress: &dyn Fn(InstallProgress),
) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }

    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|e| format!("{step}: could not start installer ({})", e.kind()))?;
    let (sender, receiver) = std::sync::mpsc::channel::<String>();
    let mut readers = Vec::new();
    for stream in [
        child
            .stdout
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
        child
            .stderr
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
    ]
    .into_iter()
    .flatten()
    {
        let sender = sender.clone();
        readers.push(thread::spawn(move || {
            use std::io::BufRead;
            let mut reader = std::io::BufReader::new(stream);
            let mut raw = Vec::new();
            loop {
                raw.clear();
                match reader.read_until(b'\n', &mut raw) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let line: String = String::from_utf8_lossy(&raw)
                            .trim_end()
                            .chars()
                            .take(500)
                            .collect();
                        if !line.is_empty() && sender.send(line).is_err() {
                            break;
                        }
                    }
                }
            }
        }));
    }
    drop(sender);

    let start = Instant::now();
    let mut tail: std::collections::VecDeque<String> = std::collections::VecDeque::new();
    let mut current_step: Option<String> = None;
    let mut handle = |line: String, tail: &mut std::collections::VecDeque<String>| {
        let (heading, counter) = parse_progress_line(&line);
        if heading.is_some() {
            current_step = heading;
        }
        on_progress(InstallProgress {
            phase,
            step: current_step.clone(),
            current: counter.map(|c| c.0),
            total: counter.map(|c| c.1),
            line: line.clone(),
        });
        tail.push_back(line);
        if tail.len() > 40 {
            tail.pop_front();
        }
    };

    let status = loop {
        while let Ok(line) = receiver.try_recv() {
            handle(line, &mut tail);
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                kill_tree(&mut child);
                return Err(format!(
                    "{step}: failed while waiting for installer ({})",
                    error.kind()
                ));
            }
        }
        if cancel.is_cancelled() {
            kill_tree(&mut child);
            return Err(CANCELLED.into());
        }
        if start.elapsed() >= timeout {
            kill_tree(&mut child);
            return Err(format!("{step}: installer exceeded the {} minute time limit. You can retry; Samsung's installer keeps its resume markers.", timeout.as_secs() / 60));
        }
        thread::sleep(Duration::from_millis(150));
    };
    for reader in readers {
        let _ = reader.join();
    }
    while let Ok(line) = receiver.try_recv() {
        handle(line, &mut tail);
    }

    if !status.success() {
        let detail = tail_text(&tail.into_iter().collect::<Vec<_>>().join("\n"), 3000);
        return Err(format!(
            "{step} failed with exit code {:?}. {}",
            status.code(),
            if detail.is_empty() {
                "No diagnostic output was returned.".into()
            } else {
                detail
            }
        ));
    }
    Ok(())
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
            let major = text.split_whitespace().find_map(|part| {
                let clean = part
                    .trim_start_matches("version")
                    .trim_matches(|c: char| !c.is_ascii_digit() && c != '.');
                clean.split('.').next()?.parse::<u32>().ok()
            });
            if major.map(|v| v >= 4).unwrap_or(false) {
                return Ok(candidate);
            }
        }
    }

    #[cfg(target_os = "macos")]
    return Err(
        "Samsung's TV SDK installer requires Bash 4 or newer. macOS includes Bash 3.2. Install a current Bash with Homebrew (brew install bash), then restart EasyTizen.".into()
    );
    #[cfg(not(target_os = "macos"))]
    Err("Samsung's SDK installer requires Bash 4 or newer. Install Bash 4+ and retry.".into())
}

fn run_sdk_install(
    script: &Path,
    sdk: &Path,
    home: &Path,
    cancel: &CancelToken,
    on_progress: &dyn Fn(InstallProgress),
) -> Result<(), String> {
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
        command
            .arg(script)
            .args(["-Path"])
            .arg(sdk)
            .args(["-DownloadJobs", DOWNLOAD_JOBS]);
        command.env("USERPROFILE", home).env("HOME", home);
        run_checked(
            command,
            "Tizen SDK installation",
            "sdk",
            Duration::from_secs(3 * 60 * 60),
            cancel,
            on_progress,
        )?;
    }
    #[cfg(not(windows))]
    {
        let bash = bash4()?;
        let mut command = Command::new(bash);
        command
            .arg(script)
            .args(["--path"])
            .arg(sdk)
            .args(["--download-jobs", DOWNLOAD_JOBS]);
        command.env("HOME", home).env("USERPROFILE", home);
        run_checked(
            command,
            "Tizen SDK installation",
            "sdk",
            Duration::from_secs(3 * 60 * 60),
            cancel,
            on_progress,
        )?;
    }
    Ok(())
}

fn run_tv_extension_install(
    script: &Path,
    sdk: &Path,
    home: &Path,
    cancel: &CancelToken,
    on_progress: &dyn Fn(InstallProgress),
) -> Result<(), String> {
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
        command
            .arg(script)
            .args(["-SdkPath"])
            .arg(sdk)
            .args(["-DownloadJobs", DOWNLOAD_JOBS]);
        command.env("USERPROFILE", home).env("HOME", home);
        run_checked(
            command,
            "Samsung TV Extension installation",
            "tv_extension",
            Duration::from_secs(2 * 60 * 60),
            cancel,
            on_progress,
        )?;
    }
    #[cfg(not(windows))]
    {
        let bash = bash4()?;
        let mut command = Command::new(bash);
        command
            .arg(script)
            .arg(format!("--sdk-path={}", sdk.display()))
            .args(["--download-jobs", DOWNLOAD_JOBS]);
        command.env("HOME", home).env("USERPROFILE", home);
        run_checked(
            command,
            "Samsung TV Extension installation",
            "tv_extension",
            Duration::from_secs(2 * 60 * 60),
            cancel,
            on_progress,
        )?;
    }
    Ok(())
}

fn base_sdk_ready(path: &Path) -> bool {
    path.join("sdk.info").is_file()
        && path
            .join("tools")
            .join(if cfg!(windows) { "sdb.exe" } else { "sdb" })
            .is_file()
        && path
            .join("tools")
            .join("tizen-core")
            .join(if cfg!(windows) { "tz.exe" } else { "tz" })
            .is_file()
}

fn tv_extension_ready(path: &Path) -> bool {
    if path.join(".tv-sdk-installed").is_file() {
        return true;
    }
    std::fs::read_to_string(path.join(".platform-installed"))
        .map(|content| content.contains("TV-SAMSUNG-Public"))
        .unwrap_or(false)
}

/// What the UI shows before the user commits to a ~15 GB download.
#[derive(Debug, serde::Serialize)]
pub struct Preflight {
    pub path: String,
    pub free_bytes: Option<u64>,
    pub required_bytes: u64,
    pub enough_space: bool,
    /// The base SDK is already present at `path`; only the TV Extension may be needed.
    pub sdk_present: bool,
    pub tv_extension_present: bool,
    /// macOS/Linux: a Bash 4+ interpreter was found (the Samsung installer needs it).
    pub shell_ok: bool,
    pub path_error: Option<String>,
}

fn nearest_existing(path: &Path) -> Option<&Path> {
    let mut current = Some(path);
    while let Some(p) = current {
        if p.exists() {
            return Some(p);
        }
        current = p.parent();
    }
    None
}

/// Checks a candidate SDK folder without changing anything on disk.
pub fn preflight(requested_path: Option<&str>) -> Preflight {
    let (sdk, path_error) = match sdk_root(requested_path) {
        Ok(p) => {
            let text = p.to_string_lossy().into_owned();
            // The Samsung installer cannot work in paths with spaces or non-Latin characters.
            let error =
                (!text.is_ascii() || text.contains(' ')).then(|| "bad_characters".to_string());
            (p, error)
        }
        Err(_) => (PathBuf::new(), Some("not_absolute".to_string())),
    };
    let sdk_present = base_sdk_ready(&sdk);
    let free_bytes = nearest_existing(&sdk).and_then(|p| available_space(p).ok());
    #[cfg(windows)]
    let shell_ok = true;
    #[cfg(not(windows))]
    let shell_ok = bash4().is_ok();
    Preflight {
        path: sdk.to_string_lossy().into_owned(),
        free_bytes,
        required_bytes: REQUIRED_FREE_BYTES,
        enough_space: sdk_present
            || free_bytes
                .map(|f| f >= REQUIRED_FREE_BYTES)
                .unwrap_or(false),
        sdk_present,
        tv_extension_present: tv_extension_ready(&sdk),
        shell_ok,
        path_error,
    }
}

/// Install the base Tizen SDK and Samsung TV Extension into a user-owned directory.
/// Existing SDK installations are reused; the extension is installed only when missing.
/// Returns `Err(CANCELLED)` when the user stopped it; a later run resumes where the Samsung
/// installer left its markers.
pub fn install(
    app: &AppHandle,
    requested_path: Option<&str>,
    cancel: &CancelToken,
    on_progress: &dyn Fn(InstallProgress),
) -> Result<PathBuf, String> {
    let sdk = sdk_root(requested_path)?;
    let home = home_dir()?;
    let parent = sdk.parent().unwrap_or(&home);
    std::fs::create_dir_all(parent).map_err(|e| format!("Cannot access SDK folder: {e}"))?;

    if !base_sdk_ready(&sdk) {
        let free =
            available_space(parent).map_err(|e| format!("Cannot check free disk space: {e}"))?;
        if free < REQUIRED_FREE_BYTES {
            let free_gb = free as f64 / 1024_f64.powi(3);
            return Err(format!(
                "A first-time Tizen SDK installation requires about 15 GB of free disk space. Only {free_gb:.1} GB is available on this drive."
            ));
        }
        let script = resource_script(
            app,
            if cfg!(windows) {
                "tizen-sdk-install/tizen-sdk-install.ps1"
            } else {
                "tizen-sdk-install/tizen-sdk-install.sh"
            },
        )?;
        run_sdk_install(&script, &sdk, &home, cancel, on_progress)?;
        if !base_sdk_ready(&sdk) {
            return Err(format!(
                "The Samsung SDK installer returned success, but required files are missing from {} (sdk.info, tools/sdb, or tools/tizen-core/tz).",
                sdk.display()
            ));
        }
    }

    if !tv_extension_ready(&sdk) {
        let script = resource_script(
            app,
            if cfg!(windows) {
                "tizen-tv-sdk-install/tizen-tv-sdk-install.ps1"
            } else {
                "tizen-tv-sdk-install/tizen-tv-sdk-install.sh"
            },
        )?;
        run_tv_extension_install(&script, &sdk, &home, cancel, on_progress)?;
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
    fn progress_lines() {
        assert_eq!(
            parse_progress_line("[INFO] === Installing Tizen platform packages ==="),
            (Some("Installing Tizen platform packages".into()), None)
        );
        assert_eq!(
            parse_progress_line("2026-01-01 [INFO] [12/87] Installing sdb..."),
            (None, Some((12, 87)))
        );
        assert_eq!(parse_progress_line("[INFO] plain line"), (None, None));
        assert_eq!(parse_progress_line("[9/3] bogus"), (None, None));
    }

    #[cfg(unix)]
    #[test]
    fn streams_progress_and_cancels_the_whole_process_group() {
        let seen = std::sync::Mutex::new(Vec::new());
        let mut ok = Command::new("sh");
        ok.args([
            "-c",
            "echo '=== Step one ==='; echo '[1/2] Installing a'; echo '[2/2] Installing b' >&2",
        ]);
        run_checked(
            ok,
            "t",
            "sdk",
            Duration::from_secs(20),
            &CancelToken::new(),
            &|p| seen.lock().unwrap().push(p),
        )
        .unwrap();
        let seen = seen.into_inner().unwrap();
        assert_eq!(seen.len(), 3);
        assert!(seen
            .iter()
            .any(|p| p.current == Some(1) && p.total == Some(2)));
        assert!(seen.iter().any(|p| p.step.as_deref() == Some("Step one")));

        let mut failing = Command::new("sh");
        failing.args(["-c", "echo boom; exit 7"]);
        let err = run_checked(
            failing,
            "t",
            "sdk",
            Duration::from_secs(20),
            &CancelToken::new(),
            &|_| {},
        )
        .unwrap_err();
        assert!(
            err.contains("exit code Some(7)") && err.contains("boom"),
            "{err}"
        );

        let cancel = CancelToken::new();
        let trigger = cancel.clone();
        let marker = tempfile::tempdir().unwrap();
        let file = marker.path().join("child-survived");
        let mut slow = Command::new("sh");
        slow.arg("-c")
            .arg(format!("(sleep 3; touch '{}') & sleep 30", file.display()));
        let stopper = thread::spawn(move || {
            thread::sleep(Duration::from_millis(400));
            trigger.cancel();
        });
        let started = Instant::now();
        let err =
            run_checked(slow, "t", "sdk", Duration::from_secs(60), &cancel, &|_| {}).unwrap_err();
        stopper.join().unwrap();
        assert_eq!(err, CANCELLED);
        assert!(started.elapsed() < Duration::from_secs(10));
        thread::sleep(Duration::from_millis(3200));
        assert!(
            !file.exists(),
            "background children of the installer must be stopped too"
        );
    }

    #[test]
    fn preflight_reports_space_and_bad_paths() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("not-yet/sdk");
        let p = preflight(target.to_str());
        assert!(p.free_bytes.is_some());
        assert!(!p.sdk_present && !p.tv_extension_present);
        assert_eq!(p.required_bytes, REQUIRED_FREE_BYTES);
        assert_eq!(
            preflight(Some("relative/sdk")).path_error.as_deref(),
            Some("not_absolute")
        );
        #[cfg(unix)]
        assert_eq!(
            preflight(Some("/tmp/my sdk")).path_error.as_deref(),
            Some("bad_characters")
        );
    }

    #[test]
    fn tv_extension_marker_is_required() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!tv_extension_ready(dir.path()));
        std::fs::write(dir.path().join(".tv-sdk-installed"), "").unwrap();
        assert!(tv_extension_ready(dir.path()));
    }
}
