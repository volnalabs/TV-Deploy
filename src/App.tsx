import { CheckCircle2, Moon, Settings as Cog, Sun } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { IconButton, Pill, Step } from "./components/ui";
import { InstallPanel } from "./features/Install";
import { PackagePanel } from "./features/Package";
import { SettingsDialog } from "./features/SettingsDialog";
import { SetupPanel } from "./features/Setup";
import { TvPanel } from "./features/Tv";
import { I18nProvider, useI18n } from "./hooks/i18n";
import { api } from "./services/api";
import type { PackageReport, Settings, ToolchainStatus, TvInfo } from "./types";

const APP_VERSION = "0.1.0";

function useResolvedTheme(pref: Settings["theme"]): "light" | "dark" {
  const [sys, setSys] = useState(() => window.matchMedia("(prefers-color-scheme: dark)").matches);
  useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const on = (e: MediaQueryListEvent) => setSys(e.matches);
    mq.addEventListener("change", on);
    return () => mq.removeEventListener("change", on);
  }, []);
  return pref === "system" ? (sys ? "dark" : "light") : pref;
}

function Shell({ settings, refresh }: { settings: Settings; refresh: () => void }) {
  const { t } = useI18n();
  const [tool, setTool] = useState<ToolchainStatus | null>(null);
  const [tv, setTv] = useState<TvInfo | null>(null);
  const [report, setReport] = useState<PackageReport | null>(null);
  const [packagePath, setPackagePath] = useState<string | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const theme = useResolvedTheme(settings.theme);

  const loadTool = useCallback(() => void api.toolchainStatus().then(setTool), []);
  useEffect(loadTool, [loadTool, settings.sdk_path]);
  useEffect(() => { document.documentElement.dataset.theme = theme; }, [theme]);

  const toggleTheme = () => api.savePreferences({ theme: theme === "dark" ? "light" : "dark", language: settings.language, default_port: settings.default_port, timeout_ms: settings.timeout_ms }).then(refresh);
  const ready = !!tool && tool.sdk_ready && tool.tv_extension_ready && tool.cli_ready && tool.node !== "missing";

  return (
    <div className="flex h-full flex-col">
      <header className="flex h-14 shrink-0 items-center gap-3 border-b border-[var(--border)] px-5">
        <img src="/logo.png" alt="" width={26} height={26} className="rounded-md" />
        <span className="font-semibold tracking-tight">{t("appName")}</span>
        <span className="text-xs text-[var(--muted)]">{t("version", { v: APP_VERSION })}</span>
        <div className="ml-auto flex items-center gap-1">
          {tool && (ready ? <Pill tone="ok"><CheckCircle2 size={12} aria-hidden />{t("setupReady")}</Pill> : <Pill tone="warn">{t("setupTitle")}</Pill>)}
          <IconButton label={theme === "dark" ? t("themeLight") : t("themeDark")} onClick={toggleTheme}>{theme === "dark" ? <Sun size={18} aria-hidden /> : <Moon size={18} aria-hidden />}</IconButton>
          <IconButton label={t("settings")} onClick={() => setShowSettings(true)}><Cog size={18} aria-hidden /></IconButton>
        </div>
      </header>

      <main className="min-h-0 flex-1 overflow-auto">
        <div className="mx-auto max-w-[720px] px-6 py-8">
          <h1 className="m-0 mb-1 text-2xl font-semibold tracking-tight">{t("tagline")}</h1>
          <p className="m-0 mb-8 text-[var(--muted)]">{t("step1")} → {t("step2")} → {t("step3")}</p>

          <Step n={1} title={t("step1")} done={!!tv}>
            {tool && !ready ? <SetupPanel status={tool} onReady={() => { loadTool(); refresh(); }} /> : <TvPanel saved={settings.devices} tv={tv} onTv={setTv} onSavedChanged={refresh} />}
          </Step>
          <Step n={2} title={t("step2")} done={!!report}>
            <PackagePanel report={report} onReport={(r, path) => { setReport(r); setPackagePath(path); }} />
          </Step>
          <Step n={3} title={t("step3")}>
            <InstallPanel tv={tv} report={report} packagePath={packagePath}
              profiles={settings.certificate_profiles} onProfilesChanged={refresh} />
          </Step>
        </div>
      </main>

      {showSettings && <SettingsDialog settings={settings} onClose={() => setShowSettings(false)} onChanged={() => { refresh(); loadTool(); }} />}
    </div>
  );
}

export default function App() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const refresh = useCallback(() => void api.getSettings().then(setSettings), []);
  useEffect(refresh, [refresh]);
  if (!settings) return null;
  return (
    <I18nProvider pref={settings.language}>
      <Shell settings={settings} refresh={refresh} />
    </I18nProvider>
  );
}
