import { Download, History, KeyRound, Rocket, Settings as Cog, Tv2, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { AppContext, type AppCtx, type Page } from "./app/context";
import { Logo } from "./components/Logo";
import { Button, IconButton, ToastProvider } from "./components/ui";
import { CertificatesPage } from "./features/CertificatesPage";
import { DevicesPage } from "./features/DevicesPage";
import { HistoryPage } from "./features/HistoryPage";
import { InstallPage } from "./features/InstallPage";
import { Onboarding } from "./features/Onboarding";
import { SettingsPage } from "./features/SettingsPage";
import { I18nProvider, useI18n } from "./hooks/i18n";
import type { Key } from "./i18n";
import { api, isDemo } from "./services/api";
import type { AppInfo, Settings, ToolchainStatus, TvInfo, UpdateInfo } from "./types";

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

const NAV: { page: Page; icon: typeof Tv2; label: Key }[] = [
  { page: "install", icon: Rocket, label: "navInstall" },
  { page: "tvs", icon: Tv2, label: "navTvs" },
  { page: "certs", icon: KeyRound, label: "navCerts" },
  { page: "history", icon: History, label: "navHistory" },
  { page: "settings", icon: Cog, label: "navSettings" },
];

function Shell({ info, settings, refresh }: { info: AppInfo; settings: Settings; refresh: () => Promise<void> }) {
  const { t } = useI18n();
  const [page, setPage] = useState<Page>("install");
  const [tool, setTool] = useState<ToolchainStatus | null>(null);
  const [tv, setTv] = useState<TvInfo | null>(null);
  const [wizard, setWizard] = useState(!settings.onboarding_done);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const theme = useResolvedTheme(settings.theme);
  const usesSdk = !info.mobile && settings.engine === "sdk";

  const reloadTool = useCallback(() => { if (!info.mobile) void api.toolchainStatus().then(setTool).catch(() => setTool(null)); }, [info.mobile]);
  useEffect(reloadTool, [reloadTool, settings.sdk_path, settings.engine]);
  useEffect(() => { document.documentElement.dataset.theme = theme; }, [theme]);
  useEffect(() => {
    if (settings.check_updates && !isDemo) void api.checkForUpdate().then((u) => u.update_available && setUpdate(u)).catch(() => undefined);
    // Checked once per launch.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const toolsMissing = usesSdk && !!tool && !(tool.sdk_ready && tool.tv_extension_ready && tool.cli_ready && tool.node !== "missing");
  const ctx = useMemo<AppCtx>(() => ({ info, settings, refresh, tool, reloadTool, tv, setTv, go: setPage, toolsMissing }), [info, settings, refresh, tool, reloadTool, tv, toolsMissing]);

  return (
    <AppContext.Provider value={ctx}>
      <div className="flex h-full">
        <aside className="hidden w-[216px] shrink-0 flex-col border-r border-[var(--border)] bg-[var(--surface)] px-3 py-4 lg:flex">
          <div className="mb-5 flex items-center gap-2.5 px-2"><Logo size={30} /><span className="text-[16px] font-semibold tracking-tight">EasyTizen</span></div>
          <nav className="flex flex-col gap-0.5">
            {NAV.map(({ page: p, icon: Icon, label }) => (
              <button key={p} type="button" aria-current={page === p ? "page" : undefined} onClick={() => setPage(p)}
                className={`flex h-9 items-center gap-2.5 rounded-lg px-2.5 text-left transition-colors ${page === p ? "bg-[var(--accent-soft)] font-medium text-[var(--accent)]" : "text-[var(--muted)] hover:bg-[var(--surface-2)] hover:text-[var(--text)]"}`}>
                <Icon size={17} aria-hidden />{t(label)}
              </button>
            ))}
          </nav>
          <div className="mt-auto px-2 text-xs text-[var(--faint)]">
            {tv && <p className="mb-2 flex items-center gap-1.5 truncate text-[var(--ok)]"><span className="h-2 w-2 shrink-0 rounded-full bg-[var(--ok)]" />{tv.name}</p>}
            {isDemo && <p className="mb-1 text-[var(--warn)]">{t("demoBadge")}</p>}
            <p>{t("version", { v: info.version })}</p>
          </div>
        </aside>

        <div className="flex min-w-0 flex-1 flex-col">
          <header className="safe-top border-b border-[var(--border)] bg-[var(--surface)] lg:hidden">
            <div className="flex h-13 items-center gap-2.5 px-4 py-2.5">
              <Logo size={26} /><span className="text-[16px] font-semibold tracking-tight">EasyTizen</span>
              {isDemo && <span className="ml-auto truncate text-xs text-[var(--warn)]">{t("demoBadge")}</span>}
            </div>
          </header>
          {update && (
            <div className="fade-up flex flex-wrap items-center gap-3 border-b border-[var(--border)] bg-[var(--accent-soft)] px-5 py-2.5">
              <span className="min-w-0 flex-1 font-medium">{t("updateAvailable", { v: update.latest })}</span>
              <Button size="sm" variant="primary" onClick={() => void api.openExternal(update.download_url)}><Download size={14} aria-hidden />{t("updateDownload")}</Button>
              <IconButton label={t("close")} onClick={() => setUpdate(null)}><X size={16} aria-hidden /></IconButton>
            </div>
          )}
          <main className="min-h-0 flex-1 overflow-auto">
            <div key={page} className="mx-auto max-w-[760px] px-4 py-6 sm:px-8 sm:py-8">
              {page === "install" && <InstallPage />}
              {page === "tvs" && <DevicesPage />}
              {page === "certs" && <CertificatesPage />}
              {page === "history" && <HistoryPage />}
              {page === "settings" && <SettingsPage onShowWizard={() => setWizard(true)} />}
            </div>
          </main>
          <nav className="safe-bottom grid grid-cols-5 border-t border-[var(--border)] bg-[var(--surface)] lg:hidden">
            {NAV.map(({ page: p, icon: Icon, label }) => (
              <button key={p} type="button" aria-current={page === p ? "page" : undefined} onClick={() => setPage(p)}
                className={`flex min-w-0 flex-col items-center gap-0.5 pt-2 pb-1.5 text-[10px] tracking-tight transition-colors ${page === p ? "font-medium text-[var(--accent)]" : "text-[var(--muted)]"}`}>
                <Icon size={20} aria-hidden /><span className="w-full truncate text-center">{t(label)}</span>
              </button>
            ))}
          </nav>
        </div>
      </div>
      {wizard && <Onboarding onDone={() => { setWizard(false); setPage("install"); }} />}
    </AppContext.Provider>
  );
}

export default function App() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [info, setInfo] = useState<AppInfo | null>(null);
  const refresh = useCallback(() => api.getSettings().then(setSettings), []);
  useEffect(() => { void refresh(); void api.appInfo().then(setInfo); }, [refresh]);
  if (!settings || !info) return null;
  return (
    <I18nProvider pref={settings.language}>
      <ToastProvider>
        <Shell info={info} settings={settings} refresh={refresh} />
      </ToastProvider>
    </I18nProvider>
  );
}
