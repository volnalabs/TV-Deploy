//! Starts the pinned Samsung CLI with the bundled Node.js runtime.
//!
//! Blocking by design (std only) so it is trivially unit-testable; the Tauri layer runs it on
//! a blocking thread pool. Supports timeout and cooperative cancellation; stdout/stderr are
//! captured with a hard size cap.

use super::commands::Invocation;
use super::envelope::{parse_envelope, Envelope};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;

const MAX_CAPTURE: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Debug, Clone)]
pub struct Toolchain {
    /// Path to the `node` executable (bundled runtime, or an explicitly chosen one).
    pub node: PathBuf,
    /// Directory containing the vendored `*-cli.js` files (`resources/samsung-cli/lib/cli`).
    pub cli_dir: PathBuf,
    /// Tizen SDK directory (used for pre-checks and for `sdk-init`). NOT passed via env:
    /// the Samsung scripts ignore `TIZEN_SDK_PATH` and read `<HOME>/.tizen.sdk.path.config`.
    pub sdk_path: PathBuf,
    /// Private HOME for the Samsung scripts (`HOME` + `USERPROFILE`). Keeps TV Deploy's SDK
    /// pointer, certificates and profiles separate from a user's own Tizen Studio setup.
    pub home_dir: PathBuf,
}

#[derive(Debug, Error)]
pub enum RunError {
    #[error("could not start Node.js: {0}")]
    Spawn(String),
    #[error("operation timed out after {0:?}")]
    Timeout(Duration),
    #[error("operation was cancelled")]
    Cancelled,
    #[error("Samsung CLI script is missing from the installation")]
    ScriptMissing,
}

#[derive(Debug)]
pub struct RunOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    /// `None` when the script printed no recognisable envelope.
    pub envelope: Option<Envelope>,
}

pub struct RunOptions {
    pub timeout: Duration,
    pub cancel: CancelToken,
}

fn drain<R: Read + Send + 'static>(mut r: R) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut out = Vec::new();
        let mut buf = [0u8; 8192];
        loop {
            match r.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if out.len() < MAX_CAPTURE {
                        let take = n.min(MAX_CAPTURE - out.len());
                        out.extend_from_slice(&buf[..take]);
                    }
                }
            }
        }
        String::from_utf8_lossy(&out).into_owned()
    })
}

pub fn run(tc: &Toolchain, inv: &Invocation, opts: &RunOptions) -> Result<RunOutput, RunError> {
    run_with_env(tc, inv, opts, &[])
}

/// Run a typed Samsung CLI invocation with a small, caller-supplied environment overlay.
///
/// This is used for Samsung's documented hidden-password prompt: the UI enters the
/// certificate password, Rust passes it as TIZEN_CERTIFICATE_PASSWORD to the child
/// environment, and the password never appears in argv, logs, or persisted settings.
pub fn run_with_env(
    tc: &Toolchain,
    inv: &Invocation,
    opts: &RunOptions,
    environment: &[(String, String)],
) -> Result<RunOutput, RunError> {
    let script = tc.cli_dir.join(inv.script.file_name());
    if !script.is_file() {
        return Err(RunError::ScriptMissing);
    }
    if opts.cancel.is_cancelled() {
        return Err(RunError::Cancelled);
    }
    let mut cmd = Command::new(&tc.node);
    cmd.arg(&script)
        .args(&inv.args)
        .env("HOME", &tc.home_dir)
        .env("USERPROFILE", &tc.home_dir)
        .env_remove("TIZEN_SDK_PATH")
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in environment {
        cmd.env(key, value);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| RunError::Spawn(e.kind().to_string()))?;
    let out_h = drain(child.stdout.take().expect("piped"));
    let err_h = drain(child.stderr.take().expect("piped"));

    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => {}
            Err(e) => return Err(RunError::Spawn(e.kind().to_string())),
        }
        if opts.cancel.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = out_h.join();
            let _ = err_h.join();
            return Err(RunError::Cancelled);
        }
        if start.elapsed() >= opts.timeout {
            let _ = child.kill();
            let _ = child.wait();
            let _ = out_h.join();
            let _ = err_h.join();
            return Err(RunError::Timeout(opts.timeout));
        }
        thread::sleep(Duration::from_millis(25));
    };
    let stdout = out_h.join().unwrap_or_default();
    let stderr = err_h.join().unwrap_or_default();
    let envelope = parse_envelope(&stdout);
    Ok(RunOutput {
        exit_code: status.code(),
        stdout,
        stderr,
        envelope,
    })
}

/// Locates `sdb` inside an SDK directory (same location the Samsung CLI itself uses:
/// `<sdk>/tools/sdb`).
pub fn sdb_path(sdk: &Path) -> PathBuf {
    let name = if cfg!(windows) { "sdb.exe" } else { "sdb" };
    sdk.join("tools").join(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::samsung::commands::Script;

    // NOTE: these tests run the real Node.js executable against a tiny *mock* CLI script. They
    // verify the process plumbing (timeout, cancel, capture, parsing) — NOT any Samsung or TV
    // behaviour.
    fn node() -> Option<PathBuf> {
        std::env::var_os("PATH").and_then(|p| {
            std::env::split_paths(&p)
                .map(|d| d.join(if cfg!(windows) { "node.exe" } else { "node" }))
                .find(|c| c.is_file())
        })
    }

    fn setup(js: &str) -> Option<(tempfile::TempDir, Toolchain)> {
        let node = node()?;
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join(Script::CheckNode.file_name()), js).unwrap();
        let tc = Toolchain {
            node,
            cli_dir: d.path().to_path_buf(),
            sdk_path: d.path().join("sdk"),
            home_dir: d.path().join("home"),
        };
        Some((d, tc))
    }

    fn opts(ms: u64) -> RunOptions {
        RunOptions {
            timeout: Duration::from_millis(ms),
            cancel: CancelToken::new(),
        }
    }

    #[test]
    fn captures_and_parses() {
        let Some((_d, tc)) = setup(
            r#"console.error("progress"); console.log(JSON.stringify({status:"success",result:{home:process.env.HOME.endsWith('home'),sdkenv:process.env.TIZEN_SDK_PATH===undefined}}))"#,
        ) else {
            return;
        };
        let out = run(&tc, &Invocation::check_node(), &opts(10_000)).unwrap();
        assert_eq!(out.exit_code, Some(0));
        assert!(out.stderr.contains("progress"));
        assert!(out.envelope.unwrap().is_success());
    }

    #[test]
    fn times_out() {
        let Some((_d, tc)) = setup("setInterval(()=>{},1000)") else {
            return;
        };
        let t = Instant::now();
        assert!(matches!(
            run(&tc, &Invocation::check_node(), &opts(300)),
            Err(RunError::Timeout(_))
        ));
        assert!(t.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn cancels() {
        let Some((_d, tc)) = setup("setInterval(()=>{},1000)") else {
            return;
        };
        let o = opts(30_000);
        let c = o.cancel.clone();
        let h = thread::spawn(move || {
            thread::sleep(Duration::from_millis(200));
            c.cancel();
        });
        assert!(matches!(
            run(&tc, &Invocation::check_node(), &o),
            Err(RunError::Cancelled)
        ));
        h.join().unwrap();
    }

    #[test]
    fn unexpected_exit_without_envelope() {
        let Some((_d, tc)) = setup("process.exit(3)") else {
            return;
        };
        let out = run(&tc, &Invocation::check_node(), &opts(10_000)).unwrap();
        assert_eq!(out.exit_code, Some(3));
        assert!(out.envelope.is_none());
    }

    #[test]
    fn missing_script_and_bad_node() {
        let Some((d, mut tc)) = setup("") else { return };
        assert!(matches!(
            run(
                &tc,
                &Invocation::scan(None, 26101, 1000).unwrap(),
                &opts(1000)
            ),
            Err(RunError::ScriptMissing)
        ));
        tc.node = d.path().join("no-such-node");
        assert!(matches!(
            run(&tc, &Invocation::check_node(), &opts(1000)),
            Err(RunError::Spawn(_))
        ));
    }
}

#[cfg(all(test, unix))]
mod e2e_mock_sdb {
    //! Runs the REAL vendored Samsung scripts against a MOCK `sdb` shell script. Confirms our
    //! plumbing (private HOME, sdk-init, argv, envelope parsing) end to end. It does NOT test
    //! a TV, Developer Mode, or real sdb behaviour.
    use super::*;
    use crate::samsung::commands::Invocation;
    use crate::samsung::results::{connect_result, duid_result};

    fn repo() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn init_connect_duid_through_real_scripts() {
        let Some(node) = std::env::var_os("PATH").and_then(|p| {
            std::env::split_paths(&p)
                .map(|d| d.join("node"))
                .find(|c| c.is_file())
        }) else {
            return;
        };
        let home = tempfile::tempdir().unwrap();
        let sdk = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(sdk.path().join("tools")).unwrap();
        let mock = repo().join("crates/tvdeploy-core/tests/fixtures/mock-sdk/tools/sdb");
        std::fs::copy(&mock, sdk.path().join("tools/sdb")).unwrap();
        let tc = Toolchain {
            node,
            cli_dir: repo().join("resources/samsung-cli/lib/cli"),
            sdk_path: sdk.path().to_path_buf(),
            home_dir: home.path().to_path_buf(),
        };
        let o = || RunOptions {
            timeout: Duration::from_secs(60),
            cancel: CancelToken::new(),
        };

        let init = run(
            &tc,
            &Invocation::sdk_init(sdk.path().to_str().unwrap()).unwrap(),
            &o(),
        )
        .unwrap();
        assert!(
            init.envelope.as_ref().unwrap().is_success(),
            "{}",
            init.stdout
        );
        assert!(home.path().join(".tizen.sdk.path.config").is_file());

        let ip = "192.168.1.5".parse().unwrap();
        let c = run(&tc, &Invocation::connect(ip, 26101), &o()).unwrap();
        assert_eq!(
            connect_result(c.envelope.as_ref().unwrap()).unwrap().serial,
            "192.168.1.5:26101"
        );

        let d = run(&tc, &Invocation::acquire_duid(ip, 26101), &o()).unwrap();
        assert_eq!(
            duid_result(d.envelope.as_ref().unwrap()).unwrap().duid,
            "MOCKDUID1234ABCD"
        );
    }
}
