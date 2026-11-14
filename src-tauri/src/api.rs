//! Tauri commands. The frontend can only reach the system through these typed functions.
//! Every argument is validated here (and again inside `easytizen-core`) before anything runs.
//!
//! Commands that touch a TV, a certificate or a package dispatch to one of two engines:
//! [`crate::sdk_engine`] (official Tizen SDK + Samsung CLI, desktop) or [`crate::builtin`]
//! (EasyTizen's own SDB client and signer — the only engine on Android).

use crate::history::{self, HistoryEntry};
use crate::settings::{self, Engine, SavedCertificateProfile, SavedDevice, Settings};
use crate::{builtin, sdk_engine, AppState};
use easytizen_core::cancel::CancelToken;
use easytizen_core::connectivity::{local_ip_towards, probe_tcp, Probe, MIN_DIAGNOSTIC_TIMEOUT};
use easytizen_core::diagnostics::{self, Diagnosis};
use easytizen_core::package::{analyze_wgt, PackageError, PackageReport};
use easytizen_core::sdb::scan::{primary_local_ip, subnet_prefix_of};
use easytizen_core::validate::{self, parse_port, parse_tv_ip};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, State};

#[derive(Debug, Serialize, Clone)]
pub struct ApiError {
    pub code: String,
    pub detail: Option<String>,
}

impl ApiError {
    pub fn new(code: &str) -> Self {
        Self {
            code: code.into(),
            detail: None,
        }
    }
    pub fn with(code: &str, detail: impl Into<String>) -> Self {
        let detail = diagnostics::redact(&detail.into().chars().take(3000).collect::<String>());
        Self {
            code: code.into(),
            detail: (!detail.is_empty()).then_some(detail),
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

pub type ApiResult<T> = Result<T, ApiError>;

pub fn register(state: &AppState, op_id: &Option<String>) -> CancelToken {
    let token = CancelToken::new();
    if let Some(id) = op_id {
        state.ops.lock().unwrap().insert(id.clone(), token.clone());
    }
    token
}

pub fn unregister(state: &AppState, op_id: &Option<String>) {
    if let Some(id) = op_id {
        state.ops.lock().unwrap().remove(id);
    }
}

#[tauri::command]
pub fn cancel_operation(state: State<'_, AppState>, op_id: String) {
    if let Some(token) = state.ops.lock().unwrap().get(&op_id) {
        token.cancel();
    }
}

// ---------- shared result shapes ----------

#[derive(Serialize)]
pub struct Outcome<T> {
    pub ok: bool,
    pub data: Option<T>,
    pub diagnosis: Option<Diagnosis>,
    /// Sanitised message for an expandable "technical details" area.
    pub technical: Option<String>,
}

pub fn fail<T>(diagnosis: Diagnosis, technical: Option<String>) -> Outcome<T> {
    Outcome {
        ok: false,
        data: None,
        diagnosis: Some(diagnosis),
        technical,
    }
}

pub fn success<T>(data: T) -> Outcome<T> {
    Outcome {
        ok: true,
        data: Some(data),
        diagnosis: None,
        technical: None,
    }
}

#[derive(Serialize)]
pub struct FoundTv {
    pub ip: String,
    pub port: u16,
    pub name: Option<String>,
}

#[derive(Serialize)]
pub struct ScanData {
    pub devices: Vec<FoundTv>,
    pub subnets: Vec<String>,
}

#[derive(Serialize)]
pub struct TvInfo {
    pub ip: String,
    pub port: u16,
    pub duid: Option<String>,
    pub name: String,
    pub model: Option<String>,
    pub host_ip: Option<String>,
}

#[derive(Serialize)]
pub struct ConnectOutcome {
    #[serde(flatten)]
    pub outcome: Outcome<TvInfo>,
    pub host_ip: Option<String>,
    pub probe: Option<Probe>,
}

#[derive(Serialize)]
pub struct InstallResult {
    pub app_name: Option<String>,
    pub app_id: Option<String>,
    pub version: Option<String>,
    pub tv_ip: String,
    /// `Some(false)` when launching was requested but the TV did not confirm it.
    pub launched: Option<bool>,
}

/// Saves (or refreshes) a TV after a confirmed connection and returns its display name.
pub fn remember_device(
    state: &AppState,
    ip: &str,
    port: u16,
    duid: Option<String>,
    model: Option<String>,
) -> String {
    let mut s = state.settings.lock().unwrap();
    let existing = s.devices.iter().position(|d| d.ip == ip && d.port == port);
    let name = match existing {
        Some(i) => {
            let device = &mut s.devices[i];
            if duid.is_some() {
                device.duid = duid;
            }
            if model.is_some() {
                device.model = model;
            }
            device.last_connected = Some(crate::now_secs());
            device.name.clone()
        }
        None => {
            let name = format!("Samsung TV {ip}");
            s.devices.push(SavedDevice {
                ip: ip.to_string(),
                port,
                name: name.clone(),
                duid,
                model,
                last_connected: Some(crate::now_secs()),
            });
            name
        }
    };
    let _ = settings::save(&state.data_dir, &s);
    name
}

pub fn valid_duid(duid: &str) -> bool {
    easytizen_core::samsung_ca::csr::is_valid_duid(duid)
}

pub fn validate_profile_name(name: &str) -> ApiResult<()> {
    let ok = (3..=64).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    if ok {
        Ok(())
    } else {
        Err(ApiError::new("invalid_profile_name"))
    }
}

pub fn validate_cert_password(password: &str) -> ApiResult<()> {
    if password.len() < 8
        || password.len() > 128
        || !password.bytes().any(|c| c.is_ascii_uppercase())
        || !password.bytes().any(|c| c.is_ascii_lowercase())
        || !password.bytes().any(|c| c.is_ascii_digit())
        || password.chars().any(|c| c.is_control())
    {
        return Err(ApiError::new("cert_password_invalid"));
    }
    Ok(())
}

/// The DUIDs must be ones EasyTizen itself read from a connected TV.
pub fn require_verified_duids(state: &AppState, duids: &[String]) -> ApiResult<()> {
    let s = state.settings.lock().unwrap();
    for duid in duids {
        if !valid_duid(duid) {
            return Err(ApiError::new("invalid_duid"));
        }
        if !s
            .devices
            .iter()
            .any(|d| d.duid.as_deref() == Some(duid.as_str()))
        {
            return Err(ApiError::new("duid_not_verified"));
        }
    }
    Ok(())
}

fn current_engine(state: &AppState) -> Engine {
    if cfg!(mobile) {
        Engine::Builtin
    } else {
        state.settings.lock().unwrap().engine
    }
}

// ---------- app info, settings ----------

#[derive(Serialize)]
pub struct AppInfo {
    version: &'static str,
    /// "android" | "windows" | "macos" | "linux"
    platform: &'static str,
    arch: &'static str,
    mobile: bool,
    /// Engines this build can use.
    engines: Vec<Engine>,
    /// This device's address on the local network — the value for the TV's "Host PC IP".
    host_ip: Option<String>,
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        platform: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        mobile: cfg!(mobile),
        engines: if cfg!(mobile) {
            vec![Engine::Builtin]
        } else {
            vec![Engine::Sdk, Engine::Builtin]
        },
        host_ip: primary_local_ip().map(|ip| ip.to_string()),
    }
}

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
    check_updates: bool,
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
    s.check_updates = check_updates;
    settings::save(&state.data_dir, &s).map_err(|_| ApiError::new("io_error"))
}

#[tauri::command]
pub fn set_engine(state: State<'_, AppState>, engine: Engine) -> ApiResult<()> {
    if cfg!(mobile) && engine != Engine::Builtin {
        return Err(ApiError::new("engine_unavailable"));
    }
    let mut s = state.settings.lock().unwrap();
    s.engine = engine;
    settings::save(&state.data_dir, &s).map_err(|_| ApiError::new("io_error"))
}

#[tauri::command]
pub fn complete_onboarding(state: State<'_, AppState>) -> ApiResult<()> {
    let mut s = state.settings.lock().unwrap();
    s.onboarding_done = true;
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
    let name: String = name
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(60)
        .collect();
    if name.is_empty() {
        return Err(ApiError::new("invalid_name"));
    }
    let mut s = state.settings.lock().unwrap();
    if let Some(d) = s.devices.iter_mut().find(|d| d.ip == ip && d.port == port) {
        d.name = name;
    }
    settings::save(&state.data_dir, &s).map_err(|_| ApiError::new("io_error"))
}

// ---------- SDK toolchain (desktop engine) ----------

#[tauri::command]
pub fn toolchain_status(app: AppHandle, state: State<'_, AppState>) -> sdk_engine::ToolchainStatus {
    sdk_engine::toolchain_status(&app, &state)
}

#[tauri::command]
pub fn find_existing_sdk() -> Vec<String> {
    sdk_engine::find_existing_sdk()
}

#[tauri::command]
pub async fn set_sdk_path(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> ApiResult<()> {
    sdk_engine::set_sdk_path(&app, &state, path).await
}

#[tauri::command]
pub fn sdk_preflight(
    state: State<'_, AppState>,
    path: Option<String>,
) -> crate::sdk_installer::Preflight {
    let fallback = state.settings.lock().unwrap().pending_sdk_install.clone();
    crate::sdk_installer::preflight(path.as_deref().or(fallback.as_deref()))
}

#[tauri::command]
pub async fn install_samsung_sdk(
    app: AppHandle,
    state: State<'_, AppState>,
    path: Option<String>,
    op_id: Option<String>,
) -> ApiResult<String> {
    let cancel = register(&state, &op_id);
    let result = sdk_engine::install_sdk(&app, &state, path, op_id.clone(), cancel).await;
    unregister(&state, &op_id);
    result
}

// ---------- TVs ----------

#[tauri::command]
pub async fn scan_tvs(
    app: AppHandle,
    state: State<'_, AppState>,
    subnet: Option<String>,
    op_id: Option<String>,
) -> ApiResult<Outcome<ScanData>> {
    if let Some(s) = &subnet {
        validate::parse_subnet_prefix(s)?;
    }
    let cancel = register(&state, &op_id);
    let result = match current_engine(&state) {
        Engine::Sdk => sdk_engine::scan(&app, &state, subnet, cancel).await,
        Engine::Builtin => builtin::scan(&app, &state, subnet, op_id.clone(), cancel).await,
    };
    unregister(&state, &op_id);
    result
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
    let (default_port, timeout_ms) = {
        let s = state.settings.lock().unwrap();
        (s.default_port, s.timeout_ms)
    };
    let port = match port {
        Some(p) => parse_port(p)?,
        None => default_port,
    };
    let host_ip = local_ip_towards(ip4).map(|i| i.to_string());

    // Our own reachability probe first, so a failure can be explained precisely.
    let probe_timeout =
        Duration::from_millis(timeout_ms.clamp(MIN_DIAGNOSTIC_TIMEOUT.as_millis() as u64, 15_000));
    let probe = tauri::async_runtime::spawn_blocking(move || probe_tcp(ip4, port, probe_timeout))
        .await
        .map_err(|_| ApiError::new("internal"))?;
    if let Some(d) = diagnostics::from_probe(probe) {
        return Ok(ConnectOutcome {
            outcome: fail(d, None),
            host_ip,
            probe: Some(probe),
        });
    }

    let cancel = register(&state, &op_id);
    let outcome = match current_engine(&state) {
        Engine::Sdk => sdk_engine::connect(&app, &state, ip4, port, cancel).await,
        Engine::Builtin => builtin::connect(&state, ip4, port, cancel).await,
    };
    unregister(&state, &op_id);
    let mut outcome = outcome?;
    if let Some(info) = outcome.data.as_mut() {
        info.host_ip = host_ip.clone();
    }
    Ok(ConnectOutcome {
        outcome,
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
    match current_engine(&state) {
        Engine::Sdk => sdk_engine::disconnect(&app, &state, ip4, port).await,
        // The built-in engine opens a connection per operation; nothing stays connected.
        Engine::Builtin => Ok(true),
    }
}

// ---------- packages ----------

pub fn package_error(e: &PackageError) -> ApiError {
    let (code, detail) = match e {
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
}

#[derive(Serialize)]
pub struct OpenedPackage {
    /// Path the backend will read when installing (the original file on desktop, a private
    /// copy on Android where pickers return `content://` handles).
    path: String,
    report: PackageReport,
}

/// Reads a document the user picked into `destination` (Android `content://` URIs and
/// plain paths alike), with a hard size cap.
pub fn copy_picked_file(
    app: &AppHandle,
    source: &str,
    destination: &Path,
    cap: u64,
) -> ApiResult<u64> {
    use std::io::{Read, Write};
    use tauri_plugin_fs::{FilePath, FsExt, OpenOptions};
    let file_path: FilePath = source.parse().map_err(|_| ApiError::new("invalid_path"))?;
    let mut options = OpenOptions::new();
    options.read(true);
    let mut input = app
        .fs()
        .open(file_path, options)
        .map_err(|_| ApiError::new("pkg_unreadable"))?;
    let mut output = std::fs::File::create(destination).map_err(|_| ApiError::new("io_error"))?;
    let mut buffer = vec![0u8; 256 * 1024];
    let mut total = 0u64;
    loop {
        let n = input
            .read(&mut buffer)
            .map_err(|_| ApiError::new("pkg_unreadable"))?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > cap {
            drop(output);
            let _ = std::fs::remove_file(destination);
            return Err(ApiError::new("pkg_too_large"));
        }
        output
            .write_all(&buffer[..n])
            .map_err(|_| ApiError::new("io_error"))?;
    }
    Ok(total)
}

pub fn is_uri(source: &str) -> bool {
    source.starts_with("content://") || source.starts_with("file://")
}

#[tauri::command]
pub async fn open_package(
    app: AppHandle,
    state: State<'_, AppState>,
    source: String,
) -> ApiResult<OpenedPackage> {
    let staging = state.cache_dir.join("picked");
    tauri::async_runtime::spawn_blocking(move || {
        let path = if is_uri(&source) {
            std::fs::create_dir_all(&staging).map_err(|_| ApiError::new("io_error"))?;
            // One staged package at a time: drop the previous pick.
            if let Ok(entries) = std::fs::read_dir(&staging) {
                for entry in entries.flatten() {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
            let target = staging.join(format!("package-{}.wgt", crate::now_millis()));
            copy_picked_file(
                &app,
                &source,
                &target,
                easytizen_core::package::MAX_ARCHIVE_BYTES,
            )?;
            target
        } else {
            validate::check_cli_path(&source)?;
            PathBuf::from(&source)
        };
        match analyze_wgt(&path) {
            Ok(report) => Ok(OpenedPackage {
                path: path.to_string_lossy().into_owned(),
                report,
            }),
            Err(e) => {
                if path.starts_with(&staging) {
                    let _ = std::fs::remove_file(&path);
                }
                Err(package_error(&e))
            }
        }
    })
    .await
    .map_err(|_| ApiError::new("internal"))?
}

// ---------- certificates ----------

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCertificateArgs {
    pub profile_name: String,
    pub identity: String,
    pub password: String,
    /// TVs the distributor certificate should be valid for (first = the connected TV).
    pub duids: Vec<String>,
}

#[tauri::command]
pub async fn create_certificate(
    app: AppHandle,
    state: State<'_, AppState>,
    args: CreateCertificateArgs,
    op_id: Option<String>,
) -> ApiResult<SavedCertificateProfile> {
    validate_profile_name(&args.profile_name)?;
    validate_cert_password(&args.password)?;
    let identity = args.identity.trim().to_string();
    if !(2..=64).contains(&identity.chars().count()) || identity.chars().any(|c| c.is_control()) {
        return Err(ApiError::new("invalid_identity"));
    }
    if args.duids.is_empty() || args.duids.len() > 50 {
        return Err(ApiError::new("invalid_duid"));
    }
    require_verified_duids(&state, &args.duids)?;
    let engine = current_engine(&state);
    {
        let s = state.settings.lock().unwrap();
        if let Some(existing) = s
            .certificate_profiles
            .iter()
            .find(|p| p.name == args.profile_name)
        {
            if existing.engine == engine && args.duids.iter().all(|d| existing.covers(d)) {
                return Ok(existing.clone());
            }
            return Err(ApiError::new("profile_exists"));
        }
    }
    let args = CreateCertificateArgs { identity, ..args };
    let cancel = register(&state, &op_id);
    let result = match engine {
        Engine::Sdk => sdk_engine::create_certificate(&app, &state, &args, cancel).await,
        Engine::Builtin => {
            builtin::create_certificate(&app, &state, &args, op_id.clone(), cancel).await
        }
    };
    unregister(&state, &op_id);
    let profile = result?;
    let mut s = state.settings.lock().unwrap();
    s.certificate_profiles.retain(|p| p.name != profile.name);
    s.certificate_profiles.push(profile.clone());
    settings::save(&state.data_dir, &s).map_err(|_| ApiError::new("io_error"))?;
    Ok(profile)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCertificateArgs {
    pub profile_name: String,
    /// Paths (desktop) or `content://` URIs (Android) of the two keystores.
    pub author_source: String,
    pub distributor_source: String,
    pub author_password: String,
    pub distributor_password: String,
}

#[tauri::command]
pub async fn import_certificate(
    app: AppHandle,
    state: State<'_, AppState>,
    args: ImportCertificateArgs,
) -> ApiResult<SavedCertificateProfile> {
    validate_profile_name(&args.profile_name)?;
    if args.author_password.is_empty() || args.distributor_password.is_empty() {
        return Err(ApiError::new("cert_password_wrong"));
    }
    if state
        .settings
        .lock()
        .unwrap()
        .certificate_profiles
        .iter()
        .any(|p| p.name == args.profile_name)
    {
        return Err(ApiError::new("profile_exists"));
    }
    let profile = match current_engine(&state) {
        Engine::Sdk => sdk_engine::import_certificate(&app, &state, &args).await,
        Engine::Builtin => builtin::import_certificate(&app, &state, &args).await,
    }?;
    let mut s = state.settings.lock().unwrap();
    s.certificate_profiles.push(profile.clone());
    settings::save(&state.data_dir, &s).map_err(|_| ApiError::new("io_error"))?;
    Ok(profile)
}

#[tauri::command]
pub async fn remove_certificate(
    app: AppHandle,
    state: State<'_, AppState>,
    profile_name: String,
) -> ApiResult<()> {
    validate_profile_name(&profile_name)?;
    let engine = state
        .settings
        .lock()
        .unwrap()
        .certificate_profiles
        .iter()
        .find(|p| p.name == profile_name)
        .map(|p| p.engine)
        .ok_or_else(|| ApiError::new("certificate_profile_missing"))?;
    match engine {
        Engine::Sdk => sdk_engine::remove_certificate(&app, &state, &profile_name).await?,
        Engine::Builtin => builtin::remove_certificate(&state, &profile_name)?,
    }
    state.passwords.lock().unwrap().remove(&profile_name);
    let mut s = state.settings.lock().unwrap();
    s.certificate_profiles.retain(|p| p.name != profile_name);
    settings::save(&state.data_dir, &s).map_err(|_| ApiError::new("io_error"))
}

// ---------- install ----------

#[derive(serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstallArgs {
    pub package_path: String,
    /// Signing profile; `None` only together with `resign = false` (package already signed).
    pub profile_name: Option<String>,
    pub ip: String,
    pub port: u32,
    pub duid: Option<String>,
    pub run_after_install: bool,
    /// Sign a working copy with the profile (default). `false` installs the file as it is.
    pub resign: bool,
    /// Certificate password for the built-in engine; remembered in memory for this session.
    pub password: Option<String>,
}

/// Everything an engine needs to run one installation.
pub struct InstallJob {
    pub args: InstallArgs,
    pub report: PackageReport,
    pub profile: Option<SavedCertificateProfile>,
    pub ip: std::net::Ipv4Addr,
    pub port: u16,
    pub op_id: Option<String>,
    pub cancel: CancelToken,
}

#[tauri::command]
pub async fn install_package(
    app: AppHandle,
    state: State<'_, AppState>,
    args: InstallArgs,
    op_id: Option<String>,
) -> ApiResult<InstallResult> {
    let ip = parse_tv_ip(&args.ip)?;
    let port = parse_port(args.port)?;
    if is_uri(&args.package_path) {
        return Err(ApiError::new("invalid_path"));
    }
    validate::check_cli_path(&args.package_path)?;
    let report = tauri::async_runtime::spawn_blocking({
        let path = PathBuf::from(&args.package_path);
        move || analyze_wgt(&path)
    })
    .await
    .map_err(|_| ApiError::new("internal"))?
    .map_err(|e| package_error(&e))?;

    let (tv_name, profile) = {
        let s = state.settings.lock().unwrap();
        let device = s
            .devices
            .iter()
            .find(|d| d.ip == ip.to_string() && d.port == port);
        let tv_name = device
            .map(|d| d.name.clone())
            .unwrap_or_else(|| format!("Samsung TV {ip}"));
        let profile = match (&args.profile_name, args.resign) {
            (Some(name), true) => {
                let duid = args
                    .duid
                    .as_deref()
                    .filter(|d| valid_duid(d))
                    .ok_or_else(|| ApiError::new("invalid_duid"))?;
                if device.and_then(|d| d.duid.as_deref()) != Some(duid) {
                    return Err(ApiError::new("duid_not_verified"));
                }
                let profile = s
                    .certificate_profiles
                    .iter()
                    .find(|p| &p.name == name)
                    .cloned()
                    .ok_or_else(|| ApiError::new("certificate_profile_missing"))?;
                if !profile.covers(duid) {
                    return Err(ApiError::new("profile_duid_mismatch"));
                }
                Some(profile)
            }
            (None, true) => return Err(ApiError::new("certificate_profile_missing")),
            (_, false) => None,
        };
        (tv_name, profile)
    };
    if !args.resign && report.signature != easytizen_core::package::SignatureState::SignedStructure
    {
        return Err(ApiError::new("package_not_signed"));
    }

    let engine = profile
        .as_ref()
        .map(|p| p.engine)
        .unwrap_or_else(|| current_engine(&state));
    if cfg!(mobile) && engine == Engine::Sdk {
        return Err(ApiError::new("engine_unavailable"));
    }
    let cancel = register(&state, &op_id);
    let job = InstallJob {
        args,
        report: report.clone(),
        profile,
        ip,
        port,
        op_id: op_id.clone(),
        cancel,
    };
    let result = match engine {
        Engine::Sdk => sdk_engine::install(&app, &state, job).await,
        Engine::Builtin => builtin::install(&app, &state, job).await,
    };
    unregister(&state, &op_id);

    let (outcome, code, detail) = match &result {
        Ok(_) => ("success", None, None),
        Err(e) if e.code == "cancelled" => ("cancelled", None, None),
        Err(e) => ("failed", Some(e.code.clone()), e.detail.clone()),
    };
    // A wrong or missing password is a prompt, not an installation attempt.
    if !matches!(
        code.as_deref(),
        Some("cert_password_required" | "cert_password_wrong")
    ) {
        let _ = history::append(
            &state.data_dir,
            HistoryEntry {
                time: crate::now_secs(),
                app_name: report.app_name.clone(),
                app_id: report.app_id.clone(),
                version: report.version.clone(),
                file_name: report.file_name.clone(),
                tv_name,
                tv_ip: ip.to_string(),
                result: outcome.into(),
                error_code: code,
                detail,
            },
        );
    }
    result
}

#[tauri::command]
pub async fn uninstall_app(
    state: State<'_, AppState>,
    ip: String,
    port: u32,
    app_id: String,
) -> ApiResult<()> {
    let ip4 = parse_tv_ip(&ip)?;
    let port = parse_port(port)?;
    if current_engine(&state) != Engine::Builtin {
        return Err(ApiError::new("engine_unavailable"));
    }
    builtin::uninstall(&state, ip4, port, app_id).await
}

// ---------- history, diagnostics, updates ----------

#[tauri::command]
pub fn get_history(state: State<'_, AppState>) -> Vec<HistoryEntry> {
    history::load(&state.data_dir)
}

#[tauri::command]
pub fn clear_history(state: State<'_, AppState>) -> ApiResult<()> {
    history::clear(&state.data_dir).map_err(|_| ApiError::new("io_error"))
}

fn mask_duid(duid: &str) -> String {
    let tail: String = duid
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("…{tail}")
}

/// A plain-text report for bug reports. Contains no passwords, keys or tokens; DUIDs are
/// shortened and the home folder is replaced with `~`.
#[tauri::command]
pub fn diagnostics_report(app: AppHandle, state: State<'_, AppState>) -> String {
    let s = state.settings.lock().unwrap().clone();
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok();
    let tidy = |text: &str| match &home {
        Some(h) if !h.is_empty() => text.replace(h.as_str(), "~"),
        _ => text.to_string(),
    };
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!(
        "EasyTizen {} — diagnostics",
        env!("CARGO_PKG_VERSION")
    ));
    lines.push(format!(
        "Platform: {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    lines.push(format!(
        "Engine: {:?}",
        if cfg!(mobile) {
            Engine::Builtin
        } else {
            s.engine
        }
    ));
    lines.push(format!(
        "Local address: {}",
        primary_local_ip()
            .map(|ip| format!("{ip} (subnet {}.x)", subnet_prefix_of(ip)))
            .unwrap_or_else(|| "none found".into())
    ));
    lines.push(format!(
        "Default TV port: {}, timeout: {} ms",
        s.default_port, s.timeout_ms
    ));
    if cfg!(desktop) {
        let t = sdk_engine::toolchain_status(&app, &state);
        lines.push(format!(
            "Samsung tools: node={:?} cli={} sdk={} tv_extension={} path={}",
            t.node,
            t.cli_ready,
            t.sdk_ready,
            t.tv_extension_ready,
            t.sdk_path
                .as_deref()
                .map(tidy)
                .unwrap_or_else(|| "not set".into())
        ));
        if let Some(pending) = &s.pending_sdk_install {
            lines.push(format!("Unfinished SDK installation: {}", tidy(pending)));
        }
    }
    lines.push(format!("\nSaved TVs: {}", s.devices.len()));
    for d in &s.devices {
        lines.push(format!(
            "  {}:{}  model={}  duid={}  last_connected={}",
            d.ip,
            d.port,
            d.model.as_deref().unwrap_or("?"),
            d.duid
                .as_deref()
                .map(mask_duid)
                .unwrap_or_else(|| "none".into()),
            d.last_connected
                .map(|t| t.to_string())
                .unwrap_or_else(|| "never".into())
        ));
    }
    lines.push(format!(
        "\nCertificate profiles: {}",
        s.certificate_profiles.len()
    ));
    for p in &s.certificate_profiles {
        lines.push(format!(
            "  {}  engine={:?}  tvs={}  imported={}  expires={}",
            p.name,
            p.engine,
            p.duids
                .iter()
                .map(|d| mask_duid(d))
                .collect::<Vec<_>>()
                .join(","),
            p.imported,
            p.expires_at
                .map(|t| t.to_string())
                .unwrap_or_else(|| "?".into())
        ));
    }
    let entries = history::load(&state.data_dir);
    lines.push(format!("\nRecent installations: {}", entries.len().min(10)));
    for e in entries.iter().take(10) {
        lines.push(format!(
            "  {}  {}  {} -> {}  {}{}",
            e.time,
            e.result,
            e.app_id.as_deref().unwrap_or(&e.file_name),
            e.tv_ip,
            e.error_code.as_deref().unwrap_or(""),
            e.detail
                .as_deref()
                .map(|d| format!("\n      {}", tidy(d).replace('\n', "\n      ")))
                .unwrap_or_default()
        ));
    }
    diagnostics::redact(&lines.join("\n"))
}

/// Writes a text file chosen through the native save dialog (diagnostics export).
#[tauri::command]
pub fn save_text_file(path: String, content: String) -> ApiResult<()> {
    validate::check_cli_path(&path)?;
    if content.len() > 2 * 1024 * 1024 {
        return Err(ApiError::new("invalid_setting"));
    }
    let lower = path.to_ascii_lowercase();
    if !(lower.ends_with(".txt") || lower.ends_with(".log")) {
        return Err(ApiError::new("invalid_path"));
    }
    std::fs::write(&path, content).map_err(|_| ApiError::new("io_error"))
}

#[tauri::command]
pub async fn check_for_update() -> ApiResult<easytizen_core::update::UpdateInfo> {
    tauri::async_runtime::spawn_blocking(|| {
        easytizen_core::update::check(env!("CARGO_PKG_VERSION"))
    })
    .await
    .map_err(|_| ApiError::new("internal"))?
    .map_err(|code| ApiError::new(&format!("update_{code}")))
}

/// Opens a web page in the system browser. Only EasyTizen's own GitHub pages and Samsung's
/// account/developer sites are allowed, so the frontend cannot be abused as a link opener.
#[tauri::command]
pub fn open_external(app: AppHandle, url: String) -> ApiResult<()> {
    use tauri_plugin_opener::OpenerExt;
    if !is_allowed_external(&url) {
        return Err(ApiError::new("invalid_path"));
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|_| ApiError::new("io_error"))
}

fn is_allowed_external(url: &str) -> bool {
    const ALLOWED: [&str; 4] = [
        "https://github.com/volnalabs/TV-Deploy",
        "https://account.samsung.com/",
        "https://developer.samsung.com/",
        "https://v3.account.samsung.com/",
    ];
    ALLOWED.iter().any(|prefix| url.starts_with(prefix))
        && !url.chars().any(|c| c.is_control() || c == ' ')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_policy() {
        assert!(validate_cert_password("Passw0rdX").is_ok());
        for bad in [
            "short1A",
            "alllowercase1",
            "ALLUPPERCASE1",
            "NoDigitsHere",
            "Has\nNewline1A",
        ] {
            assert!(validate_cert_password(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn profile_names() {
        assert!(validate_profile_name("EasyTizen-TV_1").is_ok());
        for bad in ["ab", "has space", "sl/ash", "../up", "naïve"] {
            assert!(validate_profile_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn duid_masking_keeps_only_the_tail() {
        assert_eq!(mask_duid("ABCDEFGH12345678"), "…5678");
    }

    #[test]
    fn uri_detection_and_link_allow_list() {
        assert!(is_uri("content://com.android.providers/document/1"));
        assert!(!is_uri("/home/u/app.wgt"));
        assert!(!is_uri("C:\\Users\\u\\app.wgt"));
        assert!(is_allowed_external(
            "https://github.com/volnalabs/TV-Deploy/releases/latest"
        ));
        assert!(is_allowed_external(
            "https://account.samsung.com/mobile/account/check.do?x=1"
        ));
        assert!(!is_allowed_external(
            "https://evil.example/https://github.com/volnalabs/TV-Deploy"
        ));
        assert!(!is_allowed_external("file:///etc/passwd"));
        assert!(!is_allowed_external(
            "https://github.com/volnalabs/TV-Deploy\n"
        ));
    }
}
