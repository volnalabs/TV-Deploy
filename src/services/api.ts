import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  ApiError, AppInfo, CertificateProfile, ConnectOutcome, Engine, HistoryEntry, InstallResult, OpenedPackage, Outcome,
  ScanData, SdkPreflight, Settings, ToolchainStatus, UpdateInfo,
} from "../types";
import { demoInvoke, demoListen } from "./demo";

/** True inside the real application; false in a plain browser (`npm run dev`), where an
 *  in-memory demo backend with an emulated TV stands in for the Rust side. */
export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
export const isDemo = !inTauri;

function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return inTauri ? invoke<T>(command, args) : demoInvoke<T>(command, args);
}

/** Subscribes to a backend progress event; resolves to an unsubscribe function. */
export function on<T>(event: string, handler: (payload: T) => void): Promise<() => void> {
  return inTauri ? listen<T>(event, (e) => handler(e.payload)) : Promise.resolve(demoListen(event, handler as (p: unknown) => void));
}

export function toApiError(e: unknown): ApiError {
  if (e && typeof e === "object" && "code" in e) return e as ApiError;
  return { code: "internal", detail: typeof e === "string" ? e : null };
}

export interface InstallArgs {
  packagePath: string; profileName: string | null; ip: string; port: number; duid: string | null;
  runAfterInstall: boolean; resign: boolean; password: string | null;
}

/** Typed bridge to the Rust backend. The frontend has no other way to touch the system. */
export const api = {
  appInfo: () => call<AppInfo>("app_info"),
  getSettings: () => call<Settings>("get_settings"),
  savePreferences: (s: Pick<Settings, "theme" | "language" | "default_port" | "timeout_ms" | "check_updates">) =>
    call<void>("save_preferences", { theme: s.theme, language: s.language, defaultPort: s.default_port, timeoutMs: s.timeout_ms, checkUpdates: s.check_updates }),
  setEngine: (engine: Engine) => call<void>("set_engine", { engine }),
  completeOnboarding: () => call<void>("complete_onboarding"),
  cancel: (opId: string) => call<void>("cancel_operation", { opId }),

  toolchainStatus: () => call<ToolchainStatus>("toolchain_status"),
  findExistingSdk: () => call<string[]>("find_existing_sdk"),
  setSdkPath: (path: string) => call<void>("set_sdk_path", { path }),
  sdkPreflight: (path: string | null) => call<SdkPreflight>("sdk_preflight", { path }),
  installSamsungSdk: (path: string | null, opId: string) => call<string>("install_samsung_sdk", { path, opId }),

  scanTvs: (opId: string, subnet?: string) => call<Outcome<ScanData>>("scan_tvs", { opId, subnet: subnet ?? null }),
  connectTv: (ip: string, port: number | null, opId: string) => call<ConnectOutcome>("connect_tv", { ip, port, opId }),
  disconnectTv: (ip: string, port: number) => call<boolean>("disconnect_tv", { ip, port }),
  removeDevice: (ip: string, port: number) => call<void>("remove_device", { ip, port }),
  renameDevice: (ip: string, port: number, name: string) => call<void>("rename_device", { ip, port, name }),

  openPackage: (source: string) => call<OpenedPackage>("open_package", { source }),

  createCertificate: (args: { profileName: string; identity: string; password: string; duids: string[] }, opId: string) =>
    call<CertificateProfile>("create_certificate", { args, opId }),
  importCertificate: (args: { profileName: string; authorSource: string; distributorSource: string; authorPassword: string; distributorPassword: string }) =>
    call<CertificateProfile>("import_certificate", { args }),
  removeCertificate: (profileName: string) => call<void>("remove_certificate", { profileName }),

  installPackage: (args: InstallArgs, opId: string) => call<InstallResult>("install_package", { args, opId }),
  uninstallApp: (ip: string, port: number, appId: string) => call<void>("uninstall_app", { ip, port, appId }),

  getHistory: () => call<HistoryEntry[]>("get_history"),
  clearHistory: () => call<void>("clear_history"),
  diagnosticsReport: () => call<string>("diagnostics_report"),
  saveTextFile: (path: string, content: string) => call<void>("save_text_file", { path, content }),
  checkForUpdate: () => call<UpdateInfo>("check_for_update"),
  openExternal: (url: string) => call<void>("open_external", { url }),
};

/** Native file pickers. On Android they return `content://` handles, which the backend reads. */
export async function pickFile(extensions?: string[], title?: string): Promise<string | null> {
  if (!inTauri) return extensions?.includes("wgt") ? "/demo/MyApp.wgt" : `/demo/${extensions?.[0] ?? "file"}.p12`;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({ multiple: false, directory: false, title, filters: extensions ? [{ name: extensions.map((e) => `.${e}`).join(", "), extensions }] : undefined });
  return typeof picked === "string" ? picked : null;
}

export async function pickFolder(): Promise<string | null> {
  if (!inTauri) return "/home/demo/tizen-sdk";
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({ multiple: false, directory: true });
  return typeof picked === "string" ? picked : null;
}

export async function pickSavePath(defaultName: string): Promise<string | null> {
  if (!inTauri) return null;
  const { save } = await import("@tauri-apps/plugin-dialog");
  return (await save({ defaultPath: defaultName, filters: [{ name: "Text", extensions: ["txt"] }] })) ?? null;
}
