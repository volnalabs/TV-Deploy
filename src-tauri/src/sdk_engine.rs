//! SDK engine (desktop): the official Tizen SDK and the pinned Samsung CLI do the work;
//! EasyTizen validates the inputs, orchestrates the tools and interprets their answers.

use crate::api::{
    fail, remember_device, success, ApiError, ApiResult, CreateCertificateArgs, FoundTv,
    ImportCertificateArgs, InstallJob, InstallResult, Outcome, ScanData, TvInfo,
};
use crate::settings::{self, Engine, SavedCertificateProfile};
use crate::toolchain::{self, NodeSource};
use crate::AppState;
use easytizen_core::cancel::CancelToken;
use easytizen_core::diagnostics::{self, Diagnosis};
use easytizen_core::package::{analyze_wgt, SignatureState};
use easytizen_core::samsung::commands::Invocation;
use easytizen_core::samsung::results::{connect_result, duid_result};
use easytizen_core::samsung::runner::{run, run_with_env, RunError, RunOptions, RunOutput};
use easytizen_core::sign::SigningIdentity;
use easytizen_core::validate::{self, parse_tv_ip};
use serde::Serialize;
use std::fs;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

#[derive(Serialize)]
pub struct ToolchainStatus {
    pub node: NodeSource,
    pub cli_ready: bool,
    pub sdk_path: Option<String>,
    pub sdk_ready: bool,
    pub tv_extension_ready: bool,
    /// Free space the Samsung installer demands for a fresh SDK (15 GB).
    pub sdk_required_gb: u32,
    /// Folder of an installation that was started but did not finish.
    pub pending_install: Option<String>,
}

pub fn toolchain_status(app: &AppHandle, state: &AppState) -> ToolchainStatus {
    let located = toolchain::locate(app);
    let (sdk, pending_install) = {
        let s = state.settings.lock().unwrap();
        (s.sdk_path.clone(), s.pending_sdk_install.clone())
    };
    let sdk_ready = sdk
        .as_deref()
        .map(|p| toolchain::sdk_is_usable(Path::new(p)))
        .unwrap_or(false);
    let tv_extension_ready = sdk
        .as_deref()
        .map(|p| toolchain::tv_sdk_is_usable(Path::new(p)))
        .unwrap_or(false);
    ToolchainStatus {
        node: located.node_source,
        cli_ready: located.cli_dir.is_some(),
        sdk_path: sdk,
        sdk_ready,
        tv_extension_ready,
        sdk_required_gb: 15,
        pending_install,
    }
}

pub fn find_existing_sdk() -> Vec<String> {
    toolchain::candidate_sdk_dirs()
        .into_iter()
        .filter(|p| toolchain::sdk_is_usable(p))
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

fn options(seconds: u64, cancel: &CancelToken) -> RunOptions {
    RunOptions {
        timeout: Duration::from_secs(seconds),
        cancel: cancel.clone(),
    }
}

fn cli_success(output: &RunOutput, code: &str) -> ApiResult<()> {
    if output.exit_code == Some(0)
        && output
            .envelope
            .as_ref()
            .map(|e| e.is_success())
            .unwrap_or(false)
    {
        return Ok(());
    }
    let detail = output
        .envelope
        .as_ref()
        .map(|e| diagnostics::technical_detail(e, None))
        .unwrap_or_else(|| output.stderr.chars().take(2500).collect::<String>());
    Err(ApiError::with(code, detail))
}

fn run_error(code: &str, e: RunError) -> ApiError {
    match e {
        RunError::Cancelled => ApiError::new("cancelled"),
        other => ApiError::with(code, other.to_string()),
    }
}

async fn init_sdk(app: &AppHandle, state: &AppState, path: &str) -> ApiResult<()> {
    validate::check_cli_path(path)?;
    let tc = toolchain::build(app, &state.data_dir, Some(path)).map_err(ApiError::new)?;
    let inv = Invocation::sdk_init(path)?;
    let out = tauri::async_runtime::spawn_blocking(move || {
        run(&tc, &inv, &options(60, &CancelToken::new()))
    })
    .await
    .map_err(|_| ApiError::new("internal"))?
    .map_err(|e| run_error("sdk_init_failed", e))?;
    cli_success(&out, "sdk_init_failed")
}

pub async fn set_sdk_path(app: &AppHandle, state: &AppState, path: String) -> ApiResult<()> {
    validate::check_cli_path(&path)?;
    if !toolchain::sdk_is_usable(Path::new(&path)) {
        return Err(ApiError::new("sdk_invalid"));
    }
    init_sdk(app, state, &path).await?;
    let mut s = state.settings.lock().unwrap();
    s.sdk_path = Some(path);
    s.pending_sdk_install = None;
    settings::save(&state.data_dir, &s).map_err(|_| ApiError::new("io_error"))
}

#[derive(Serialize, Clone)]
struct SdkProgressEvent {
    op_id: Option<String>,
    #[serde(flatten)]
    progress: crate::sdk_installer::InstallProgress,
}

/// Installs the official Tizen SDK baseline and Samsung TV Extension with the pinned Samsung
/// installer scripts, reporting live progress (`sdk-progress` events). Cancelling stops the
/// installer; the folder is remembered so the next attempt resumes it.
pub async fn install_sdk(
    app: &AppHandle,
    state: &AppState,
    path: Option<String>,
    op_id: Option<String>,
    cancel: CancelToken,
) -> ApiResult<String> {
    let requested = {
        let s = state.settings.lock().unwrap();
        path.or_else(|| s.pending_sdk_install.clone())
            .or_else(|| s.sdk_path.clone())
    };
    let check = crate::sdk_installer::preflight(requested.as_deref());
    if let Some(reason) = check.path_error {
        return Err(ApiError::new(&format!("sdk_path_{reason}")));
    }
    if !check.enough_space {
        return Err(ApiError::new("sdk_no_space"));
    }
    if !check.shell_ok {
        return Err(ApiError::new("sdk_needs_bash"));
    }
    {
        let mut s = state.settings.lock().unwrap();
        s.pending_sdk_install = Some(check.path.clone());
        let _ = settings::save(&state.data_dir, &s);
    }

    let worker_app = app.clone();
    let target = check.path.clone();
    let installed = tauri::async_runtime::spawn_blocking(move || {
        crate::sdk_installer::install(&worker_app, Some(&target), &cancel, &|progress| {
            let _ = worker_app.emit(
                "sdk-progress",
                SdkProgressEvent {
                    op_id: op_id.clone(),
                    progress,
                },
            );
        })
    })
    .await
    .map_err(|_| ApiError::new("internal"))?
    .map_err(|detail| {
        if detail == crate::sdk_installer::CANCELLED {
            ApiError::new("cancelled")
        } else {
            ApiError::with("sdk_install_failed", detail)
        }
    })?;

    let path_string = installed.to_string_lossy().into_owned();
    init_sdk(app, state, &path_string).await?;
    let mut s = state.settings.lock().unwrap();
    s.sdk_path = Some(path_string.clone());
    s.pending_sdk_install = None;
    settings::save(&state.data_dir, &s).map_err(|_| ApiError::new("io_error"))?;
    Ok(path_string)
}

// ---------- TVs ----------

fn explain(out: &Result<RunOutput, RunError>) -> (Diagnosis, Option<String>) {
    match out {
        Err(e) => (diagnostics::from_run_error(e), None),
        Ok(o) => match &o.envelope {
            Some(env) => (
                diagnostics::from_envelope(env).unwrap_or(Diagnosis::Unknown),
                Some(diagnostics::technical_detail(
                    env,
                    std::env::var("HOME")
                        .or_else(|_| std::env::var("USERPROFILE"))
                        .ok()
                        .as_deref(),
                )),
            ),
            None => (
                Diagnosis::CliFailure,
                Some(diagnostics::redact(
                    &o.stderr.chars().take(2000).collect::<String>(),
                )),
            ),
        },
    }
}

fn toolchain_or_diagnosis(
    app: &AppHandle,
    state: &AppState,
) -> Result<easytizen_core::samsung::runner::Toolchain, Diagnosis> {
    let sdk = state.settings.lock().unwrap().sdk_path.clone();
    toolchain::build(app, &state.data_dir, sdk.as_deref()).map_err(|code| match code {
        "sdk_not_set" => Diagnosis::ToolsMissing,
        _ => Diagnosis::NodeProblem,
    })
}

pub async fn scan(
    app: &AppHandle,
    state: &AppState,
    subnet: Option<String>,
    cancel: CancelToken,
) -> ApiResult<Outcome<ScanData>> {
    let (port, timeout_ms) = {
        let s = state.settings.lock().unwrap();
        (s.default_port, s.timeout_ms)
    };
    let tc = match toolchain_or_diagnosis(app, state) {
        Ok(tc) => tc,
        Err(d) => return Ok(fail(d, None)),
    };
    let inv = Invocation::scan(subnet.as_deref(), port, timeout_ms.max(500))?;
    let out = tauri::async_runtime::spawn_blocking(move || run(&tc, &inv, &options(120, &cancel)))
        .await
        .map_err(|_| ApiError::new("internal"))?;
    if let Ok(o) = &out {
        if let Some(env) = o.envelope.as_ref().filter(|e| e.is_success()) {
            let r = env.result.clone().unwrap_or_default();
            let devices = r["devices"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|d| {
                            let ip = d["ip"].as_str()?;
                            parse_tv_ip(ip).ok()?;
                            Some(FoundTv {
                                ip: ip.to_string(),
                                port: u16::try_from(d["port"].as_u64()?).ok()?,
                                name: None,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            let subnets = r["subnets_scanned"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|s| s.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            return Ok(success(ScanData { devices, subnets }));
        }
    }
    let (d, t) = explain(&out);
    Ok(fail(d, t))
}

pub async fn connect(
    app: &AppHandle,
    state: &AppState,
    ip: Ipv4Addr,
    port: u16,
    cancel: CancelToken,
) -> ApiResult<Outcome<TvInfo>> {
    let tc = match toolchain_or_diagnosis(app, state) {
        Ok(tc) => tc,
        Err(d) => return Ok(fail(d, None)),
    };
    let (tc2, c2) = (tc.clone(), cancel.clone());
    let conn = tauri::async_runtime::spawn_blocking(move || {
        run(&tc2, &Invocation::connect(ip, port), &options(60, &c2))
    })
    .await
    .map_err(|_| ApiError::new("internal"))?;
    let connected = conn
        .as_ref()
        .ok()
        .and_then(|o| o.envelope.as_ref())
        .filter(|e| e.is_success())
        .and_then(connect_result);
    let Some(dev) = connected else {
        let (d, t) = explain(&conn);
        // Port is open but sdb did not confirm a connection.
        let d = if d == Diagnosis::Unknown {
            Diagnosis::ConnectionRejected
        } else {
            d
        };
        return Ok(fail(d, t));
    };

    // DUID (the connection itself is already confirmed by sdb).
    let duid = tauri::async_runtime::spawn_blocking(move || {
        run(
            &tc,
            &Invocation::acquire_duid(ip, port),
            &options(60, &cancel),
        )
    })
    .await
    .map_err(|_| ApiError::new("internal"))?
    .ok()
    .and_then(|o| o.envelope)
    .filter(|e| e.is_success())
    .and_then(|e| duid_result(&e))
    .map(|d| d.duid);

    let name = remember_device(state, &dev.ip, dev.port, duid.clone(), None);
    let duid = duid.or_else(|| {
        let s = state.settings.lock().unwrap();
        s.devices
            .iter()
            .find(|d| d.ip == dev.ip && d.port == dev.port)
            .and_then(|d| d.duid.clone())
    });
    Ok(success(TvInfo {
        ip: dev.ip,
        port: dev.port,
        duid,
        name,
        model: None,
        host_ip: None,
    }))
}

pub async fn disconnect(
    app: &AppHandle,
    state: &AppState,
    ip: Ipv4Addr,
    port: u16,
) -> ApiResult<bool> {
    let sdk = state.settings.lock().unwrap().sdk_path.clone();
    let tc = toolchain::build(app, &state.data_dir, sdk.as_deref()).map_err(ApiError::new)?;
    let out = tauri::async_runtime::spawn_blocking(move || {
        run(
            &tc,
            &Invocation::disconnect(ip, port),
            &options(30, &CancelToken::new()),
        )
    })
    .await
    .map_err(|_| ApiError::new("internal"))?;
    Ok(matches!(&out, Ok(o) if o.envelope.as_ref().map(|e| e.is_success()).unwrap_or(false)))
}

// ---------- certificates ----------

fn sdk_data_dir(sdk: &Path) -> PathBuf {
    let info = fs::read_to_string(sdk.join("sdk.info")).unwrap_or_default();
    if let Some(value) = info.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        (key.trim() == "TIZEN_SDK_DATA_PATH").then(|| value.trim().trim_matches('"').to_string())
    }) {
        let path = PathBuf::from(value);
        if path.is_absolute() {
            return path;
        }
    }
    let name = sdk
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("tizen-sdk");
    sdk.parent().unwrap_or(sdk).join(format!("{name}-data"))
}

fn ready_toolchain(
    app: &AppHandle,
    state: &AppState,
) -> ApiResult<easytizen_core::samsung::runner::Toolchain> {
    let sdk = state
        .settings
        .lock()
        .unwrap()
        .sdk_path
        .clone()
        .ok_or_else(|| ApiError::new("sdk_not_set"))?;
    if !toolchain::tv_sdk_is_usable(Path::new(&sdk)) {
        return Err(ApiError::new("tv_extension_missing"));
    }
    toolchain::build(app, &state.data_dir, Some(&sdk)).map_err(ApiError::new)
}

/// Creates a Samsung author + distributor certificate and a signing profile through the
/// official certificate manager. Samsung Account sign-in happens in the system browser.
/// A flow interrupted after the author certificate is resumed, not restarted.
pub async fn create_certificate(
    app: &AppHandle,
    state: &AppState,
    args: &CreateCertificateArgs,
    cancel: CancelToken,
) -> ApiResult<SavedCertificateProfile> {
    let tc = ready_toolchain(app, state)?;
    let duid_list = args.duids.join(",");
    let author = Invocation::generate_samsung_author(&args.profile_name, &args.identity)?;
    let distributor = Invocation::generate_samsung_distributor(&args.profile_name, &duid_list)?;
    let create_profile = Invocation::create_samsung_profile(&args.profile_name)?;
    let password_env = vec![(
        "TIZEN_CERTIFICATE_PASSWORD".to_string(),
        args.password.clone(),
    )];
    let (name, identity, duids) = (
        args.profile_name.clone(),
        args.identity.clone(),
        args.duids.clone(),
    );
    let worker_name = name.clone();
    let worker_duids = duids.clone();

    tauri::async_runtime::spawn_blocking(move || -> ApiResult<()> {
        let profile_dir = sdk_data_dir(&tc.sdk_path)
            .join("keystore")
            .join("samsung")
            .join(&worker_name);
        let author_p12 = profile_dir.join("author.p12");
        let distributor_p12 = profile_dir.join("distributor.p12");

        if !author_p12.exists() {
            let out = run_with_env(&tc, &author, &options(7 * 60, &cancel), &password_env)
                .map_err(|e| run_error("certificate_failed", e))?;
            cli_success(&out, "certificate_failed")?;
        }
        if !distributor_p12.exists() {
            let out = run_with_env(&tc, &distributor, &options(7 * 60, &cancel), &password_env)
                .map_err(|e| run_error("certificate_failed", e))?;
            cli_success(&out, "certificate_failed")?;
        } else {
            let csr = fs::read_to_string(profile_dir.join("distributor.csr")).unwrap_or_default();
            if !worker_duids
                .iter()
                .all(|d| csr.contains(&format!("URN:tizen:deviceid={d}")))
            {
                return Err(ApiError::new("profile_duid_mismatch"));
            }
        }
        let out = run(&tc, &create_profile, &options(90, &cancel))
            .map_err(|e| run_error("certificate_failed", e))?;
        cli_success(&out, "certificate_failed")
    })
    .await
    .map_err(|_| ApiError::new("internal"))??;

    Ok(SavedCertificateProfile::new(
        name,
        identity,
        duids,
        Engine::Sdk,
    ))
}

/// Imports an existing author + distributor pair into the SDK keystore with the official
/// certificate manager. The keystores are first opened locally to check the passwords and to
/// read which TVs the distributor certificate covers.
pub async fn import_certificate(
    app: &AppHandle,
    state: &AppState,
    args: &ImportCertificateArgs,
) -> ApiResult<SavedCertificateProfile> {
    let tc = ready_toolchain(app, state)?;
    for source in [&args.author_source, &args.distributor_source] {
        validate::check_cli_path(source)?;
    }
    let name = args.profile_name.clone();
    let (author_source, distributor_source) =
        (args.author_source.clone(), args.distributor_source.clone());
    let (author_password, distributor_password) = (
        args.author_password.clone(),
        args.distributor_password.clone(),
    );

    tauri::async_runtime::spawn_blocking(move || -> ApiResult<SavedCertificateProfile> {
        let read = |path: &str| -> ApiResult<Vec<u8>> {
            let meta = fs::metadata(path).map_err(|_| ApiError::new("cert_file_invalid"))?;
            if !meta.is_file() || meta.len() > 256 * 1024 {
                return Err(ApiError::new("cert_file_invalid"));
            }
            fs::read(path).map_err(|_| ApiError::new("cert_file_invalid"))
        };
        let open = |role: &str, path: &str, password: &str| -> ApiResult<SigningIdentity> {
            SigningIdentity::from_p12(&read(path)?, password).map_err(|e| {
                let code = match e {
                    easytizen_core::sign::IdentityError::BadPasswordOrFormat => {
                        "cert_password_wrong"
                    }
                    _ => "cert_file_invalid",
                };
                ApiError::new(&format!("{role}_{code}"))
            })
        };
        let author = open("author", &author_source, &author_password)?;
        let distributor = open("distributor", &distributor_source, &distributor_password)?;
        if distributor.info.duids.is_empty() {
            return Err(ApiError::new("not_a_distributor_certificate"));
        }
        if !author.info.duids.is_empty() {
            return Err(ApiError::new("not_an_author_certificate"));
        }
        for (kind, source, password) in [
            ("author", &author_source, &author_password),
            ("distributor", &distributor_source, &distributor_password),
        ] {
            let inv = Invocation::import_samsung_certificate(&name, source, kind)?;
            let env = vec![("TIZEN_CERTIFICATE_PASSWORD".to_string(), password.clone())];
            let out = run_with_env(&tc, &inv, &options(120, &CancelToken::new()), &env)
                .map_err(|e| run_error("certificate_failed", e))?;
            cli_success(&out, "certificate_failed")?;
        }
        let out = run(
            &tc,
            &Invocation::create_samsung_profile(&name)?,
            &options(90, &CancelToken::new()),
        )
        .map_err(|e| run_error("certificate_failed", e))?;
        cli_success(&out, "certificate_failed")?;

        let mut profile = SavedCertificateProfile::new(
            name.clone(),
            author.info.subject.clone(),
            distributor.info.duids.clone(),
            Engine::Sdk,
        );
        profile.expires_at = Some(author.info.not_after.min(distributor.info.not_after));
        profile.imported = true;
        Ok(profile)
    })
    .await
    .map_err(|_| ApiError::new("internal"))?
}

pub async fn remove_certificate(
    app: &AppHandle,
    state: &AppState,
    profile_name: &str,
) -> ApiResult<()> {
    let sdk = state
        .settings
        .lock()
        .unwrap()
        .sdk_path
        .clone()
        .ok_or_else(|| ApiError::new("sdk_not_set"))?;
    let tc = toolchain::build(app, &state.data_dir, Some(&sdk)).map_err(ApiError::new)?;
    let invocation = Invocation::remove_samsung_profile(profile_name)?;
    let out = tauri::async_runtime::spawn_blocking(move || {
        run(&tc, &invocation, &options(60, &CancelToken::new()))
    })
    .await
    .map_err(|_| ApiError::new("internal"))?
    .map_err(|e| run_error("certificate_failed", e))?;
    cli_success(&out, "certificate_failed")
}

// ---------- install ----------

#[derive(Serialize, Clone)]
struct InstallProgress {
    op_id: Option<String>,
    stage: &'static str,
    fraction: f32,
}

pub async fn install(
    app: &AppHandle,
    state: &AppState,
    job: InstallJob,
) -> ApiResult<InstallResult> {
    let InstallJob {
        args,
        report,
        profile,
        ip,
        port,
        op_id,
        cancel,
    } = job;
    let tc = ready_toolchain(app, state)?;
    let staging = state.cache_dir.join("signed-packages");
    let source = PathBuf::from(&args.package_path);
    let serial = format!("{ip}:{port}");
    let run_after_install = args.run_after_install;
    let app = app.clone();

    tauri::async_runtime::spawn_blocking(move || -> ApiResult<()> {
        let emit = |stage: &'static str| {
            let _ = app.emit("install-progress", InstallProgress { op_id: op_id.clone(), stage, fraction: 0.0 });
        };
        let package = match &profile {
            Some(profile) => {
                emit("signing");
                fs::create_dir_all(&staging).map_err(|_| ApiError::new("io_error"))?;
                let staged = staging.join(format!("easytizen-{}.wgt", crate::now_millis()));
                fs::copy(&source, &staged).map_err(|_| ApiError::new("package_copy_failed"))?;
                let tz = tc.sdk_path.join("tools").join("tizen-core").join(if cfg!(windows) { "tz.exe" } else { "tz" });
                let mut signer = Command::new(&tz);
                signer
                    .args(["pack", "-b"])
                    .arg(&staged)
                    .args(["-t", "wgt", "-s"])
                    .arg(&profile.name)
                    .env("HOME", &tc.home_dir)
                    .env("USERPROFILE", &tc.home_dir)
                    .env_remove("TIZEN_SDK_PATH")
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped());
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    signer.creation_flags(0x0800_0000);
                }
                let signed = signer.output().map_err(|e| ApiError::with("sign_failed", format!("Could not start Tizen signer: {}", e.kind())));
                let signed = match signed {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = fs::remove_file(&staged);
                        return Err(e);
                    }
                };
                if !signed.status.success() {
                    let mut detail = String::from_utf8_lossy(&signed.stderr).to_string();
                    if detail.trim().is_empty() {
                        detail = String::from_utf8_lossy(&signed.stdout).to_string();
                    }
                    let _ = fs::remove_file(&staged);
                    return Err(ApiError::with("sign_failed", detail));
                }
                // Do not trust the exit code alone: both signature documents must be there.
                let ok = analyze_wgt(&staged).map(|r| r.signature == SignatureState::SignedStructure).unwrap_or(false);
                if !ok {
                    let _ = fs::remove_file(&staged);
                    return Err(ApiError::with(
                        "sign_failed",
                        "The Tizen signer returned success, but the package does not contain both signature files.",
                    ));
                }
                staged
            }
            None => source.clone(),
        };
        let is_working_copy = package != source;
        let finish = |result: ApiResult<()>| {
            if is_working_copy {
                let _ = fs::remove_file(&package);
            }
            result
        };
        if cancel.is_cancelled() {
            return finish(Err(ApiError::new("cancelled")));
        }

        emit("installing");
        let invocation = match package.to_str().ok_or_else(|| ApiError::new("invalid_path")).and_then(|p| {
            Invocation::install_package(p, &serial, run_after_install).map_err(ApiError::from)
        }) {
            Ok(i) => i,
            Err(e) => return finish(Err(e)),
        };
        let out = run(&tc, &invocation, &options(5 * 60, &cancel)).map_err(|e| run_error("install_failed", e));
        finish(out.and_then(|o| {
            cli_success(&o, "install_failed").map_err(|e| {
                let low = e.detail.as_deref().unwrap_or("").to_ascii_lowercase();
                let code = if low.contains("author") && (low.contains("match") || low.contains("different")) {
                    "tv_author_mismatch"
                } else if low.contains("certificate") || low.contains("signature") {
                    "tv_cert_rejected"
                } else {
                    "install_failed"
                };
                ApiError { code: code.into(), detail: e.detail }
            })
        }))
    })
    .await
    .map_err(|_| ApiError::new("internal"))??;

    Ok(InstallResult {
        app_name: report.app_name,
        app_id: report.app_id,
        version: report.version,
        tv_ip: ip.to_string(),
        launched: args.run_after_install.then_some(true),
    })
}
