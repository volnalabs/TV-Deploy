import { CheckCircle2, Copy, Trash2, Tv2 } from "lucide-react";
import { useApp } from "../app/context";
import { Button, copyText, EmptyState, IconButton, PageHeader, Pill, useToast } from "../components/ui";
import { useI18n } from "../hooks/i18n";
import { formatDate } from "../lib/format";
import { api } from "../services/api";
import { ToolsPanel } from "./Setup";
import { TvFinder } from "./TvFinder";

export function DevicesPage() {
  const { t, lang } = useI18n();
  const { settings, refresh, tv, setTv, go, toolsMissing } = useApp();
  const toast = useToast();

  return (
    <div>
      <PageHeader title={t("tvsTitle")} subtitle={t("tvsSubtitle")} />
      <section className="card mb-4 p-5">
        {toolsMissing ? <ToolsPanel /> : <TvFinder showSaved={false} onConnected={(connected) => { setTv(connected); toast("ok", `${connected.name}: ${t("tvConnected").toLowerCase()}`); }} />}
      </section>
      {settings.devices.length === 0 ? (
        <EmptyState icon={<Tv2 size={26} aria-hidden />} title={t("tvEmptyTitle")} body={t("tvEmptyBody")} />
      ) : (
        <ul className="m-0 list-none space-y-3 p-0">
          {settings.devices.map((d) => {
            const current = tv?.ip === d.ip && tv.port === d.port;
            return (
              <li key={`${d.ip}:${d.port}`} className="card fade-up p-4">
                <div className="flex items-center gap-3">
                  <span className={`grid h-10 w-10 shrink-0 place-items-center rounded-xl ${current ? "bg-[var(--ok-soft)] text-[var(--ok)]" : "bg-[var(--surface-2)] text-[var(--muted)]"}`}><Tv2 size={20} aria-hidden /></span>
                  <div className="min-w-0 flex-1">
                    <input aria-label={t("rename")} defaultValue={d.name} maxLength={60} key={d.name}
                      onBlur={(e) => { const v = e.target.value.trim(); if (v && v !== d.name) void api.renameDevice(d.ip, d.port, v).then(refresh); else e.target.value = d.name; }}
                      onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
                      className="selectable -ml-1.5 h-7 w-full min-w-0 rounded-md border border-transparent bg-transparent px-1.5 font-semibold outline-none hover:border-[var(--border-strong)] focus:border-[var(--accent)]" />
                    <p className="selectable truncate text-xs text-[var(--muted)]">{d.ip}:{d.port}{d.model ? ` · ${d.model}` : ""}</p>
                  </div>
                  {current && <Pill tone="ok"><CheckCircle2 size={12} aria-hidden />{t("tvConnected")}</Pill>}
                  <IconButton label={t("tvForget")} onClick={() => { if (current) setTv(null); void api.removeDevice(d.ip, d.port).then(refresh); }}><Trash2 size={16} aria-hidden /></IconButton>
                </div>
                <div className="mt-3 flex flex-wrap items-center gap-x-4 gap-y-2 border-t border-[var(--border)] pt-3 text-xs text-[var(--muted)]">
                  <span>{d.last_connected ? t("tvLastSeen", { date: formatDate(d.last_connected, lang, true) }) : t("tvNeverSeen")}</span>
                  {d.duid && (
                    <button type="button" className="selectable inline-flex items-center gap-1.5 hover:text-[var(--text)]" onClick={() => copyText(d.duid!).then((ok) => ok && toast("ok", t("copied")))}>
                      DUID {d.duid}<Copy size={12} aria-label={t("copy")} />
                    </button>
                  )}
                  {current && <Button size="sm" className="ml-auto" onClick={() => go("install")}>{t("tvUseForInstall")}</Button>}
                </div>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
