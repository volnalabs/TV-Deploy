//! Tauri commands. Every argument is validated here (and again inside `tvdeploy-core`) before
//! anything is executed.

use crate::settings::{self, SavedDevice, Settings};
use crate::toolchain::{self, NodeSource};
use crate::AppState;
use serde::Serialize;
use std::path::PathBuf;
use std::time::Duration;
use tauri::{AppHandle, State};
use tvdeploy_core::connectivity::{local_ip_towards, probe_tcp, Probe};
use tvdeploy_core::diagnostics::{self, Diagnosis};
use tvdeploy_core::package::{analyze_wgt, PackageError, PackageReport};
use tvdeploy_core::samsung::commands::Invocation;
use tvdeploy_core::samsung::results::{connect_result, duid_result};
use tvdeploy_core::samsung::runner::{run, CancelToken, RunError, RunOptions, RunOutput};
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
        Duration::from_millis(timeout_ms.clamp(500, 15_000)),
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
