export type Diagnosis =
  | "tools_missing" | "node_problem" | "not_enough_disk_space" | "tv_unreachable" | "port_closed"
  | "network_unreachable" | "connection_rejected" | "operation_timed_out" | "cancelled" | "not_a_tv"
  | "cli_failure" | "unknown";

export interface ApiError { code: string; detail?: string | null }
export interface Outcome<T> { ok: boolean; data: T | null; diagnosis: Diagnosis | null; technical: string | null }

export type Engine = "sdk" | "builtin";
export type Platform = "android" | "windows" | "macos" | "linux";
export interface AppInfo {
  version: string; platform: Platform | string; arch: string; mobile: boolean; engines: Engine[]; host_ip: string | null;
}

export interface FoundTv { ip: string; port: number; name: string | null }
export interface ScanData { devices: FoundTv[]; subnets: string[] }
export interface TvInfo { ip: string; port: number; duid: string | null; name: string; model: string | null; host_ip: string | null }
export type ConnectOutcome = Outcome<TvInfo> & { host_ip: string | null; probe: string | null };

export type SignatureState = "unsigned" | "signed_structure" | "partial" | "malformed";
export interface Finding { code: string; detail: string | null }
export interface PackageReport {
  file_name: string; size_bytes: number; entry_count: number; uncompressed_bytes: number;
  app_name: string | null; app_id: string | null; package_id: string | null; version: string | null;
  required_api_version: string | null; signature: SignatureState;
  has_author_signature: boolean; has_distributor_signature: boolean; has_index_html: boolean;
  warnings: Finding[];
}
export interface OpenedPackage { path: string; report: PackageReport }

export interface SavedDevice { ip: string; port: number; name: string; duid: string | null; model: string | null; last_connected: number | null }
export interface CertificateProfile {
  name: string; identity: string; duids: string[]; engine: Engine;
  created_at: number | null; expires_at: number | null; imported: boolean;
}
export interface Settings {
  theme: "system" | "light" | "dark"; language: "auto" | "ru" | "en"; engine: Engine;
  sdk_path: string | null; default_port: number; timeout_ms: number; devices: SavedDevice[];
  certificate_profiles: CertificateProfile[]; onboarding_done: boolean; check_updates: boolean;
  pending_sdk_install: string | null;
}
export interface InstallResult { app_name: string | null; app_id: string | null; version: string | null; tv_ip: string; launched: boolean | null }
export interface ToolchainStatus {
  node: "bundled" | "system" | "missing"; cli_ready: boolean; sdk_path: string | null; sdk_ready: boolean;
  tv_extension_ready: boolean; sdk_required_gb: number; pending_install: string | null;
}
export interface SdkPreflight {
  path: string; free_bytes: number | null; required_bytes: number; enough_space: boolean; sdk_present: boolean;
  tv_extension_present: boolean; shell_ok: boolean; path_error: string | null;
}
export interface SdkProgress { op_id: string | null; phase: "sdk" | "tv_extension"; step: string | null; current: number | null; total: number | null; line: string }
export type InstallStage = "signing" | "connecting" | "uploading" | "installing" | "launching";
export interface InstallProgress { op_id: string | null; stage: InstallStage; fraction: number }
export type CertStage = "signin" | "author" | "distributor" | "saving";
export interface CertProgress { op_id: string | null; stage: CertStage; url: string | null }
export interface HistoryEntry {
  time: number; app_name: string | null; app_id: string | null; version: string | null; file_name: string;
  tv_name: string; tv_ip: string; result: "success" | "failed" | "cancelled"; error_code: string | null; detail: string | null;
}
export interface UpdateInfo { current: string; latest: string; update_available: boolean; release_url: string; download_url: string }
