import { Check, CheckCircle2, Download, KeyRound, Loader2, Rocket } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useApp } from "../app/context";
import { Button, Details, Field, IconTile, Modal, Notice, Progress, Toggle } from "../components/ui";
import { useI18n } from "../hooks/i18n";
import type { Key } from "../i18n";
import { daysLeft, opId } from "../lib/format";
import { api, on, toApiError } from "../services/api";
import type { ApiError, InstallProgress, InstallResult, InstallStage, OpenedPackage, TvInfo } from "../types";
import { CreateCertificateDialog, ImportCertificateDialog } from "./CertDialogs";

const STAGES: InstallStage[] = ["signing", "connecting", "uploading", "installing", "launching"];

export function InstallStep({ tv, pkg, onReset }: { tv: TvInfo; pkg: OpenedPackage; onReset: () => void }) {
  const { t, err } = useI18n();
  const { settings, info } = useApp();
  const engine = info.mobile ? "builtin" : settings.engine;
  const profiles = useMemo(
    () => settings.certificate_profiles.filter((p) => p.engine === engine && !!tv.duid && p.duids.some((d) => d.toLowerCase() === tv.duid!.toLowerCase()) && (daysLeft(p.expires_at) ?? 1) >= 0),
    [settings.certificate_profiles, engine, tv.duid],
  );
  const alreadySigned = pkg.report.signature === "signed_structure";
  const [profile, setProfile] = useState("");
  const [resign, setResign] = useState(true);
  const [runAfter, setRunAfter] = useState(true);
  const [dialog, setDialog] = useState<"create" | "import" | null>(null);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<InstallProgress | null>(null);
  const [result, setResult] = useState<InstallResult | null>(null);
  const [error, setError] = useState<ApiError | null>(null);
  const [cancelled, setCancelled] = useState(false);
  const [askPassword, setAskPassword] = useState<"required" | "wrong" | null>(null);
  const [password, setPassword] = useState("");
  const [removing, setRemoving] = useState(false);
  const op = useRef<string | null>(null);

  useEffect(() => {
    if (!profiles.some((p) => p.name === profile)) setProfile(profiles[0]?.name ?? "");
  }, [profiles, profile]);
  useEffect(() => {
    const off = on<InstallProgress>("install-progress", (p) => { if (p.op_id === op.current) setProgress(p); });
    return () => { void off.then((f) => f()); };
  }, []);

  const willSign = resign || !alreadySigned;
  const canInstall = !busy && (willSign ? !!profile && !!tv.duid : true);

  const install = async (withPassword: string | null = null) => {
    const id = opId();
    op.current = id;
    setBusy(true); setError(null); setResult(null); setCancelled(false); setProgress(null); setAskPassword(null);
    try {
      setResult(await api.installPackage({ packagePath: pkg.path, profileName: willSign ? profile : null, ip: tv.ip, port: tv.port, duid: tv.duid, runAfterInstall: runAfter, resign: willSign, password: withPassword }, id));
    } catch (e) {
      const parsed = toApiError(e);
      if (parsed.code === "cancelled") setCancelled(true);
      else if (parsed.code === "cert_password_required") setAskPassword("required");
      else if (parsed.code === "cert_password_wrong") setAskPassword("wrong");
      else setError(parsed);
    } finally {
      setBusy(false); op.current = null; setPassword("");
    }
  };

  const uninstallAndRetry = async () => {
    if (!pkg.report.app_id) return;
    setRemoving(true);
    try { await api.uninstallApp(tv.ip, tv.port, pkg.report.app_id); setRemoving(false); await install(); }
    catch (e) { setRemoving(false); setError(toApiError(e)); }
  };

  if (result) {
    return (
      <div className="fade-up py-4 text-center">
        <span className="mx-auto grid h-16 w-16 place-items-center rounded-full bg-[var(--ok-soft)] text-[var(--ok)]">
          <svg width="30" height="30" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden><path className="check-draw" d="M5 12.5l4.5 4.5L19 7.5" /></svg>
        </span>
        <p className="mt-4 text-lg font-semibold">{t("installSuccessTitle")}</p>
        <p className="mt-1 text-[var(--muted)]">{t("installSuccess", { name: result.app_name ?? pkg.report.file_name, tv: tv.name })}</p>
        {result.launched != null && <p className={`mt-1 text-[13px] ${result.launched ? "text-[var(--muted)]" : "text-[var(--warn)]"}`}>{result.launched ? t("installLaunched") : t("installNotLaunched")}</p>}
        <div className="mt-5"><Button onClick={onReset}>{t("installAnother")}</Button></div>
      </div>
    );
  }

  if (busy || removing) {
    const index = progress ? STAGES.indexOf(progress.stage) : -1;
    const visible = STAGES.filter((s) => (s !== "signing" || willSign) && (s !== "launching" || runAfter) && (s !== "connecting" || engine === "builtin") && (s !== "uploading" || engine === "builtin"));
    return (
      <div className="fade-up">
        <ul className="m-0 list-none space-y-3 p-0">
          {removing && <li className="flex items-center gap-3"><Loader2 size={18} className="spin text-[var(--accent)]" aria-hidden />{t("installUninstalling")}</li>}
          {!removing && visible.map((s) => {
            const i = STAGES.indexOf(s);
            const state = i < index ? "done" : i === index ? "active" : "todo";
            return (
              <li key={s} className={`flex items-center gap-3 ${state === "todo" ? "text-[var(--faint)]" : ""}`}>
                <span className={`grid h-6 w-6 shrink-0 place-items-center rounded-full ${state === "done" ? "bg-[var(--ok-soft)] text-[var(--ok)]" : state === "active" ? "bg-[var(--accent-soft)] text-[var(--accent)]" : "bg-[var(--surface-2)]"}`}>
                  {state === "done" ? <Check size={14} aria-hidden /> : state === "active" ? <Loader2 size={14} className="spin" aria-hidden /> : null}
                </span>
                <span className={`min-w-0 flex-1 ${state === "active" ? "font-medium" : ""}`}>{t(`stage_${s}` as Key)}</span>
                {state === "active" && (s === "uploading" || s === "signing") && <span className="text-xs text-[var(--muted)] tabular-nums">{Math.round(progress!.fraction * 100)}%</span>}
              </li>
            );
          })}
        </ul>
        <div className="mt-4"><Progress value={progress && (progress.stage === "uploading" || progress.stage === "signing") ? progress.fraction : null} /></div>
        {!removing && <div className="mt-4 flex flex-wrap items-center gap-3"><Button size="sm" onClick={() => op.current && void api.cancel(op.current)}>{t("cancel")}</Button><span className="text-xs text-[var(--muted)]">{t("installCancelNote")}</span></div>}
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {!tv.duid && willSign && <Notice tone="warn">{t("tvNoDuid")}</Notice>}
      {alreadySigned && <Toggle checked={resign} onChange={setResign} label={t("installResign")} />}
      {alreadySigned && <p className="-mt-2 text-xs text-[var(--muted)]">{t("installResignHint")}</p>}

      {willSign && tv.duid && (profiles.length > 0 ? (
        <Field label={t("installCert")}>
          <div className="flex gap-2">
            <select value={profile} onChange={(e) => setProfile(e.target.value)} className="input min-w-0 flex-1">
              {profiles.map((p) => <option key={p.name} value={p.name}>{p.name} · {p.identity}</option>)}
            </select>
            <Button onClick={() => setDialog("create")} aria-label={t("installCertCreate")} title={t("installCertCreate")}><KeyRound size={16} aria-hidden /></Button>
          </div>
        </Field>
      ) : (
        <div className="rounded-xl border border-[var(--border)] p-4">
          <div className="flex items-start gap-3">
            <IconTile><KeyRound size={20} aria-hidden /></IconTile>
            <div className="min-w-0 flex-1">
              <p className="font-semibold">{t("installCertNone")}</p>
              <p className="mt-0.5 text-[13.5px] text-[var(--muted)]">{t("installCertNoneBody")}</p>
              <div className="mt-3 flex flex-wrap gap-2">
                <Button variant="primary" onClick={() => setDialog("create")}><KeyRound size={16} aria-hidden />{t("installCertCreate")}</Button>
                <Button onClick={() => setDialog("import")}>{t("installCertImport")}</Button>
              </div>
            </div>
          </div>
        </div>
      ))}

      <Toggle checked={runAfter} onChange={setRunAfter} label={t("installRunAfter")} />
      {cancelled && <Notice tone="info">{t("installCancelled")}</Notice>}
      {error && (
        <Notice tone="err" title={t("installFailedTitle")}
          action={error.code === "tv_author_mismatch" && engine === "builtin" && pkg.report.app_id ? <Button size="sm" onClick={() => void uninstallAndRetry()}>{t("installUninstallRetry")}</Button> : undefined}>
          <p className="text-[var(--text)]">{err(error)}</p>
          {error.detail && <Details label={t("technicalDetails")}>{error.detail}</Details>}
        </Notice>
      )}
      <div>
        <Button variant="primary" size="lg" className="w-full sm:w-auto" disabled={!canInstall} onClick={() => void install()}>
          {runAfter ? <Rocket size={18} aria-hidden /> : <Download size={18} aria-hidden />}{t("installButton")}
        </Button>
        {willSign && <p className="mt-2.5 flex items-center gap-1.5 text-xs text-[var(--muted)]"><CheckCircle2 size={13} aria-hidden />{t("installOriginalSafe")}</p>}
      </div>

      {dialog === "create" && <CreateCertificateDialog preselectDuid={tv.duid} onClose={() => setDialog(null)} onCreated={(p) => { setDialog(null); setProfile(p.name); }} />}
      {dialog === "import" && <ImportCertificateDialog onClose={() => setDialog(null)} onImported={(p) => { setDialog(null); setProfile(p.name); }} />}
      {askPassword && (
        <Modal title={t("passwordTitle")} onClose={() => setAskPassword(null)}
          footer={<><Button variant="ghost" onClick={() => setAskPassword(null)}>{t("cancel")}</Button><Button variant="primary" disabled={!password} onClick={() => void install(password)}>{t("passwordUnlock")}</Button></>}>
          <form onSubmit={(e) => { e.preventDefault(); if (password) void install(password); }} className="space-y-3">
            <p className="text-[var(--muted)]">{t("passwordBody", { name: profile })}</p>
            <Field label={t("passwordLabel")} error={askPassword === "wrong" ? t("passwordWrong") : null}>
              <input className="input" type="password" autoFocus value={password} onChange={(e) => setPassword(e.target.value)} autoComplete="off" aria-invalid={askPassword === "wrong"} />
            </Field>
          </form>
        </Modal>
      )}
    </div>
  );
}
