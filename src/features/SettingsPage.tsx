import { ClipboardCopy, Download, ExternalLink, RefreshCw, Save } from "lucide-react";
import { useState, type ReactNode } from "react";
import { useApp } from "../app/context";
import { Logo } from "../components/Logo";
import { Button, copyText, Details, Field, PageHeader, Segmented, Toggle, useToast } from "../components/ui";
import { useI18n } from "../hooks/i18n";
import { api, pickSavePath, toApiError } from "../services/api";
import type { Settings, UpdateInfo } from "../types";
import { EngineChoice } from "./EngineChoice";
import { ToolsPanel } from "./Setup";

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="card fade-up p-5">
      <h2 className="mb-3 text-xs font-semibold tracking-wide text-[var(--muted)] uppercase">{title}</h2>
      {children}
    </section>
  );
}

export const REPO_URL = "https://github.com/volnalabs/TV-Deploy";

export function SettingsPage({ onShowWizard }: { onShowWizard: () => void }) {
  const { t } = useI18n();
  const { settings, refresh, info } = useApp();
  const toast = useToast();
  const [port, setPort] = useState(String(settings.default_port));
  const [timeout, setTimeoutMs] = useState(String(settings.timeout_ms));
  const [invalid, setInvalid] = useState(false);
  const [checking, setChecking] = useState(false);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [updateNote, setUpdateNote] = useState<string | null>(null);
  const [report, setReport] = useState<string | null>(null);

  const persist = async (patch: Partial<Settings>, p = port, tm = timeout) => {
    const next = { ...settings, ...patch };
    try {
      await api.savePreferences({ theme: next.theme, language: next.language, default_port: Number(p), timeout_ms: Number(tm), check_updates: next.check_updates });
      setInvalid(false);
      await refresh();
    } catch { setInvalid(true); }
  };
  const check = async () => {
    setChecking(true); setUpdate(null); setUpdateNote(null);
    try {
      const result = await api.checkForUpdate();
      if (result.update_available) setUpdate(result); else setUpdateNote(t("updatesLatest"));
    } catch (e) { setUpdateNote(toApiError(e).code === "update_no_release" ? t("updatesNone") : t("updatesFailed")); }
    finally { setChecking(false); }
  };
  const loadReport = async () => { const text = report ?? (await api.diagnosticsReport()); setReport(text); return text; };

  return (
    <div>
      <PageHeader title={t("settingsTitle")} />
      <div className="space-y-4">
        <Section title={t("appearance")}>
          <div className="flex flex-wrap gap-x-8 gap-y-4">
            <Segmented label={t("appearance")} value={settings.theme} onChange={(v) => void persist({ theme: v })} options={[{ value: "system", label: t("themeSystem") }, { value: "light", label: t("themeLight") }, { value: "dark", label: t("themeDark") }]} />
            <Segmented label={t("language")} value={settings.language} onChange={(v) => void persist({ language: v })} options={[{ value: "auto", label: t("langAuto") }, { value: "ru", label: "Русский" }, { value: "en", label: "English" }]} />
          </div>
        </Section>

        {!info.mobile && <Section title={t("engineTitle")}><EngineChoice /></Section>}
        {!info.mobile && settings.engine === "sdk" && <Section title={t("toolsTitle")}><ToolsPanel /></Section>}

        <Section title={t("connection")}>
          <div className="grid max-w-md grid-cols-2 gap-4">
            <Field label={t("defaultPort")}><input className="input" inputMode="numeric" value={port} onChange={(e) => setPort(e.target.value)} onBlur={() => void persist({})} /></Field>
            <Field label={t("timeout")}><input className="input" inputMode="numeric" value={timeout} onChange={(e) => setTimeoutMs(e.target.value)} onBlur={() => void persist({})} /></Field>
          </div>
          {invalid && <p role="alert" className="mt-2 text-[var(--err)]">{t("invalidSetting")}</p>}
        </Section>

        <Section title={t("updates")}>
          <Toggle checked={settings.check_updates} onChange={(v) => void persist({ check_updates: v })} label={t("updatesAuto")} />
          <div className="mt-3 flex flex-wrap items-center gap-3">
            <Button size="sm" busy={checking} onClick={() => void check()}><RefreshCw size={14} aria-hidden />{checking ? t("updatesChecking") : t("updatesCheck")}</Button>
            <span className="text-[13px] text-[var(--muted)]">{t("version", { v: info.version })}</span>
            {updateNote && <span className="text-[13px] text-[var(--muted)]">{updateNote}</span>}
          </div>
          {update && (
            <div className="mt-3 flex flex-wrap items-center gap-3 rounded-xl bg-[var(--accent-soft)] p-3">
              <span className="min-w-0 flex-1 font-medium">{t("updateAvailable", { v: update.latest })}</span>
              <Button size="sm" onClick={() => void api.openExternal(update.release_url)}>{t("updateNotes")}</Button>
              <Button size="sm" variant="primary" onClick={() => void api.openExternal(update.download_url)}><Download size={14} aria-hidden />{t("updateDownload")}</Button>
            </div>
          )}
        </Section>

        <Section title={t("diagnostics")}>
          <p className="text-[var(--muted)]">{t("diagnosticsBody")}</p>
          <div className="mt-3 flex flex-wrap gap-2">
            <Button size="sm" onClick={async () => { if (await copyText(await loadReport())) toast("ok", t("copied")); }}><ClipboardCopy size={14} aria-hidden />{t("diagnosticsCopy")}</Button>
            {!info.mobile && <Button size="sm" onClick={async () => {
              const path = await pickSavePath("easytizen-diagnostics.txt");
              if (!path) return;
              try { await api.saveTextFile(path, await loadReport()); toast("ok", t("diagnosticsSaved")); } catch { toast("err", t("err_io_error")); }
            }}><Save size={14} aria-hidden />{t("diagnosticsSave")}</Button>}
          </div>
          <div onClick={() => void loadReport()}><Details label={t("technicalDetails")}>{report ?? "…"}</Details></div>
        </Section>

        <Section title={t("about")}>
          <div className="flex items-start gap-3">
            <Logo size={40} />
            <div className="min-w-0 flex-1">
              <p className="font-semibold">EasyTizen {info.version}</p>
              <p className="mt-0.5 text-[13px] text-[var(--muted)]">{t("aboutBody")}</p>
              <div className="mt-3 flex flex-wrap gap-2">
                <Button size="sm" onClick={() => void api.openExternal(REPO_URL)}><ExternalLink size={14} aria-hidden />{t("aboutSource")}</Button>
                <Button size="sm" variant="ghost" onClick={onShowWizard}>{t("aboutWizard")}</Button>
              </div>
            </div>
          </div>
        </Section>
      </div>
    </div>
  );
}
