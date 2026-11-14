import { CheckCircle2, FolderOpen, HardDrive, Wrench } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useApp } from "../app/context";
import { Button, Details, IconTile, Notice, Pill, Progress } from "../components/ui";
import { useI18n } from "../hooks/i18n";
import { formatBytes, opId } from "../lib/format";
import { api, on, pickFolder, toApiError } from "../services/api";
import type { ApiError, SdkPreflight, SdkProgress } from "../types";

/** Samsung tools for the SDK engine: automatic install with live progress, cancel and
 *  resume, folder choice with a free-space check, or pointing at an existing SDK. */
export function ToolsPanel({ onReady }: { onReady?: () => void }) {
  const { t, lang, err } = useI18n();
  const { tool, reloadTool, refresh } = useApp();
  const [found, setFound] = useState<string[]>([]);
  const [folder, setFolder] = useState<string | null>(null);
  const [check, setCheck] = useState<SdkPreflight | null>(null);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<SdkProgress | null>(null);
  const [log, setLog] = useState<string[]>([]);
  const [error, setError] = useState<ApiError | null>(null);
  const [stopped, setStopped] = useState(false);
  const op = useRef<string | null>(null);

  useEffect(() => { api.findExistingSdk().then(setFound).catch(() => setFound([])); }, []);
  useEffect(() => { api.sdkPreflight(folder).then(setCheck).catch(() => setCheck(null)); }, [folder, tool?.pending_install]);
  useEffect(() => {
    const off = on<SdkProgress>("sdk-progress", (p) => {
      if (p.op_id !== op.current) return;
      setProgress((prev) => ({ ...p, current: p.current ?? (prev?.phase === p.phase ? prev.current : null), total: p.total ?? (prev?.phase === p.phase ? prev.total : null) }));
      setLog((l) => [...l.slice(-60), p.line]);
    });
    return () => { void off.then((f) => f()); };
  }, []);

  const finish = useCallback(async () => { await refresh(); reloadTool(); onReady?.(); }, [refresh, reloadTool, onReady]);

  const install = async () => {
    const id = opId();
    op.current = id;
    setBusy(true); setError(null); setStopped(false); setProgress(null); setLog([]);
    try {
      await api.installSamsungSdk(folder, id);
      await finish();
    } catch (e) {
      const parsed = toApiError(e);
      if (parsed.code === "cancelled") setStopped(true); else setError(parsed);
      await refresh(); reloadTool();
    } finally {
      setBusy(false); op.current = null;
    }
  };

  const useExisting = async (path: string) => {
    setBusy(true); setError(null);
    try { await api.setSdkPath(path); await finish(); }
    catch (e) { setError(toApiError(e)); }
    finally { setBusy(false); }
  };

  if (!tool) return null;
  const ready = tool.sdk_ready && tool.tv_extension_ready && tool.cli_ready && tool.node !== "missing";
  const brokenBuild = tool.node === "missing" ? t("toolsNodeMissing") : !tool.cli_ready ? t("toolsCliMissing") : null;
  const fraction = progress?.current != null && progress.total ? progress.current / progress.total : null;
  const bytes = (n: number) => formatBytes(n, lang);
  const blocked = !!check && (!!check.path_error || !check.enough_space || !check.shell_ok);

  if (ready) {
    return (
      <div className="flex items-center gap-3">
        <IconTile tone="ok"><CheckCircle2 size={20} aria-hidden /></IconTile>
        <div className="min-w-0 flex-1">
          <p className="font-semibold">{t("toolsReady")}</p>
          <p className="selectable truncate text-xs text-[var(--muted)]">{tool.sdk_path}</p>
        </div>
        <Button size="sm" disabled={busy} onClick={async () => { const p = await pickFolder(); if (p) await useExisting(p); }}>{t("toolsChangeFolder")}</Button>
        {error && <span className="sr-only">{err(error)}</span>}
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="flex items-start gap-3">
        <IconTile><Wrench size={20} aria-hidden /></IconTile>
        <div className="min-w-0 flex-1">
          <p className="font-semibold">{t("toolsNeeded")}</p>
          <p className="mt-0.5 text-[var(--muted)]">{t("toolsBody")}</p>
        </div>
      </div>
      {brokenBuild && <Notice tone="err">{brokenBuild}</Notice>}
      {tool.pending_install && !busy && <Notice tone="info">{t("toolsPending")}</Notice>}

      {busy && op.current ? (
        <div className="fade-up rounded-xl bg-[var(--surface-2)] p-4">
          <div className="mb-2 flex items-center gap-2">
            <Pill tone="accent">{progress?.phase === "tv_extension" ? t("toolsPhaseTv") : t("toolsPhaseSdk")}</Pill>
            <span className="min-w-0 flex-1 truncate text-[13px]">{progress?.step ?? t("toolsWorking")}</span>
            {progress?.current != null && progress.total != null && <span className="text-xs text-[var(--muted)]">{t("toolsPackages", { current: progress.current, total: progress.total })}</span>}
          </div>
          <Progress value={fraction} label={t("toolsTitle")} />
          <p className="mt-3 text-xs text-[var(--muted)]">{t("toolsKeepOpen")}</p>
          {log.length > 0 && <Details label={t("technicalDetails")}>{log.slice(-12).join("\n")}</Details>}
          <div className="mt-3"><Button size="sm" onClick={() => op.current && void api.cancel(op.current)}>{t("cancel")}</Button></div>
        </div>
      ) : (
        <>
          {check && !tool.sdk_ready && (
            <div className="rounded-xl border border-[var(--border)] p-3.5">
              <div className="flex items-center gap-3">
                <HardDrive size={18} className="shrink-0 text-[var(--muted)]" aria-hidden />
                <div className="min-w-0 flex-1">
                  <p className="text-xs text-[var(--muted)]">{t("toolsFolder")}</p>
                  <p className="selectable truncate font-medium">{check.path}</p>
                </div>
                <Button size="sm" onClick={async () => { const p = await pickFolder(); if (p) setFolder(p); }}>{t("toolsChangeFolder")}</Button>
              </div>
              {check.path_error
                ? <p role="alert" className="mt-2 text-[13px] text-[var(--err)]">{t("toolsBadPath")}</p>
                : check.free_bytes != null && (check.enough_space
                  ? <p className="mt-2 text-xs text-[var(--muted)]">{t("toolsFree", { free: bytes(check.free_bytes), need: bytes(check.required_bytes) })}</p>
                  : <p role="alert" className="mt-2 text-[13px] text-[var(--err)]">{t("toolsNoSpace", { free: bytes(check.free_bytes), need: bytes(check.required_bytes) })}</p>)}
              {!check.shell_ok && <p role="alert" className="mt-2 text-[13px] text-[var(--err)]">{t("toolsNeedBash")}</p>}
            </div>
          )}
          {found.length > 0 && (
            <div>
              <p className="mb-1.5 text-xs font-medium tracking-wide text-[var(--muted)] uppercase">{t("toolsFoundSdk")}</p>
              {found.map((p) => (
                <div key={p} className="flex items-center gap-3 border-t border-[var(--border)] py-2">
                  <span className="selectable min-w-0 flex-1 truncate">{p}</span>
                  <Button size="sm" busy={busy} disabled={!!brokenBuild} onClick={() => useExisting(p)}>{t("toolsUse")}</Button>
                </div>
              ))}
            </div>
          )}
          {stopped && <Notice tone="info">{t("toolsCancelled")}</Notice>}
          {error && <Notice tone="err" title={error.code === "sdk_invalid" ? t("toolsInvalid") : err(error)}>{error.detail && <Details label={t("technicalDetails")}>{error.detail}</Details>}</Notice>}
          <div className="flex flex-wrap gap-2">
            <Button variant="primary" busy={busy} disabled={!!brokenBuild || (blocked && !tool.sdk_ready)} onClick={() => void install()}>
              <Wrench size={16} aria-hidden />
              {tool.sdk_ready ? t("toolsInstallExtension") : tool.pending_install || stopped ? t("toolsResume") : t("toolsInstall")}
            </Button>
            <Button disabled={busy || !!brokenBuild} onClick={async () => { const p = await pickFolder(); if (p) await useExisting(p); }}><FolderOpen size={16} aria-hidden />{t("toolsChooseExisting")}</Button>
          </div>
        </>
      )}
    </div>
  );
}
