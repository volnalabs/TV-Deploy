/**
 * Demo backend for running the real interface in a plain browser (`npm run dev`) — used for
 * UI development and for documentation screenshots. It keeps everything in memory and
 * emulates one TV on the network. It is never used inside the packaged application.
 *
 * `?demo=fresh` starts from first-run state; `?platform=android` mimics the phone build.
 */
import type {
  AppInfo, CertificateProfile, ConnectOutcome, HistoryEntry, InstallResult, OpenedPackage, Outcome, ScanData,
  SdkPreflight, Settings, ToolchainStatus, UpdateInfo,
} from "../types";

const params = new URLSearchParams(typeof location !== "undefined" ? location.search : "");
const android = params.get("platform") === "android";
const fresh = params.get("demo") === "fresh";
const TV = { ip: "192.168.1.57", port: 26101, duid: "QK4EMULATEDTV0001", model: "Tizen TV (emulated)" };
const now = () => Math.floor(Date.now() / 1000);
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

const settings: Settings = {
  theme: (params.get("theme") as Settings["theme"]) || "system",
  language: (params.get("lang") as Settings["language"]) || "auto",
  engine: android ? "builtin" : "sdk",
  sdk_path: fresh ? null : "/home/demo/tizen-sdk",
  default_port: 26101,
  timeout_ms: 3000,
  devices: fresh ? [] : [
    { ip: TV.ip, port: TV.port, name: "Living room TV", duid: TV.duid, model: TV.model, last_connected: now() - 3600 },
    { ip: "192.168.1.84", port: 26101, name: "Bedroom TV", duid: "QK4EMULATEDTV0002", model: "Tizen TV (emulated)", last_connected: now() - 86400 * 9 },
  ],
  certificate_profiles: fresh ? [] : [
    { name: "EasyTizen-57", identity: "Demo Author", duids: [TV.duid], engine: android ? "builtin" : "sdk", created_at: now() - 86400 * 20, expires_at: now() + 86400 * 345, imported: false },
  ],
  onboarding_done: !fresh,
  check_updates: true,
  pending_sdk_install: null,
};
let sdkReady = !fresh;
let history: HistoryEntry[] = fresh ? [] : [
  { time: now() - 5400, app_name: "My App", app_id: "AbCdE12345.MyApp", version: "1.4.0", file_name: "MyApp.wgt", tv_name: "Living room TV", tv_ip: TV.ip, result: "success", error_code: null, detail: null },
  { time: now() - 86400 * 2, app_name: "Media Player", app_id: "XyZ9876543.Player", version: "2.0.1", file_name: "player.wgt", tv_name: "Bedroom TV", tv_ip: "192.168.1.84", result: "failed", error_code: "tv_cert_rejected", detail: "app_id[XyZ9876543.Player] install failed[118, -12], reason: Check certificate error : :Invalid certificate chain with certificate in signature" },
  { time: now() - 86400 * 6, app_name: "My App", app_id: "AbCdE12345.MyApp", version: "1.3.2", file_name: "MyApp.wgt", tv_name: "Living room TV", tv_ip: TV.ip, result: "success", error_code: null, detail: null },
];
const cancelled = new Set<string>();
const listeners = new Map<string, Set<(p: unknown) => void>>();
const emit = (event: string, payload: unknown) => listeners.get(event)?.forEach((h) => h(payload));

export function demoListen(event: string, handler: (p: unknown) => void): () => void {
  if (!listeners.has(event)) listeners.set(event, new Set());
  listeners.get(event)!.add(handler);
  return () => listeners.get(event)?.delete(handler);
}

const fail = (code: string, detail: string | null = null) => Promise.reject({ code, detail });
const guard = (opId: unknown) => (typeof opId === "string" && cancelled.has(opId) ? fail("cancelled") : Promise.resolve());

type Args = Record<string, any>;
const handlers: Record<string, (a: Args) => unknown> = {
  app_info: (): AppInfo => ({ version: "1.0.0", platform: android ? "android" : "linux", arch: "x86_64", mobile: android, engines: android ? ["builtin"] : ["sdk", "builtin"], host_ip: "192.168.1.23" }),
  get_settings: () => structuredClone(settings),
  save_preferences: (a) => { Object.assign(settings, { theme: a.theme, language: a.language, default_port: a.defaultPort, timeout_ms: a.timeoutMs, check_updates: a.checkUpdates }); },
  set_engine: (a) => { settings.engine = a.engine; },
  complete_onboarding: () => { settings.onboarding_done = true; },
  cancel_operation: (a) => { cancelled.add(a.opId); },
  toolchain_status: (): ToolchainStatus => ({ node: "bundled", cli_ready: true, sdk_path: settings.sdk_path, sdk_ready: sdkReady, tv_extension_ready: sdkReady, sdk_required_gb: 15, pending_install: settings.pending_sdk_install }),
  find_existing_sdk: () => [],
  set_sdk_path: (a) => { settings.sdk_path = a.path; sdkReady = true; },
  sdk_preflight: (a): SdkPreflight => ({ path: a.path ?? "/home/demo/tizen-sdk", free_bytes: 212 * 1024 ** 3, required_bytes: 15 * 1024 ** 3, enough_space: true, sdk_present: false, tv_extension_present: false, shell_ok: true, path_error: null }),
  install_samsung_sdk: async (a) => {
    const steps = ["Checking system requirements", "Installing Tizen platform packages", "Verifying installation"];
    for (let i = 1; i <= 40; i++) {
      await wait(120);
      await guard(a.opId);
      emit("sdk-progress", { op_id: a.opId, phase: i <= 30 ? "sdk" : "tv_extension", step: steps[Math.min(2, Math.floor(i / 14))], current: i <= 30 ? i : i - 30, total: i <= 30 ? 30 : 10, line: `[${i}/40] Installing package ${i}...` });
    }
    settings.sdk_path = a.path ?? "/home/demo/tizen-sdk"; sdkReady = true;
    return settings.sdk_path;
  },
  scan_tvs: async (a): Promise<Outcome<ScanData>> => {
    for (let i = 1; i <= 10; i++) { await wait(90); await guard(a.opId); emit("scan-progress", { op_id: a.opId, fraction: i / 10 }); }
    return { ok: true, data: { devices: [{ ip: TV.ip, port: TV.port, name: TV.model }], subnets: ["192.168.1"] }, diagnosis: null, technical: null };
  },
  connect_tv: async (a): Promise<ConnectOutcome> => {
    await wait(700);
    if (a.ip !== TV.ip && a.ip !== "192.168.1.84") return { ok: false, data: null, diagnosis: "port_closed", technical: null, host_ip: "192.168.1.23", probe: "refused" };
    const duid = a.ip === TV.ip ? TV.duid : "QK4EMULATEDTV0002";
    let saved = settings.devices.find((d) => d.ip === a.ip);
    if (!saved) { saved = { ip: a.ip, port: TV.port, name: `Samsung TV ${a.ip}`, duid, model: TV.model, last_connected: now() }; settings.devices.push(saved); }
    saved.last_connected = now();
    return { ok: true, data: { ip: a.ip, port: TV.port, duid, name: saved.name, model: TV.model, host_ip: "192.168.1.23" }, diagnosis: null, technical: null, host_ip: "192.168.1.23", probe: "open" };
  },
  disconnect_tv: () => true,
  remove_device: (a) => { settings.devices = settings.devices.filter((d) => d.ip !== a.ip); },
  rename_device: (a) => { const d = settings.devices.find((x) => x.ip === a.ip); if (d) d.name = a.name; },
  open_package: async (a): Promise<OpenedPackage> => {
    await wait(350);
    return { path: a.source, report: { file_name: "MyApp.wgt", size_bytes: 4_812_300, entry_count: 148, uncompressed_bytes: 9_300_000, app_name: "My App", app_id: "AbCdE12345.MyApp", package_id: "AbCdE12345", version: "1.4.0", required_api_version: "6.0", signature: "unsigned", has_author_signature: false, has_distributor_signature: false, has_index_html: true, warnings: [] } };
  },
  create_certificate: async (a): Promise<CertificateProfile> => {
    const stages = ["signin", "author", "distributor", "saving"];
    for (const stage of stages) {
      emit("cert-progress", { op_id: a.opId, stage, url: stage === "signin" ? "https://account.samsung.com/" : null });
      await wait(stage === "signin" ? 1600 : 700);
      await guard(a.opId);
    }
    const profile: CertificateProfile = { name: a.args.profileName, identity: a.args.identity, duids: a.args.duids, engine: settings.engine, created_at: now(), expires_at: now() + 86400 * 365, imported: false };
    settings.certificate_profiles.push(profile);
    return profile;
  },
  import_certificate: async (a): Promise<CertificateProfile> => {
    await wait(500);
    const profile: CertificateProfile = { name: a.args.profileName, identity: "Imported Author", duids: [TV.duid], engine: settings.engine, created_at: now(), expires_at: now() + 86400 * 200, imported: true };
    settings.certificate_profiles.push(profile);
    return profile;
  },
  remove_certificate: (a) => { settings.certificate_profiles = settings.certificate_profiles.filter((p) => p.name !== a.profileName); },
  install_package: async (a): Promise<InstallResult> => {
    const p = (stage: string, fraction = 0) => emit("install-progress", { op_id: a.opId, stage, fraction });
    for (let i = 0; i <= 5; i++) { p("signing", i / 5); await wait(110); }
    p("connecting"); await wait(400); await guard(a.opId);
    for (let i = 0; i <= 12; i++) { p("uploading", i / 12); await wait(110); await guard(a.opId); }
    p("installing"); await wait(1300);
    if (a.args.runAfterInstall) { p("launching"); await wait(500); }
    history.unshift({ time: now(), app_name: "My App", app_id: "AbCdE12345.MyApp", version: "1.4.0", file_name: "MyApp.wgt", tv_name: settings.devices.find((d) => d.ip === a.args.ip)?.name ?? a.args.ip, tv_ip: a.args.ip, result: "success", error_code: null, detail: null });
    return { app_name: "My App", app_id: "AbCdE12345.MyApp", version: "1.4.0", tv_ip: a.args.ip, launched: a.args.runAfterInstall ? true : null };
  },
  uninstall_app: () => wait(600),
  get_history: () => structuredClone(history),
  clear_history: () => { history = []; },
  diagnostics_report: () => "EasyTizen 1.0.0 — diagnostics (demo mode)\nPlatform: browser\nEngine: demo\nLocal address: 192.168.1.23 (subnet 192.168.1.x)\n\nSaved TVs: 2\n  192.168.1.57:26101  model=Tizen TV (emulated)  duid=…0001\n  192.168.1.84:26101  model=Tizen TV (emulated)  duid=…0002\n\nCertificate profiles: 1\n  EasyTizen-57  engine=Sdk  tvs=…0001",
  save_text_file: () => undefined,
  check_for_update: (): UpdateInfo => ({ current: "1.0.0", latest: "1.0.0", update_available: false, release_url: "https://github.com/volnalabs/TV-Deploy/releases/latest", download_url: "https://github.com/volnalabs/TV-Deploy/releases/latest" }),
  open_external: (a) => { window.open(a.url, "_blank", "noopener"); },
};

export async function demoInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const handler = handlers[command];
  if (!handler) return Promise.reject({ code: "internal", detail: `demo backend has no command ${command}` });
  return (await handler((args ?? {}) as Args)) as T;
}
