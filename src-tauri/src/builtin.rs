//! Built-in engine: EasyTizen's own SDB client, signer and Samsung certificate flow.
//! Needs no Tizen SDK and no Node.js — this is the engine the Android app runs on.
//!
//! Certificates live in `<app data>/certificates/<profile>/{author,distributor}.p12`,
//! encrypted with the password the user chose. The password itself is kept only in memory
//! for the running session.

use crate::api::{
    fail, remember_device, success, ApiError, ApiResult, CreateCertificateArgs, FoundTv,
    ImportCertificateArgs, InstallJob, InstallResult, Outcome, ScanData, TvInfo,
};
use crate::settings::{Engine, SavedCertificateProfile};
use crate::AppState;
use easytizen_core::cancel::CancelToken;
use easytizen_core::diagnostics;
use easytizen_core::samsung_ca::{self, api::CaClient, oauth::OAuthSession, CaError};
use easytizen_core::sdb::{self, scan, InstallFailure, InstallStage, InstallVerdict, SdbError};
use easytizen_core::sign::{sign_wgt, IdentityError, SignError, SigningIdentity};
use serde::Serialize;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const AUTHOR_FILE: &str = "author.p12";
const DISTRIBUTOR_FILE: &str = "distributor.p12";
const MAX_P12_BYTES: u64 = 256 * 1024;

fn profile_dir(data_dir: &Path, name: &str) -> PathBuf {
    data_dir.join("certificates").join(name)
}

fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(tmp, path)
}

fn sdb_error(e: &SdbError) -> ApiError {
    match e {
        SdbError::Cancelled => ApiError::new("cancelled"),
        SdbError::Timeout => ApiError::new("tv_timeout"),
        SdbError::Refused => ApiError::new("tv_port_closed"),
        SdbError::Unreachable => ApiError::new("tv_unreachable"),
        SdbError::NotTizen => ApiError::new("not_a_tv"),
        SdbError::SyncFailed(reason) => ApiError::with("upload_failed", reason.clone()),
        other => ApiError::with("tv_rejected", other.to_string()),
    }
}

fn ca_error(e: CaError) -> ApiError {
    match e {
        CaError::Cancelled => ApiError::new("cancelled"),
        CaError::AuthTimeout => ApiError::new("samsung_auth_timeout"),
        CaError::AuthInvalid => ApiError::new("samsung_auth_invalid"),
        CaError::PortUnavailable => ApiError::new("samsung_auth_port"),
        CaError::RateLimited => ApiError::new("samsung_rate_limited"),
        CaError::Http(status) => ApiError::with("samsung_api_error", format!("HTTP {status}")),
        CaError::Network(reason) => ApiError::with("samsung_network", reason),
        CaError::BadCertificate => ApiError::new("samsung_bad_certificate"),
        other => ApiError::with("certificate_failed", other.to_string()),
    }
}

fn identity_error(e: IdentityError) -> ApiError {
    match e {
        IdentityError::BadPasswordOrFormat => ApiError::new("cert_password_wrong"),
        other => ApiError::with("cert_file_invalid", other.to_string()),
    }
}

// ---------- TVs ----------

#[derive(Serialize, Clone)]
struct ScanProgress {
    op_id: Option<String>,
    fraction: f32,
}

pub async fn scan(
    app: &AppHandle,
    state: &AppState,
    subnet: Option<String>,
    op_id: Option<String>,
    cancel: CancelToken,
) -> ApiResult<Outcome<ScanData>> {
    let (port, timeout_ms) = {
        let s = state.settings.lock().unwrap();
        (s.default_port, s.timeout_ms)
    };
    let Some(prefix) = subnet.or_else(|| scan::primary_local_ip().map(scan::subnet_prefix_of))
    else {
        return Ok(fail(diagnostics::Diagnosis::NetworkUnreachable, None));
    };
    let app = app.clone();
    let scanned = prefix.clone();
    let devices = tauri::async_runtime::spawn_blocking(move || {
        let last = std::sync::atomic::AtomicU32::new(0);
        scan::scan_subnet(
            &prefix,
            port,
            Duration::from_millis(timeout_ms.clamp(300, 1500)),
            &cancel,
            &|fraction| {
                let percent = (fraction * 100.0) as u32;
                if last.swap(percent, std::sync::atomic::Ordering::Relaxed) != percent {
                    let _ = app.emit(
                        "scan-progress",
                        ScanProgress {
                            op_id: op_id.clone(),
                            fraction,
                        },
                    );
                }
            },
        )
    })
    .await
    .map_err(|_| ApiError::new("internal"))?;
    Ok(success(ScanData {
        devices: devices
            .into_iter()
            .map(|d| FoundTv {
                ip: d.ip,
                port: d.port,
                name: d.name,
            })
            .collect(),
        subnets: vec![scanned],
    }))
}

pub async fn connect(
    state: &AppState,
    ip: Ipv4Addr,
    port: u16,
    cancel: CancelToken,
) -> ApiResult<Outcome<TvInfo>> {
    let timeout = Duration::from_millis(state.settings.lock().unwrap().timeout_ms.max(3000));
    let identity =
        tauri::async_runtime::spawn_blocking(move || sdb::query_tv(ip, port, timeout, &cancel))
            .await
            .map_err(|_| ApiError::new("internal"))?;
    match identity {
        Ok(id) => {
            let name = remember_device(
                state,
                &ip.to_string(),
                port,
                id.duid.clone(),
                id.model.clone(),
            );
            Ok(success(TvInfo {
                ip: ip.to_string(),
                port,
                duid: id.duid,
                name,
                model: id.model,
                host_ip: None,
            }))
        }
        Err(e) => Ok(fail(diagnostics::from_sdb_error(&e), Some(e.to_string()))),
    }
}

pub async fn uninstall(state: &AppState, ip: Ipv4Addr, port: u16, app_id: String) -> ApiResult<()> {
    if !sdb::is_safe_shell_word(&app_id) {
        return Err(ApiError::new("invalid_app_id"));
    }
    let timeout = Duration::from_millis(state.settings.lock().unwrap().timeout_ms.max(3000));
    let out = tauri::async_runtime::spawn_blocking(move || {
        sdb::uninstall_app(ip, port, &app_id, timeout, &CancelToken::new())
    })
    .await
    .map_err(|_| ApiError::new("internal"))?
    .map_err(|e| sdb_error(&e))?;
    let low = out.to_ascii_lowercase();
    if low.contains("fail") {
        return Err(ApiError::with("uninstall_failed", out));
    }
    Ok(())
}

// ---------- certificates ----------

#[derive(Serialize, Clone)]
struct CertProgress {
    op_id: Option<String>,
    /// "signin" | "author" | "distributor" | "saving"
    stage: &'static str,
    /// Samsung sign-in page (only with `signin`), so the UI can offer to open it again.
    url: Option<String>,
}

fn profile_from(
    name: &str,
    identity: &str,
    author: &SigningIdentity,
    distributor: &SigningIdentity,
    imported: bool,
) -> SavedCertificateProfile {
    let mut profile = SavedCertificateProfile::new(
        name.to_string(),
        identity.to_string(),
        distributor.info.duids.clone(),
        Engine::Builtin,
    );
    profile.expires_at = Some(author.info.not_after.min(distributor.info.not_after));
    profile.imported = imported;
    profile
}

pub async fn create_certificate(
    app: &AppHandle,
    state: &AppState,
    args: &CreateCertificateArgs,
    op_id: Option<String>,
    cancel: CancelToken,
) -> ApiResult<SavedCertificateProfile> {
    use tauri_plugin_opener::OpenerExt;
    let dir = profile_dir(&state.data_dir, &args.profile_name);
    let (name, identity, password, duids) = (
        args.profile_name.clone(),
        args.identity.clone(),
        args.password.clone(),
        args.duids.clone(),
    );
    let app = app.clone();
    let worker_password = password.clone();
    let profile = tauri::async_runtime::spawn_blocking(move || -> ApiResult<SavedCertificateProfile> {
        let password = worker_password;
        let emit = |stage: &'static str, url: Option<String>| {
            let _ = app.emit("cert-progress", CertProgress { op_id: op_id.clone(), stage, url });
        };
        let author_path = dir.join(AUTHOR_FILE);
        let distributor_path = dir.join(DISTRIBUTOR_FILE);

        // An author certificate left by an interrupted attempt is reused, not re-issued.
        let existing_author = match std::fs::read(&author_path) {
            Ok(bytes) => Some(SigningIdentity::from_p12(&bytes, &password).map_err(|_| {
                ApiError::with(
                    "cert_password_wrong",
                    "An unfinished certificate with this profile name already exists. Enter the password you used then, or choose another profile name.",
                )
            })?),
            Err(_) => None,
        };

        let session = OAuthSession::start().map_err(ca_error)?;
        let url = session.sign_in_url();
        emit("signin", Some(url.clone()));
        // If the browser cannot be opened the UI still shows the link.
        let _ = app.opener().open_url(url, None::<&str>);
        let token = session.wait(samsung_ca::oauth::AUTH_TIMEOUT, &cancel).map_err(ca_error)?;
        let client = CaClient::default();

        let author = match existing_author {
            Some(identity) => identity,
            None => {
                emit("author", None);
                let p12 = samsung_ca::issue_author(&client, &token, &identity, &password).map_err(ca_error)?;
                let loaded = SigningIdentity::from_p12(&p12, &password).map_err(identity_error)?;
                write_private(&author_path, &p12).map_err(|_| ApiError::new("io_error"))?;
                loaded
            }
        };
        if cancel.is_cancelled() {
            return Err(ApiError::new("cancelled"));
        }

        emit("distributor", None);
        let p12 = samsung_ca::issue_distributor(&client, &token, &duids, &password).map_err(ca_error)?;
        let distributor = SigningIdentity::from_p12(&p12, &password).map_err(identity_error)?;
        if !duids.iter().all(|d| distributor.info.duids.iter().any(|c| c.eq_ignore_ascii_case(d))) {
            return Err(ApiError::new("samsung_bad_certificate"));
        }
        emit("saving", None);
        write_private(&distributor_path, &p12).map_err(|_| ApiError::new("io_error"))?;
        Ok(profile_from(&name, &identity, &author, &distributor, false))
    })
    .await
    .map_err(|_| ApiError::new("internal"))??;
    state
        .passwords
        .lock()
        .unwrap()
        .insert(profile.name.clone(), password);
    Ok(profile)
}

fn read_keystore(app: &AppHandle, source: &str, scratch: &Path) -> ApiResult<Vec<u8>> {
    let size = crate::api::copy_picked_file(app, source, scratch, MAX_P12_BYTES).map_err(|e| {
        if e.code == "pkg_too_large" {
            ApiError::new("cert_file_invalid")
        } else {
            e
        }
    })?;
    let bytes = std::fs::read(scratch).map_err(|_| ApiError::new("io_error"));
    let _ = std::fs::remove_file(scratch);
    if size == 0 {
        return Err(ApiError::new("cert_file_invalid"));
    }
    bytes
}

/// Imports an existing Samsung author + distributor pair (e.g. exported from Tizen Studio).
/// Both keystores are re-encrypted under the author password, which becomes the profile's
/// password.
pub async fn import_certificate(
    app: &AppHandle,
    state: &AppState,
    args: &ImportCertificateArgs,
) -> ApiResult<SavedCertificateProfile> {
    let dir = profile_dir(&state.data_dir, &args.profile_name);
    let scratch_dir = state.cache_dir.join("import");
    let app = app.clone();
    let (name, author_source, distributor_source, author_password, distributor_password) = (
        args.profile_name.clone(),
        args.author_source.clone(),
        args.distributor_source.clone(),
        args.author_password.clone(),
        args.distributor_password.clone(),
    );
    let session_password = author_password.clone();
    let profile =
        tauri::async_runtime::spawn_blocking(move || -> ApiResult<SavedCertificateProfile> {
            std::fs::create_dir_all(&scratch_dir).map_err(|_| ApiError::new("io_error"))?;
            let author_bytes =
                read_keystore(&app, &author_source, &scratch_dir.join("author.tmp"))?;
            let distributor_bytes = read_keystore(
                &app,
                &distributor_source,
                &scratch_dir.join("distributor.tmp"),
            )?;
            let author =
                SigningIdentity::from_p12(&author_bytes, &author_password).map_err(|e| {
                    ApiError {
                        code: format!("author_{}", identity_error(e).code),
                        detail: None,
                    }
                })?;
            let distributor = SigningIdentity::from_p12(&distributor_bytes, &distributor_password)
                .map_err(|e| ApiError {
                    code: format!("distributor_{}", identity_error(e).code),
                    detail: None,
                })?;
            if distributor.info.duids.is_empty() {
                return Err(ApiError::new("not_a_distributor_certificate"));
            }
            if !author.info.duids.is_empty() {
                return Err(ApiError::new("not_an_author_certificate"));
            }
            let author_p12 = author
                .repack("author", &author_password)
                .map_err(identity_error)?;
            let distributor_p12 = distributor
                .repack("distributor", &author_password)
                .map_err(identity_error)?;
            write_private(&dir.join(AUTHOR_FILE), &author_p12)
                .map_err(|_| ApiError::new("io_error"))?;
            write_private(&dir.join(DISTRIBUTOR_FILE), &distributor_p12)
                .map_err(|_| ApiError::new("io_error"))?;
            Ok(profile_from(
                &name,
                &author.info.subject,
                &author,
                &distributor,
                true,
            ))
        })
        .await
        .map_err(|_| ApiError::new("internal"))??;
    state
        .passwords
        .lock()
        .unwrap()
        .insert(profile.name.clone(), session_password);
    Ok(profile)
}

pub fn remove_certificate(state: &AppState, name: &str) -> ApiResult<()> {
    let dir = profile_dir(&state.data_dir, name);
    match std::fs::remove_dir_all(&dir) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(ApiError::new("io_error")),
        _ => Ok(()),
    }
}

// ---------- install ----------

#[derive(Serialize, Clone)]
struct InstallProgress {
    op_id: Option<String>,
    /// "signing" | "connecting" | "uploading" | "installing" | "launching"
    stage: &'static str,
    /// 0–1 for stages that report progress (signing, uploading); otherwise 0.
    fraction: f32,
}

fn stage_name(stage: InstallStage) -> &'static str {
    match stage {
        InstallStage::Connecting => "connecting",
        InstallStage::Uploading => "uploading",
        InstallStage::Installing => "installing",
        InstallStage::Launching => "launching",
    }
}

/// `(launched, profile name + password that proved correct)`.
type InstallDone = (Option<bool>, Option<(String, String)>);

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
    let app_id = report
        .app_id
        .clone()
        .filter(|id| sdb::is_safe_shell_word(id))
        .ok_or_else(|| ApiError::new("pkg_no_app_id"))?;

    // Resolve the certificate password before any slow work, so the UI can ask for it.
    let signing = match &profile {
        Some(profile) => {
            let password = args
                .password
                .clone()
                .filter(|p| !p.is_empty())
                .or_else(|| state.passwords.lock().unwrap().get(&profile.name).cloned())
                .ok_or_else(|| ApiError::new("cert_password_required"))?;
            Some((profile.clone(), password))
        }
        None => None,
    };

    let data_dir = state.data_dir.clone();
    let staging = state.cache_dir.join("signed-packages");
    let timeout = Duration::from_millis(state.settings.lock().unwrap().timeout_ms.max(3000));
    let app = app.clone();
    let source = PathBuf::from(&args.package_path);
    let run_after_install = args.run_after_install;
    let duid = args.duid.clone();

    let worker = tauri::async_runtime::spawn_blocking(move || -> ApiResult<InstallDone> {
        let last = std::cell::Cell::new((String::new(), u32::MAX));
        let emit = |stage: &'static str, fraction: f32| {
            let key = (stage.to_string(), (fraction * 100.0) as u32);
            if last.replace(key.clone()) != key {
                let _ = app.emit(
                    "install-progress",
                    InstallProgress {
                        op_id: op_id.clone(),
                        stage,
                        fraction,
                    },
                );
            }
        };

        let mut verified_password = None;
        let package = match &signing {
            Some((profile, password)) => {
                emit("signing", 0.0);
                let dir = profile_dir(&data_dir, &profile.name);
                let load = |file: &str| -> ApiResult<SigningIdentity> {
                    let bytes = std::fs::read(dir.join(file))
                        .map_err(|_| ApiError::new("certificate_profile_missing"))?;
                    SigningIdentity::from_p12(&bytes, password).map_err(identity_error)
                };
                let author = load(AUTHOR_FILE)?;
                let distributor = load(DISTRIBUTOR_FILE)?;
                verified_password = Some((profile.name.clone(), password.clone()));
                let now = crate::now_secs();
                if author.info.not_after < now || distributor.info.not_after < now {
                    return Err(ApiError::new("cert_expired"));
                }
                let covered = duid
                    .as_deref()
                    .map(|d| {
                        distributor
                            .info
                            .duids
                            .iter()
                            .any(|c| c.eq_ignore_ascii_case(d))
                    })
                    .unwrap_or(false);
                if !covered {
                    return Err(ApiError::new("profile_duid_mismatch"));
                }
                std::fs::create_dir_all(&staging).map_err(|_| ApiError::new("io_error"))?;
                let signed = staging.join(format!("easytizen-{}.wgt", crate::now_millis()));
                sign_wgt(&source, &signed, &author, &distributor, &cancel, &mut |f| {
                    emit("signing", f)
                })
                .map_err(|e| match e {
                    SignError::Cancelled => ApiError::new("cancelled"),
                    SignError::Package(p) => crate::api::package_error(&p),
                    other => ApiError::with("sign_failed", other.to_string()),
                })?;
                signed
            }
            None => source.clone(),
        };
        let is_working_copy = package != source;
        let cleanup = || {
            if is_working_copy {
                let _ = std::fs::remove_file(&package);
            }
        };

        let outcome = std::fs::File::open(&package)
            .map_err(|_| ApiError::new("pkg_unreadable"))
            .and_then(|mut file| {
                let size = file
                    .metadata()
                    .map(|m| m.len())
                    .map_err(|_| ApiError::new("pkg_unreadable"))?;
                sdb::install_wgt(
                    ip,
                    port,
                    &mut file,
                    size,
                    &app_id,
                    run_after_install,
                    timeout,
                    &cancel,
                    &mut |s, f| emit(stage_name(s), f),
                )
                .map_err(|e| sdb_error(&e))
            });
        cleanup();
        let outcome = outcome?;
        match outcome.verdict {
            InstallVerdict::Installed => Ok((outcome.launched, verified_password)),
            InstallVerdict::Failed(kind) => {
                let code = match kind {
                    InstallFailure::CertificateRejected => "tv_cert_rejected",
                    InstallFailure::AuthorMismatch => "tv_author_mismatch",
                    InstallFailure::NoSpace => "tv_no_space",
                    InstallFailure::Unrecognised => "install_unrecognised",
                    InstallFailure::Other => "install_failed",
                };
                Err(ApiError::with(code, outcome.log))
            }
        }
    })
    .await
    .map_err(|_| ApiError::new("internal"))?;

    match worker {
        Ok((launched, verified)) => {
            if let Some((name, password)) = verified {
                state.passwords.lock().unwrap().insert(name, password);
            }
            Ok(InstallResult {
                app_name: report.app_name,
                app_id: report.app_id,
                version: report.version,
                tv_ip: ip.to_string(),
                launched,
            })
        }
        Err(e) => {
            if e.code == "cert_password_wrong" {
                if let Some(profile) = &profile {
                    state.passwords.lock().unwrap().remove(&profile.name);
                }
            }
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_files_are_owner_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("certificates/p/author.p12");
        write_private(&path, b"secret").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"secret");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        assert!(!path.with_extension("tmp").exists());
    }

    #[test]
    fn error_codes_do_not_carry_secrets() {
        assert_eq!(sdb_error(&SdbError::Cancelled).code, "cancelled");
        assert_eq!(sdb_error(&SdbError::NotTizen).code, "not_a_tv");
        let e = ca_error(CaError::Network("password=hunter2".into()));
        assert_eq!(e.code, "samsung_network");
        assert_eq!(e.detail.as_deref(), Some("[redacted line]"));
    }
}
