import { open } from "@tauri-apps/plugin-dialog";
import { Trash2, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Button, IconButton, Segmented } from "../components/ui";
import { useI18n } from "../hooks/i18n";
import { api } from "../services/api";
import type { Settings } from "../types";

export function SettingsDialog({ settings, onClose, onChanged }: { settings: Settings; onClose: () => void; onChanged: () => void }) {
  const { t } = useI18n();
  const [draft, setDraft] = useState(settings);
  const [port, setPort] = useState(String(settings.default_port));
  const [timeout, setTimeoutMs] = useState(String(settings.timeout_ms));
  const [error, setError] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    ref.current?.focus();
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const persist = async (next: Settings, p = port, tm = timeout) => {
    setDraft(next);
    try {
      await api.savePreferences({ theme: next.theme, language: next.language, default_port: Number(p), timeout_ms: Number(tm) });
      setError(false);
      onChanged();
    } catch {
      setError(true);
    }
  };

  const changeSdk = async () => {
    const p = await open({ directory: true, multiple: false });
    if (typeof p === "string") {
      try {
        await api.setSdkPath(p);
        onChanged();
      } catch {
        setError(true);
      }
    }
  };

  const label = "mb-2 text-xs font-medium uppercase tracking-wide text-[var(--muted)]";
  const input = "selectable h-9 w-28 rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-[13px] outline-none focus:border-[var(--accent)]";

  return (
    <div className="fixed inset-0 z-20 grid place-items-center bg-black/30 p-6" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div ref={ref} tabIndex={-1} role="dialog" aria-modal="true" aria-label={t("settings")} className="fade-up max-h-full w-full max-w-lg overflow-auto rounded-2xl border border-[var(--border)] bg-[var(--bg)] p-6 shadow-xl outline-none">
        <div className="mb-5 flex items-center">
          <h2 className="m-0 text-lg font-semibold tracking-tight">{t("settings")}</h2>
          <div className="ml-auto"><IconButton label={t("close")} onClick={onClose}><X size={18} aria-hidden /></IconButton></div>
        </div>

        <p className={label}>{t("appearance")}</p>
        <Segmented label={t("appearance")} value={draft.theme} onChange={(v) => persist({ ...draft, theme: v })} options={[{ value: "system", label: t("themeSystem") }, { value: "light", label: t("themeLight") }, { value: "dark", label: t("themeDark") }]} />

        <p className={`${label} mt-6`}>{t("language")}</p>
        <Segmented label={t("language")} value={draft.language} onChange={(v) => persist({ ...draft, language: v })} options={[{ value: "auto", label: t("langAuto") }, { value: "ru", label: "Русский" }, { value: "en", label: "English" }]} />

        <p className={`${label} mt-6`}>{t("tools")}</p>
        <div className="flex items-center gap-3">
          <span className="selectable min-w-0 flex-1 truncate text-[var(--muted)]">{settings.sdk_path ?? t("sdkNotSet")}</span>
          <Button onClick={changeSdk}>{t("tvChange")}</Button>
        </div>
        <p className="m-0 mt-1 text-xs text-[var(--muted)]">{t("sdkFolder")}</p>

        <p className={`${label} mt-6`}>{t("devicesSection")}</p>
        <div className="flex flex-wrap items-center gap-4">
          <label className="flex items-center gap-2 text-[var(--muted)]">{t("defaultPort")}
            <input className={input} inputMode="numeric" value={port} onChange={(e) => setPort(e.target.value)} onBlur={() => persist(draft)} />
          </label>
          <label className="flex items-center gap-2 text-[var(--muted)]">{t("timeout")}
            <input className={input} inputMode="numeric" value={timeout} onChange={(e) => setTimeoutMs(e.target.value)} onBlur={() => persist(draft)} />
          </label>
        </div>
        {error && <p role="alert" className="m-0 mt-2 text-[var(--err)]">{t("invalidSetting")}</p>}

        {settings.devices.length > 0 && (
          <ul className="m-0 mt-4 list-none divide-y divide-[var(--border)] rounded-xl border border-[var(--border)] bg-[var(--surface)] p-0">
            {settings.devices.map((d) => (
              <li key={`${d.ip}:${d.port}`} className="flex items-center gap-2 px-3 py-2">
                <input
                  aria-label={t("rename")}
                  defaultValue={d.name}
                  maxLength={60}
                  onBlur={(e) => e.target.value.trim() && e.target.value !== d.name && api.renameDevice(d.ip, d.port, e.target.value).then(onChanged)}
                  className="selectable h-8 min-w-0 flex-1 rounded-md border border-transparent bg-transparent px-2 font-medium outline-none hover:border-[var(--border)] focus:border-[var(--accent)]"
                />
                <span className="selectable text-xs text-[var(--muted)]">{d.ip}:{d.port}</span>
                <IconButton label={t("tvForget")} onClick={() => api.removeDevice(d.ip, d.port).then(onChanged)}><Trash2 size={15} aria-hidden /></IconButton>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
