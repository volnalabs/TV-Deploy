//! Tauri commands. Every argument is validated here (and again inside `tvdeploy-core`) before
//! anything is executed.

use crate::settings::{self, SavedCertificateProfile, SavedDevice, Settings};
use crate::toolchain::{self, NodeSource};
use crate::AppState;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, State};
use tvdeploy_core::connectivity::{local_ip_towards, probe_tcp, Probe, MIN_DIAGNOSTIC_TIMEOUT};
use tvdeploy_core::diagnostics::{self, Diagnosis};
use tvdeploy_core::package::{analyze_wgt, PackageError, PackageReport};
use tvdeploy_core::samsung::commands::Invocation;
use tvdeploy_core::samsung::results::{connect_result, duid_result};
use tvdeploy_core::samsung::runner::{run, run_with_env, CancelToken, RunError, RunOptions, RunOutput};
use tvdeploy_core::validate::{self, parse_port, parse_tv_ip};

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub code: String,
    pub detail: Option<String>,
}

impl ApiError {
    fn new(code: &str) -> Self {
        Self {
            code: code.into(),
            detail: None,
        }
    }
}

impl From<validate::ValidationError> for ApiError {
    fn from(e: validate::ValidationError) -> Self {
        use validate::ValidationError::*;
        ApiError::new(match e {
            BadIp => "invalid_ip",
            ForbiddenIp => "forbidden_ip",
            BadPort => "invalid_port",
            BadSubnet => "invalid_subnet",
            BadSerial => "invalid_serial",
            BadPath => "invalid_path",
            BadTimeout => "invalid_timeout",
        })
    }
}

type ApiResult<T> = Result<T, ApiError>;

fn register(state: &AppState, op_id: &Option<String>) -> CancelToken {
    let t = CancelToken::new();
    if let Some(id) = op_id {
        state.ops.lock().unwrap().insert(id.clone(), t.clone());
    }
    t
}

fn unregister(state: &AppState, op_id: &Option<String>) {
    if let Some(id) = op_id {
        state.ops.lock().unwrap().remove(id);
    }
}

#[tauri::command]
pub fn cancel_operation(state: State<'_, AppState>, op_id: String) {
    if let Some(t) = state.ops.lock().unwrap().get(&op_id) {
        t.cancel();
    }
}

// ---------- toolchain ----------

#[derive(Serialize)]
pub struct ToolchainStatus {
    node: NodeSource,
    cli_ready: bool,
    sdk_path: Option<String>,
    sdk_ready: bool,
    tv_extension_ready: bool,
    /// Approximate free space the Samsung installer demands before installing a fresh SDK
    /// (reported by the Samsung CLI itself: 15 GB). Shown to users before any SDK download.
    sdk_required_gb: u32,
}

#[tauri::command]
pub fn toolchain_status(app: AppHandle, state: State<'_, AppState>) -> ToolchainStatus {
    let l = toolchain::locate(&app);
    let sdk = state.settings.lock().unwrap().sdk_path.clone();
    let sdk_ready = sdk
        .as_deref()
        .map(|p| toolchain::sdk_is_usable(std::path::Path::new(p)))
        .unwrap_or(false);
    ToolchainStatus {
        node: l.node_source,
        cli_ready: l.cli_dir.is_some(),
        sdk_path: sdk,
        tv_extension_ready: sdk.as_deref().map(|p| toolchain::tv_sdk_is_usable(std::path::Path::new(p))).unwrap_or(false),
        sdk_ready,
        sdk_required_gb: 15,
    }
}

#[tauri::command]
pub fn find_existing_sdk() -> Vec<String> {
    toolchain::candidate_sdk_dirs()
        .into_iter()
        .filter(|p| toolchain::sdk_is_usable(p))
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

#[tauri::command]
pub async fn set_sdk_path(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> ApiResult<()> {
    validate::check_cli_path(&path)?;
    let p = PathBuf::from(&path);
    if !toolchain::sdk_is_usable(&p) {
        return Err(ApiError::new("sdk_invalid"));
    }
    let tc = toolchain::build(&app, &state.data_dir, Some(&path)).map_err(ApiError::new)?;
    let inv = Invocation::sdk_init(&path)?;
    let out = tauri::async_runtime::spawn_blocking(move || {
        run(
            &tc,
            &inv,
            &RunOptions {
                timeout: Duration::from_secs(60),
                cancel: CancelToken::new(),
            },
        )
    })
    .await
    .map_err(|_| ApiError::new("internal"))?;
    let ok = matches!(&out, Ok(o) if o.envelope.as_ref().map(|e| e.is_success()).unwrap_or(false));
    if !ok {
        return Err(ApiError::new("sdk_init_failed"));
    }
    let mut s = state.settings.lock().unwrap();
    s.sdk_path = Some(path);
    settings::save(&state.data_dir, &s).map_err(|_| ApiError::new("io_error"))
}

/// Install the official Tizen SDK baseline and Samsung TV Extension. The actual
/// PowerShell/Bash installers are pinned Samsung upstream scripts; TV Deploy only
/// orchestrates them and never downloads or installs a user application.
#[tauri::command]
pub async fn install_samsung_sdk(
    app: AppHandle,
    state: State<'_, AppState>,
) -> ApiResult<String> {
    let requested = state.settings.lock().unwrap().sdk_path.clone();
    let app_for_install = app.clone();
    let path = tauri::async_runtime::spawn_blocking(move || {
        crate::sdk_installer::install(&app_for_install, requested.as_deref())
    })
    .await
    .map_err(|_| ApiError::new("internal"))?
    .map_err(|detail| ApiError { code: "sdk_install_failed".into(), detail: Some(detail) })?;

    let path_string = path.to_string_lossy().into_owned();
    validate::check_cli_path(&path_string)?;
    let tc = toolchain::build(&app, &state.data_dir, Some(&path_string)).map_err(ApiError::new)?;
    let inv = Invocation::sdk_init(&path_string)?;
    let init = tauri::async_runtime::spawn_blocking(move || {
        run(
            &tc,
            &inv,
            &RunOptions {
                timeout: Duration::from_secs(60),
                cancel: CancelToken::new(),
            },
        )
    })
    .await
    .map_err(|_| ApiError::new("internal"))?;
    let init = init.map_err(|e| ApiError { code: "sdk_init_failed".into(), detail: Some(e.to_string()) })?;
    let init_ok = init.exit_code == Some(0)
        && init.envelope.as_ref().map(|e| e.is_success()).unwrap_or(false);
    if !init_ok {
        return Err(ApiError {
            code: "sdk_init_failed".into(),
            detail: Some(diagnostics::redact(&init.stderr.chars().take(2500).collect::<String>())),
        });
    }

    let mut settings = state.settings.lock().unwrap();
    settings.sdk_path = Some(path_string.clone());
    settings::save(&state.data_dir, &settings).map_err(|_| ApiError::new("io_error"))?;
    Ok(path_string)
}

#[derive(Serialize)]
pub struct CertificateProfileInfo {
    name: String,
    identity: String,
    duid: String,
}

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
    let name = sdk.file_name().and_then(|s| s.to_str()).unwrap_or("tizen-sdk");
    sdk.parent().unwrap_or(sdk).join(format!("{name}-data"))
}

fn cli_success(output: &RunOutput, code: &str) -> ApiResult<()> {
    if output.exit_code == Some(0)
        && output.envelope.as_ref().map(|e| e.is_success()).unwrap_or(false)
    {
        return Ok(());
    }
    let detail = output
        .envelope
        .as_ref()
        .map(|e| diagnostics::technical_detail(e, None))
        .unwrap_or_else(|| diagnostics::redact(&output.stderr.chars().take(2500).collect::<String>()));
    Err(ApiError {
        code: code.into(),
        detail: if detail.is_empty() { None } else { Some(detail) },
    })
}

fn validate_cert_password(password: &str) -> ApiResult<()> {
    if password.len() < 8
        || !password.bytes().any(|c| c.is_ascii_uppercase())
        || !password.bytes().any(|c| c.is_ascii_lowercase())
        || !password.bytes().any(|c| c.is_ascii_digit())
    {
        return Err(ApiError {
            code: "cert_password_invalid".into(),
            detail: Some("Use at least 8 characters with uppercase, lowercase and a number.".into()),
        });
    }
    Ok(())
}

#[tauri::command]
pub async fn create_samsung_certificate(
    app: AppHandle,
    state: State<'_, AppState>,
    ip: String,
    port: u32,
    duid: String,
    profile_name: String,
    identity: String,
    password: String,
) -> ApiResult<CertificateProfileInfo> {
    let ip = parse_tv_ip(&ip)?;
    let port = parse_port(port)?;
    validate_cert_password(&password)?;
    if duid.trim().is_empty() || duid.len() > 256 {
        return Err(ApiError::new("invalid_duid"));
    }
    if !duid.bytes().all(|b| b.is_ascii_alphanumeric() || b"#.-:_".contains(&b)) {
        return Err(ApiError::new("invalid_duid"));
    }
    let (sdk_path, existing_profile, device_matches) = {
        let settings = state.settings.lock().unwrap();
        let device_matches = settings.devices.iter().any(|d| {
            d.ip == ip.to_string() && d.port == port && d.duid.as_deref() == Some(duid.as_str())
        });
        (
            settings.sdk_path.clone(),
            settings.certificate_profiles.iter().find(|p| p.name == profile_name).cloned(),
            device_matches,
        )
    };
    if !device_matches {
        return Err(ApiError {
            code: "duid_not_verified".into(),
            detail: Some("Reconnect to this TV in TV Deploy so its DUID is verified before issuing a certificate.".into()),
        });
    }
    if let Some(saved) = existing_profile {
        if saved.duid != duid {
            return Err(ApiError {
                code: "profile_duid_mismatch".into(),
                detail: Some("This profile is already associated with another TV. Choose a new profile name for this television.".into()),
            });
        }
        return Ok(CertificateProfileInfo { name: saved.name, identity: saved.identity, duid: saved.duid });
    }
    let sdk = sdk_path.ok_or_else(|| ApiError::new("sdk_not_set"))?;
    if !toolchain::tv_sdk_is_usable(Path::new(&sdk)) {
        return Err(ApiError::new("tv_extension_missing"));
    }
    let tc = toolchain::build(&app, &state.data_dir, Some(&sdk)).map_err(ApiError::new)?;
    let author = Invocation::generate_samsung_author(&profile_name, &identity)?;
    let distributor = Invocation::generate_samsung_distributor(&profile_name, &duid)?;
    let create_profile = Invocation::create_samsung_profile(&profile_name)?;
    let password_env = vec![("TIZEN_CERTIFICATE_PASSWORD".to_string(), password)];
    let profile_worker = profile_name.clone();
    let duid_worker = duid.clone();

    let result = tauri::async_runtime::spawn_blocking(move || {
        let data_dir = sdk_data_dir(&tc.sdk_path);
        let profile_dir = data_dir.join("keystore").join("samsung").join(&profile_worker);
        let author_p12 = profile_dir.join("author.p12");
        let distributor_p12 = profile_dir.join("distributor.p12");

        if author_p12.exists() && !distributor_p12.exists() {
            // Resume a partially completed certificate flow instead of attempting to
            // overwrite an existing Samsung author certificate.
        } else if author_p12.exists() && distributor_p12.exists() {
            // A complete pair without a settings entry may be left by a prior interrupted
            // profile-registration step; try to register the profile below.
        } else {
            let author_out = run_with_env(
                &tc,
                &author,
                &RunOptions { timeout: Duration::from_secs(7 * 60), cancel: CancelToken::new() },
                &password_env,
            ).map_err(|e| ApiError { code: "certificate_failed".into(), detail: Some(e.to_string()) })?;
            cli_success(&author_out, "certificate_failed")?;
        }

        if !distributor_p12.exists() {
            let distributor_out = run_with_env(
                &tc,
                &distributor,
                &RunOptions { timeout: Duration::from_secs(7 * 60), cancel: CancelToken::new() },
                &password_env,
            ).map_err(|e| ApiError { code: "certificate_failed".into(), detail: Some(e.to_string()) })?;
            cli_success(&distributor_out, "certificate_failed")?;
        } else {
            let csr = fs::read_to_string(profile_dir.join("distributor.csr")).unwrap_or_default();
            if !csr.contains(&format!("URN:tizen:deviceid={duid_worker}")) {
                return Err(ApiError {
                    code: "profile_duid_mismatch".into(),
                    detail: Some("A distributor certificate with this profile name already exists for a different TV. Choose another profile name.".into()),
                });
            }
        }

        let profile_out = run(
            &tc,
            &create_profile,
            &RunOptions { timeout: Duration::from_secs(90), cancel: CancelToken::new() },
        ).map_err(|e| ApiError { code: "certificate_failed".into(), detail: Some(e.to_string()) })?;
        cli_success(&profile_out, "certificate_failed")?;
        Ok(())
    })
    .await
    .map_err(|_| ApiError::new("internal"))?;
    result?;

    let profile = SavedCertificateProfile {
        name: profile_name,
        identity,
        duid: duid.clone(),
    };
    let mut settings = state.settings.lock().unwrap();
    settings.certificate_profiles.retain(|p| p.name != profile.name);
    settings.certificate_profiles.push(profile.clone());
    settings::save(&state.data_dir, &settings).map_err(|_| ApiError::new("io_error"))?;
    Ok(CertificateProfileInfo { name: profile.name, identity: profile.identity, duid: profile.duid })
}

#[tauri::command]
pub async fn remove_samsung_certificate_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    profile_name: String,
) -> ApiResult<()> {
    let sdk = state.settings.lock().unwrap().sdk_path.clone().ok_or_else(|| ApiError::new("sdk_not_set"))?;
    let tc = toolchain::build(&app, &state.data_dir, Some(&sdk)).map_err(ApiError::new)?;
    let invocation = Invocation::remove_samsung_profile(&profile_name)?;
    let out = tauri::async_runtime::spawn_blocking(move || {
        run(&tc, &invocation, &RunOptions { timeout: Duration::from_secs(60), cancel: CancelToken::new() })
    })
    .await
    .map_err(|_| ApiError::new("internal"))?
    .map_err(|e| ApiError { code: "certificate_failed".into(), detail: Some(e.to_string()) })?;
    cli_success(&out, "certificate_failed")?;
    let mut settings = state.settings.lock().unwrap();
    settings.certificate_profiles.retain(|p| p.name != profile_name);
    settings::save(&state.data_dir, &settings).map_err(|_| ApiError::new("io_error"))
}

#[derive(Serialize)]
pub struct InstallResult {
    app_name: Option<String>,
    app_id: Option<String>,
    version: Option<String>,
    signed_package_path: String,
    tv_ip: String,
}

#[tauri::command]
pub async fn install_package(
    app: AppHandle,
    state: State<'_, AppState>,
    package_path: String,
    profile_name: String,
    ip: String,
    port: u32,
    duid: String,
    run_after_install: bool,
) -> ApiResult<InstallResult> {
    validate::check_cli_path(&package_path)?;
    let ip = parse_tv_ip(&ip)?;
    let port = parse_port(port)?;
    if duid.trim().is_empty() {
        return Err(ApiError::new("invalid_duid"));
    }
    let report = tauri::async_runtime::spawn_blocking({
        let path = PathBuf::from(&package_path);
        move || analyze_wgt(&path)
    })
    .await
    .map_err(|_| ApiError::new("internal"))?
    .map_err(|e| ApiError { code: "package_invalid".into(), detail: Some(e.to_string()) })?;

    let (sdk, profile) = {
        let settings = state.settings.lock().unwrap();
        let device_matches = settings.devices.iter().any(|d| {
            d.ip == ip.to_string() && d.port == port && d.duid.as_deref() == Some(duid.as_str())
        });
        if !device_matches {
            return Err(ApiError::new("duid_not_verified"));
        }
        let profile = settings.certificate_profiles.iter().find(|p| p.name == profile_name).cloned()
            .ok_or_else(|| ApiError::new("certificate_profile_missing"))?;
        if profile.duid != duid {
            return Err(ApiError::new("profile_duid_mismatch"));
        }
        (settings.sdk_path.clone().ok_or_else(|| ApiError::new("sdk_not_set"))?, profile)
    };
    if !toolchain::tv_sdk_is_usable(Path::new(&sdk)) {
        return Err(ApiError::new("tv_extension_missing"));
    }
    let tc = toolchain::build(&app, &state.data_dir, Some(&sdk)).map_err(ApiError::new)?;
    let data_dir = state.data_dir.clone();
    let package_path = PathBuf::from(package_path);
    let serial = format!("{}:{}", ip, port);
    let app_name = report.app_name.clone();
    let app_id = report.app_id.clone();
    let version = report.version.clone();

    let install_result = tauri::async_runtime::spawn_blocking(move || {
        let staging = data_dir.join("signed-packages");
        fs::create_dir_all(&staging).map_err(|_| ApiError::new("io_error"))?;
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
        let staged = staging.join(format!("tvdeploy-{timestamp}.wgt"));
        fs::copy(&package_path, &staged).map_err(|_| ApiError::new("package_copy_failed"))?;

        let tz = tc.sdk_path.join("tools").join("tizen-core").join(if cfg!(windows) { "tz.exe" } else { "tz" });
        let mut signer = Command::new(&tz);
        signer.args(["pack", "-b"])
            .arg(&staged)
            .args(["-t", "wgt", "-s"])
            .arg(&profile.name)
            .env("HOME", &tc.home_dir)
            .env("USERPROFILE", &tc.home_dir)
            .env_remove("TIZEN_SDK_PATH")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            signer.creation_flags(0x0800_0000);
        }
        let signed = signer.output().map_err(|e| ApiError {
            code: "sign_failed".into(),
            detail: Some(format!("Could not start Tizen signer: {}", e.kind())),
        })?;
        if !signed.status.success() {
            let mut detail = String::from_utf8_lossy(&signed.stderr).to_string();
            if detail.is_empty() { detail = String::from_utf8_lossy(&signed.stdout).to_string(); }
            return Err(ApiError { code: "sign_failed".into(), detail: Some(diagnostics::redact(&detail.chars().take(2500).collect::<String>())) });
        }

        // tz pack -b re-signs an existing package in place. Do not trust its exit code alone:
        // check that both Samsung signature documents are present and structurally valid.
        let signed_report = analyze_wgt(&staged).map_err(|e| ApiError {
            code: "sign_failed".into(),
            detail: Some(format!("The signed WGT could not be inspected: {e}")),
        })?;
        if signed_report.signature != tvdeploy_core::package::SignatureState::SignedStructure {
            return Err(ApiError {
                code: "sign_failed".into(),
                detail: Some("The Tizen signer returned success, but the WGT does not contain both Samsung signature files. Installation was stopped.".into()),
            });
        }

        let install_invocation = Invocation::install_package(
            staged.to_str().ok_or_else(|| ApiError::new("invalid_path"))?,
            &serial,
            run_after_install,
        ).map_err(ApiError::from)?;
        let out = run(
            &tc,
            &install_invocation,
            &RunOptions { timeout: Duration::from_secs(5 * 60), cancel: CancelToken::new() },
        ).map_err(|e| ApiError { code: "install_failed".into(), detail: Some(e.to_string()) })?;
        cli_success(&out, "install_failed")?;

        Ok(staged.to_string_lossy().into_owned())
    })
    .await
    .map_err(|_| ApiError::new("internal"))??;

    Ok(InstallResult {
        app_name,
        app_id,
        version,
        signed_package_path: install_result,
        tv_ip: ip.to_string(),
    })
}

// ---------- devices ----------

#[derive(Serialize)]
pub struct Outcome<T> {
    ok: bool,
    data: Option<T>,
    diagnosis: Option<Diagnosis>,
    /// Sanitized message for an expandable "technical details" area.
    technical: Option<String>,
}

fn fail<T>(d: Diagnosis, technical: Option<String>) -> Outcome<T> {
    Outcome {
        ok: false,
        data: None,
        diagnosis: Some(d),
        technical,
    }
}

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

#[derive(Serialize)]
pub struct FoundTv {
    ip: String,
    port: u16,
    connected: bool,
}

#[derive(Serialize)]
pub struct ScanData {
    devices: Vec<FoundTv>,
    subnets: Vec<String>,
}

#[tauri::command]
pub async fn scan_tvs(
    app: AppHandle,
    state: State<'_, AppState>,
    subnet: Option<String>,
    op_id: Option<String>,
) -> ApiResult<Outcome<ScanData>> {
    let (sdk, port, timeout_ms) = {
        let s = state.settings.lock().unwrap();
        (s.sdk_path.clone(), s.default_port, s.timeout_ms)
    };
    let tc = match toolchain::build(&app, &state.data_dir, sdk.as_deref()) {
        Ok(t) => t,
        Err("sdk_not_set") => return Ok(fail(Diagnosis::ToolsMissing, None)),
        Err(_) => return Ok(fail(Diagnosis::NodeProblem, None)),
    };
    let inv = Invocation::scan(subnet.as_deref(), port, timeout_ms.max(500))?;
    let cancel = register(&state, &op_id);
    let c2 = cancel.clone();
    let out = tauri::async_runtime::spawn_blocking(move || {
        run(
            &tc,
            &inv,
            &RunOptions {
                timeout: Duration::from_secs(120),
                cancel: c2,
            },
        )
    })
    .await
    .map_err(|_| ApiError::new("internal"))?;
    unregister(&state, &op_id);
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
                                connected: d["status"] == "connected",
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
            return Ok(Outcome {
                ok: true,
                data: Some(ScanData { devices, subnets }),
                diagnosis: None,
                technical: None,
            });
        }
    }
    let (d, t) = explain(&out);
    Ok(fail(d, t))
}

#[derive(Serialize)]
pub struct TvInfo {
    ip: String,
    port: u16,
    serial: String,
    duid: Option<String>,
    name: String,
    host_ip: Option<String>,
}

#[derive(Serialize)]
pub struct ConnectOutcome {
    #[serde(flatten)]
    outcome: Outcome<TvInfo>,
    host_ip: Option<String>,
    probe: Option<Probe>,
}

#[tauri::command]
pub async fn connect_tv(
    app: AppHandle,
    state: State<'_, AppState>,
    ip: String,
    port: Option<u32>,
    op_id: Option<String>,
) -> ApiResult<ConnectOutcome> {
    let ip4 = parse_tv_ip(&ip)?;
    let (sdk, default_port, timeout_ms) = {
        let s = state.settings.lock().unwrap();
        (s.sdk_path.clone(), s.default_port, s.timeout_ms)
    };
    let port = match port {
        Some(p) => parse_port(p)?,
        None => default_port,
    };
    let host_ip = local_ip_towards(ip4).map(|i| i.to_string());

    // 1. Our own reachability probe, so the failure can be explained precisely.
    let probe = probe_tcp(
        ip4,
        port,
        Duration::from_millis(timeout_ms.clamp(MIN_DIAGNOSTIC_TIMEOUT.as_millis() as u64, 15_000)),
    );
    if let Some(d) = diagnostics::from_probe(probe) {
        return Ok(ConnectOutcome {
            outcome: fail(d, None),
            host_ip,
            probe: Some(probe),
        });
    }

    let tc = match toolchain::build(&app, &state.data_dir, sdk.as_deref()) {
        Ok(t) => t,
        Err("sdk_not_set") => {
            return Ok(ConnectOutcome {
                outcome: fail(Diagnosis::ToolsMissing, None),
                host_ip,
                probe: Some(probe),
            })
        }
        Err(_) => {
            return Ok(ConnectOutcome {
                outcome: fail(Diagnosis::NodeProblem, None),
                host_ip,
                probe: Some(probe),
            })
        }
    };
    let cancel = register(&state, &op_id);
    let tc2 = tc.clone();
    let c2 = cancel.clone();
    let conn = tauri::async_runtime::spawn_blocking(move || {
        run(
            &tc2,
            &Invocation::connect(ip4, port),
            &RunOptions {
                timeout: Duration::from_secs(60),
                cancel: c2,
            },
        )
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
        unregister(&state, &op_id);
        let (d, t) = explain(&conn);
        // Port is open but sdb did not confirm a connection.
        let d = if d == Diagnosis::Unknown {
            Diagnosis::ConnectionRejected
        } else {
            d
        };
        return Ok(ConnectOutcome {
            outcome: fail(d, t),
            host_ip,
            probe: Some(probe),
        });
    };

    // 2. DUID (optional extra; the connection itself is already confirmed by sdb).
    let c3 = cancel.clone();
    let duid_out = tauri::async_runtime::spawn_blocking(move || {
        run(
            &tc,
            &Invocation::acquire_duid(ip4, port),
            &RunOptions {
                timeout: Duration::from_secs(60),
                cancel: c3,
            },
        )
    })
    .await
    .map_err(|_| ApiError::new("internal"))?;
    unregister(&state, &op_id);
    let duid = duid_out
        .ok()
        .and_then(|o| o.envelope)
        .filter(|e| e.is_success())
        .and_then(|e| duid_result(&e))
        .map(|d| d.duid);

    let name = {
        let mut s = state.settings.lock().unwrap();
        let existing = s
            .devices
            .iter()
            .position(|d| d.ip == dev.ip && d.port == dev.port);
        let name = existing
            .map(|i| s.devices[i].name.clone())
            .unwrap_or_else(|| format!("Samsung TV {}", dev.ip));
        match existing {
            Some(i) => s.devices[i].duid = duid.clone().or(s.devices[i].duid.clone()),
            None => s.devices.push(SavedDevice {
                ip: dev.ip.clone(),
                port: dev.port,
                name: name.clone(),
                duid: duid.clone(),
            }),
        }
        let _ = settings::save(&state.data_dir, &s);
        name
    };
    let info = TvInfo {
        ip: dev.ip,
        port: dev.port,
        serial: dev.serial,
        duid,
        name,
        host_ip: host_ip.clone(),
    };
    Ok(ConnectOutcome {
        outcome: Outcome {
            ok: true,
            data: Some(info),
            diagnosis: None,
            technical: None,
        },
        host_ip,
        probe: Some(probe),
    })
}

#[tauri::command]
pub async fn disconnect_tv(
    app: AppHandle,
    state: State<'_, AppState>,
    ip: String,
    port: u32,
) -> ApiResult<bool> {
    let ip4 = parse_tv_ip(&ip)?;
    let port = parse_port(port)?;
    let sdk = state.settings.lock().unwrap().sdk_path.clone();
    let tc = toolchain::build(&app, &state.data_dir, sdk.as_deref()).map_err(ApiError::new)?;
    let out = tauri::async_runtime::spawn_blocking(move || {
        run(
            &tc,
            &Invocation::disconnect(ip4, port),
            &RunOptions {
                timeout: Duration::from_secs(30),
                cancel: CancelToken::new(),
            },
        )
    })
    .await
    .map_err(|_| ApiError::new("internal"))?;
    Ok(matches!(&out, Ok(o) if o.envelope.as_ref().map(|e| e.is_success()).unwrap_or(false)))
}

// ---------- packages ----------

#[tauri::command]
pub async fn analyze_package(path: String) -> ApiResult<PackageReport> {
    validate::check_cli_path(&path)?;
    let p = PathBuf::from(path);
    tauri::async_runtime::spawn_blocking(move || analyze_wgt(&p))
        .await
        .map_err(|_| ApiError::new("internal"))?
        .map_err(|e| {
            let (code, detail) = match &e {
                PackageError::Unreadable => ("pkg_unreadable", None),
                PackageError::NotAFile => ("pkg_not_a_file", None),
                PackageError::UnsupportedExtension => ("pkg_unsupported_extension", None),
                PackageError::Empty => ("pkg_empty", None),
                PackageError::TooLarge => ("pkg_too_large", None),
                PackageError::NotAZip => ("pkg_not_zip", None),
                PackageError::Unsafe(d) => ("pkg_unsafe", Some(d.clone())),
                PackageError::MissingConfig => ("pkg_missing_config", None),
                PackageError::BadConfig(d) => ("pkg_bad_config", Some(d.clone())),
            };
            ApiError {
                code: code.into(),
                detail,
            }
        })
}

// ---------- settings & saved devices ----------

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
pub fn save_preferences(
    state: State<'_, AppState>,
    theme: String,
    language: String,
    default_port: u32,
    timeout_ms: u64,
) -> ApiResult<()> {
    if !["system", "light", "dark"].contains(&theme.as_str())
        || !["auto", "ru", "en"].contains(&language.as_str())
    {
        return Err(ApiError::new("invalid_setting"));
    }
    let default_port = parse_port(default_port)?;
    let timeout_ms = validate::clamp_timeout_ms(timeout_ms)?;
    let mut s = state.settings.lock().unwrap();
    s.theme = theme;
    s.language = language;
    s.default_port = default_port;
    s.timeout_ms = timeout_ms;
    settings::save(&state.data_dir, &s).map_err(|_| ApiError::new("io_error"))
}

#[tauri::command]
pub fn remove_device(state: State<'_, AppState>, ip: String, port: u32) -> ApiResult<()> {
    let port = parse_port(port)?;
    let mut s = state.settings.lock().unwrap();
    s.devices.retain(|d| !(d.ip == ip && d.port == port));
    settings::save(&state.data_dir, &s).map_err(|_| ApiError::new("io_error"))
}

#[tauri::command]
pub fn rename_device(
    state: State<'_, AppState>,
    ip: String,
    port: u32,
    name: String,
) -> ApiResult<()> {
    let port = parse_port(port)?;
    let name: String = name.trim().chars().take(60).collect();
    if name.is_empty() {
        return Err(ApiError::new("invalid_name"));
    }
    let mut s = state.settings.lock().unwrap();
    if let Some(d) = s.devices.iter_mut().find(|d| d.ip == ip && d.port == port) {
        d.name = name;
    }
    settings::save(&state.data_dir, &s).map_err(|_| ApiError::new("io_error"))
}
