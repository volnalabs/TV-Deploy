export type Diagnosis =
  | "tools_missing" | "node_problem" | "not_enough_disk_space" | "tv_unreachable" | "port_closed"
  | "network_unreachable" | "connection_rejected" | "operation_timed_out" | "cancelled" | "cli_failure" | "unknown";

export interface ApiError { code: string; detail?: string | null }

export interface Outcome<T> { ok: boolean; data: T | null; diagnosis: Diagnosis | null; technical: string | null }

export interface FoundTv { ip: string; port: number; connected: boolean }
export interface ScanData { devices: FoundTv[]; subnets: string[] }
export interface TvInfo { ip: string; port: number; serial: string; duid: string | null; name: string; host_ip: string | null }
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

export interface SavedDevice { ip: string; port: number; name: string; duid: string | null }
export interface Settings {
  theme: "system" | "light" | "dark"; language: "auto" | "ru" | "en";
  sdk_path: string | null; default_port: number; timeout_ms: number; devices: SavedDevice[];
  certificate_profiles: CertificateProfile[];
}
export interface CertificateProfile { name: string; identity: string; duid: string }
export interface InstalledPackage {
  app_name: string | null; app_id: string | null; version: string | null;
  signed_package_path: string; tv_ip: string;
}
export interface ToolchainStatus {
  node: "bundled" | "system" | "missing"; cli_ready: boolean;
  sdk_path: string | null; sdk_ready: boolean; tv_extension_ready: boolean; sdk_required_gb: number;
}
