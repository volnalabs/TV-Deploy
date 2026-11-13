import { invoke } from "@tauri-apps/api/core";
import type { ApiError, CertificateProfile, ConnectOutcome, InstalledPackage, Outcome, PackageReport, ScanData, Settings, ToolchainStatus } from "../types";

/** Typed bridge to the Rust backend. The frontend has no other way to touch the system. */
export function toApiError(e: unknown): ApiError {
  if (e && typeof e === "object" && "code" in e) return e as ApiError;
  return { code: "internal", detail: typeof e === "string" ? e : null };
}

export const api = {
  toolchainStatus: () => invoke<ToolchainStatus>("toolchain_status"),
  findExistingSdk: () => invoke<string[]>("find_existing_sdk"),
  setSdkPath: (path: string) => invoke<void>("set_sdk_path", { path }),
  installSamsungSdk: () => invoke<string>("install_samsung_sdk"),
  createSamsungCertificate: (v: { ip: string; port: number; duid: string; profileName: string; identity: string; password: string }) =>
    invoke<CertificateProfile>("create_samsung_certificate", v),
  removeSamsungCertificateProfile: (profileName: string) =>
    invoke<void>("remove_samsung_certificate_profile", { profileName }),
  installPackage: (v: { packagePath: string; profileName: string; ip: string; port: number; duid: string; runAfterInstall: boolean }) =>
    invoke<InstalledPackage>("install_package", v),
  scanTvs: (opId: string, subnet?: string) => invoke<Outcome<ScanData>>("scan_tvs", { opId, subnet: subnet ?? null }),
  connectTv: (ip: string, port: number | null, opId: string) => invoke<ConnectOutcome>("connect_tv", { ip, port, opId }),
  disconnectTv: (ip: string, port: number) => invoke<boolean>("disconnect_tv", { ip, port }),
  analyzePackage: (path: string) => invoke<PackageReport>("analyze_package", { path }),
  cancel: (opId: string) => invoke<void>("cancel_operation", { opId }),
  getSettings: () => invoke<Settings>("get_settings"),
  savePreferences: (s: Pick<Settings, "theme" | "language" | "default_port" | "timeout_ms">) =>
    invoke<void>("save_preferences", { theme: s.theme, language: s.language, defaultPort: s.default_port, timeoutMs: s.timeout_ms }),
  removeDevice: (ip: string, port: number) => invoke<void>("remove_device", { ip, port }),
  renameDevice: (ip: string, port: number, name: string) => invoke<void>("rename_device", { ip, port, name }),
};
