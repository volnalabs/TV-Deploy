import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { FileArchive, ShieldAlert, ShieldCheck, ShieldQuestion, UploadCloud, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Button, Pill, Row } from "../components/ui";
import { useI18n } from "../hooks/i18n";
import { hasKey, type Key } from "../i18n";
import { formatBytes } from "../lib/format";
import { api, toApiError } from "../services/api";
import type { PackageReport, SignatureState } from "../types";

const SIG_TONE: Record<SignatureState, "ok" | "warn" | "err"> = { signed_structure: "ok", unsigned: "warn", partial: "warn", malformed: "err" };

export function PackagePanel({ report, onReport }: { report: PackageReport | null; onReport: (r: PackageReport | null) => void }) {
  const { t, lang } = useI18n();
  const [hover, setHover] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<Key | null>(null);

  const analyze = useCallback(async (path: string) => {
    setBusy(true);
    setError(null);
    try {
      onReport(await api.analyzePackage(path));
    } catch (e) {
      const code = `err_${toApiError(e).code}`;
      onReport(null);
      setError(hasKey(code) ? code : "err_internal");
    } finally {
      setBusy(false);
    }
  }, [onReport]);

  // Native drag & drop: Tauri hands us real file paths (browsers hide them).
  useEffect(() => {
    let off: (() => void) | undefined;
    let cancelled = false;
    getCurrentWebview()
      .onDragDropEvent((ev) => {
        const p = ev.payload;
        if (p.type === "enter" || p.type === "over") setHover(true);
        else if (p.type === "leave") setHover(false);
        else if (p.type === "drop") {
          setHover(false);
          if (p.paths[0]) void analyze(p.paths[0]);
        }
      })
      .then((u) => (cancelled ? u() : (off = u)))
      .catch(() => undefined);
    return () => {
      cancelled = true;
      off?.();
    };
  }, [analyze]);

  const pick = async () => {
    const p = await open({ multiple: false, directory: false, filters: [{ name: t("dropFormats"), extensions: ["wgt"] }] });
    if (typeof p === "string") await analyze(p);
  };

  if (report) {
    const SigIcon = report.signature === "signed_structure" ? ShieldCheck : report.signature === "malformed" ? ShieldAlert : ShieldQuestion;
    return (
      <div className="fade-up rounded-xl border border-[var(--border)] bg-[var(--surface)] p-4">
        <div className="flex items-center gap-3">
          <span className="grid h-10 w-10 place-items-center rounded-lg bg-[var(--accent-soft)] text-[var(--accent)]"><FileArchive size={20} aria-hidden /></span>
          <div className="min-w-0 flex-1">
            <p className="m-0 truncate font-semibold">{report.app_name ?? report.file_name}</p>
            <p className="selectable m-0 truncate text-xs text-[var(--muted)]">{report.file_name}</p>
          </div>
          <Button variant="ghost" onClick={() => onReport(null)}><X size={15} aria-hidden />{t("pkgClear")}</Button>
        </div>
        <dl className="m-0 mt-3">
          {report.app_name && <Row label={t("pkgName")}>{report.app_name}</Row>}
          {report.version && <Row label={t("pkgVersion")}>{report.version}</Row>}
          {report.package_id && <Row label={t("pkgId")}>{report.package_id}</Row>}
          <Row label={t("pkgSize")}>{formatBytes(report.size_bytes, lang)}</Row>
          <Row label={t("pkgType")}>{t("pkgTypeWgt")}</Row>
          {report.required_api_version && <Row label={t("pkgApi")}>{report.required_api_version}</Row>}
          <Row label={t("pkgSignature")}><Pill tone={SIG_TONE[report.signature]}><SigIcon size={12} aria-hidden />{t(`sig_${report.signature}` as Key)}</Pill></Row>
        </dl>
        <p className="m-0 mt-3 text-[var(--muted)]">{t(`sigNote_${report.signature}` as Key)}</p>
        {report.warnings.map((w) => {
          const k = `warn_${w.code}`;
          return hasKey(k) ? <p key={w.code} className="m-0 mt-2 text-[var(--warn)]">{t(k)}</p> : null;
        })}
      </div>
    );
  }

  return (
    <div>
      <div
        className={`grid place-items-center rounded-xl border-2 border-dashed px-6 py-10 text-center transition-colors ${hover ? "border-[var(--accent)] bg-[var(--accent-soft)]" : "border-[var(--border)] bg-[var(--surface)]"}`}
      >
        <UploadCloud size={28} className={hover ? "text-[var(--accent)]" : "text-[var(--muted)]"} aria-hidden />
        <p className="m-0 mt-3 text-[15px] font-semibold">{hover ? t("dropRelease") : busy ? t("pkgAnalyzing") : t("dropTitle")}</p>
        <p className="m-0 mt-1 text-[var(--muted)]">{t("dropHint")}</p>
        <div className="mt-4"><Button variant="primary" busy={busy} onClick={pick}>{t("dropButton")}</Button></div>
        <p className="m-0 mt-3 text-xs text-[var(--muted)]">{t("dropFormats")}</p>
      </div>
      {error && <p role="alert" className="m-0 mt-3 text-[var(--err)]">{t(error)}</p>}
    </div>
  );
}
