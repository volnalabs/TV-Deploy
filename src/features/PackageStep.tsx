import { FileArchive, ShieldAlert, ShieldCheck, ShieldQuestion, UploadCloud, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useApp } from "../app/context";
import { Button, IconTile, Pill, Row } from "../components/ui";
import { useI18n } from "../hooks/i18n";
import type { Key } from "../i18n";
import { hasKey } from "../i18n";
import { formatBytes } from "../lib/format";
import { api, inTauri, pickFile, toApiError } from "../services/api";
import type { ApiError, OpenedPackage, SignatureState } from "../types";

const SIG_TONE: Record<SignatureState, "ok" | "warn" | "err" | "neutral"> = { signed_structure: "ok", unsigned: "neutral", partial: "warn", malformed: "err" };

export function PackageStep({ pkg, onPackage }: { pkg: OpenedPackage | null; onPackage: (p: OpenedPackage | null) => void }) {
  const { t, lang, err } = useI18n();
  const { info } = useApp();
  const [hover, setHover] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ApiError | null>(null);

  const open = useCallback(async (source: string) => {
    setBusy(true); setError(null);
    try { onPackage(await api.openPackage(source)); }
    catch (e) { onPackage(null); setError(toApiError(e)); }
    finally { setBusy(false); }
  }, [onPackage]);

  // Native drag & drop (desktop): the webview hands over real file paths.
  useEffect(() => {
    if (!inTauri || info.mobile) return;
    let off: (() => void) | undefined;
    let cancelled = false;
    import("@tauri-apps/api/webview").then(({ getCurrentWebview }) =>
      getCurrentWebview().onDragDropEvent((ev) => {
        const p = ev.payload;
        if (p.type === "enter" || p.type === "over") setHover(true);
        else if (p.type === "leave") setHover(false);
        else if (p.type === "drop") { setHover(false); if (p.paths[0]) void open(p.paths[0]); }
      })).then((u) => (cancelled ? u() : (off = u))).catch(() => undefined);
    return () => { cancelled = true; off?.(); };
  }, [open, info.mobile]);

  // Android pickers cannot filter by the .wgt extension; the backend checks the content.
  const pick = async () => { const p = await pickFile(info.mobile ? undefined : ["wgt"], t("dropTitle")); if (p) await open(p); };

  if (pkg) {
    const r = pkg.report;
    const SigIcon = r.signature === "signed_structure" ? ShieldCheck : r.signature === "malformed" ? ShieldAlert : ShieldQuestion;
    return (
      <div className="fade-up">
        <div className="flex items-center gap-3">
          <IconTile><FileArchive size={20} aria-hidden /></IconTile>
          <div className="min-w-0 flex-1">
            <p className="truncate font-semibold">{r.app_name ?? r.file_name}</p>
            <p className="flex items-center gap-1.5 text-xs text-[var(--ok)]"><ShieldCheck size={13} aria-hidden />{t("pkgSafe")}</p>
          </div>
          <Button size="sm" variant="ghost" onClick={() => onPackage(null)}><X size={15} aria-hidden />{t("pkgClear")}</Button>
        </div>
        <dl className="m-0 mt-3">
          {r.version && <Row label={t("pkgVersion")}>{r.version}</Row>}
          {r.app_id && <Row label={t("pkgId")}>{r.app_id}</Row>}
          <Row label={t("pkgSize")}>{formatBytes(r.size_bytes, lang)} · {t("pkgFiles")}: {r.entry_count}</Row>
          {r.required_api_version && <Row label={t("pkgApi")}>{r.required_api_version}</Row>}
          <Row label={t("pkgSignature")}><Pill tone={SIG_TONE[r.signature]}><SigIcon size={12} aria-hidden />{t(`sig_${r.signature}` as Key)}</Pill></Row>
        </dl>
        <p className="mt-3 text-[13px] text-[var(--muted)]">{t(`sigNote_${r.signature}` as Key)}</p>
        {r.warnings.map((w) => { const k = `warn_${w.code}`; return hasKey(k) ? <p key={w.code} className="mt-2 text-[13px] text-[var(--warn)]">{t(k)}</p> : null; })}
      </div>
    );
  }

  return (
    <div>
      <button type="button" onClick={pick} disabled={busy}
        className={`grid w-full place-items-center rounded-xl border-2 border-dashed px-6 py-9 text-center transition-colors ${hover ? "border-[var(--accent)] bg-[var(--accent-soft)]" : "border-[var(--border-strong)] hover:border-[var(--accent)] hover:bg-[var(--surface-2)]"}`}>
        <span className={`grid h-12 w-12 place-items-center rounded-2xl ${hover ? "bg-[var(--accent)] text-[var(--accent-ink)]" : "bg-[var(--accent-soft)] text-[var(--accent)]"}`}><UploadCloud size={24} aria-hidden /></span>
        <span className="mt-3 block text-[15px] font-semibold">{hover ? t("dropRelease") : busy ? t("pkgAnalyzing") : t("dropTitle")}</span>
        <span className="mt-1 block text-[var(--muted)]">{info.mobile ? t("dropHintMobile") : t("dropHint")}</span>
        <span className="mt-2 block text-xs text-[var(--faint)]">{t("dropFormats")}</span>
      </button>
      {error && <p role="alert" className="mt-3 text-[var(--err)]">{err(error)}{error.detail ? ` (${error.detail})` : ""}</p>}
    </div>
  );
}
