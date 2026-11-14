import { ExternalLink, FileKey, KeyRound, LockKeyhole } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useApp } from "../app/context";
import { Button, Details, Field, Modal, Notice, Progress, Toggle, useToast } from "../components/ui";
import { useI18n } from "../hooks/i18n";
import type { Key } from "../i18n";
import { opId, passwordProblem, validProfileName } from "../lib/format";
import { api, on, pickFile, toApiError } from "../services/api";
import type { ApiError, CertProgress, CertStage, CertificateProfile } from "../types";

/** Create a Samsung certificate for one or more connected TVs. */
export function CreateCertificateDialog({ preselectDuid, onClose, onCreated }: { preselectDuid?: string | null; onClose: () => void; onCreated: (p: CertificateProfile) => void }) {
  const { t, err } = useI18n();
  const { settings, refresh } = useApp();
  const toast = useToast();
  const tvs = useMemo(() => settings.devices.filter((d) => !!d.duid), [settings.devices]);
  const [selected, setSelected] = useState<string[]>(() => {
    const first = preselectDuid && tvs.some((d) => d.duid === preselectDuid) ? preselectDuid : tvs[0]?.duid;
    return first ? [first] : [];
  });
  const suggestion = useMemo(() => {
    const tv = tvs.find((d) => d.duid === selected[0]);
    let base = `EasyTizen-${tv?.ip.split(".").at(-1) ?? "TV"}`;
    let n = 2;
    while (settings.certificate_profiles.some((p) => p.name === base)) base = `EasyTizen-${tv?.ip.split(".").at(-1) ?? "TV"}-${n++}`;
    return base;
  }, [tvs, selected, settings.certificate_profiles]);
  const [name, setName] = useState(suggestion);
  const [identity, setIdentity] = useState("EasyTizen User");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const [stage, setStage] = useState<CertStage | null>(null);
  const [url, setUrl] = useState<string | null>(null);
  const [error, setError] = useState<ApiError | null>(null);
  const [touched, setTouched] = useState(false);
  const op = useRef<string | null>(null);

  useEffect(() => {
    const off = on<CertProgress>("cert-progress", (p) => { if (p.op_id === op.current) { setStage(p.stage); if (p.url) setUrl(p.url); } });
    return () => { void off.then((f) => f()); };
  }, []);

  const nameError = touched && !validProfileName(name.trim()) ? t("certInvalidProfile") : null;
  const passwordError = touched && passwordProblem(password) ? t("certPasswordWeak") : null;
  const confirmError = touched && password !== confirm ? t("certPasswordMismatch") : null;
  const valid = selected.length > 0 && validProfileName(name.trim()) && identity.trim().length >= 2 && !passwordProblem(password) && password === confirm;

  const create = async () => {
    setTouched(true);
    if (!valid) return;
    const id = opId();
    op.current = id;
    setBusy(true); setError(null); setStage(null); setUrl(null);
    try {
      const profile = await api.createCertificate({ profileName: name.trim(), identity: identity.trim(), password, duids: selected }, id);
      await refresh();
      toast("ok", t("certCreated", { name: profile.name }));
      onCreated(profile);
    } catch (e) {
      const parsed = toApiError(e);
      if (parsed.code !== "cancelled") setError(parsed);
    } finally {
      setBusy(false); op.current = null;
    }
  };
  const close = () => { if (busy && op.current) void api.cancel(op.current); onClose(); };

  return (
    <Modal title={t("certCreateTitle")} onClose={close} wide
      footer={busy
        ? <Button onClick={() => op.current && void api.cancel(op.current)}>{t("cancel")}</Button>
        : <><Button variant="ghost" onClick={onClose}>{t("cancel")}</Button><Button variant="primary" disabled={tvs.length === 0} onClick={() => void create()}><KeyRound size={16} aria-hidden />{t("certCreateButton")}</Button></>}>
      {tvs.length === 0 ? <Notice tone="warn">{t("certNeedTv")}</Notice> : busy ? (
        <div className="py-6 text-center">
          <span className="relative mx-auto grid h-14 w-14 place-items-center rounded-2xl bg-[var(--accent-soft)] text-[var(--accent)]">
            <span className="pulse-ring absolute inset-0 rounded-2xl border-2 border-[var(--accent)]" /><KeyRound size={24} aria-hidden />
          </span>
          <p className="mt-4 font-medium">{stage ? t(`certStage_${stage}` as Key) : t("certStageSdk")}</p>
          {(stage === "signin" || !stage) && <p className="mt-1 text-[13px] text-[var(--muted)]">{t("certReturnHint")}</p>}
          <div className="mx-auto mt-4 max-w-xs"><Progress value={stage ? (["signin", "author", "distributor", "saving"].indexOf(stage) + 1) / 5 : null} /></div>
          {stage === "signin" && url && <Button size="sm" className="mt-4" onClick={() => void api.openExternal(url)}><ExternalLink size={14} aria-hidden />{t("certOpenAgain")}</Button>}
        </div>
      ) : (
        <div className="space-y-4">
          <div>
            <span className="mb-1.5 block text-[13px] font-medium">{t("certForTvs")}</span>
            <div className="space-y-1.5">
              {tvs.map((d) => {
                const checked = selected.includes(d.duid!);
                return (
                  <label key={d.duid} className={`flex cursor-pointer items-center gap-3 rounded-[10px] border px-3 py-2 transition-colors ${checked ? "border-[var(--accent)] bg-[var(--accent-soft)]" : "border-[var(--border-strong)]"}`}>
                    <input type="checkbox" className="accent-[var(--accent)]" checked={checked} onChange={() => setSelected((s) => (checked ? s.filter((x) => x !== d.duid) : [...s, d.duid!]))} />
                    <span className="min-w-0 flex-1 truncate font-medium">{d.name}</span>
                    <span className="selectable text-xs text-[var(--muted)]">{d.ip}</span>
                  </label>
                );
              })}
            </div>
          </div>
          <div className="grid gap-4 sm:grid-cols-2">
            <Field label={t("certProfileName")} hint={t("certProfileHint")} error={nameError}><input className="input" value={name} onChange={(e) => setName(e.target.value)} autoComplete="off" spellCheck={false} aria-invalid={!!nameError} /></Field>
            <Field label={t("certIdentity")} hint={t("certIdentityHint")}><input className="input" value={identity} onChange={(e) => setIdentity(e.target.value)} autoComplete="name" /></Field>
            <Field label={t("certPassword")} error={passwordError}><input className="input" type="password" value={password} onChange={(e) => setPassword(e.target.value)} autoComplete="new-password" aria-invalid={!!passwordError} /></Field>
            <Field label={t("certPasswordConfirm")} error={confirmError}><input className="input" type="password" value={confirm} onChange={(e) => setConfirm(e.target.value)} autoComplete="new-password" aria-invalid={!!confirmError} /></Field>
          </div>
          <p className="flex items-start gap-2 text-xs text-[var(--muted)]"><LockKeyhole size={14} className="mt-0.5 shrink-0" aria-hidden />{t("certPasswordHint")}</p>
          <Notice tone="info">{t("certSamsungNote")}</Notice>
          {error && <Notice tone="err" title={err(error)}>{error.detail && <Details label={t("technicalDetails")}>{error.detail}</Details>}</Notice>}
        </div>
      )}
    </Modal>
  );
}

/** Import an author + distributor pair created elsewhere (e.g. Tizen Studio). */
export function ImportCertificateDialog({ onClose, onImported }: { onClose: () => void; onImported: (p: CertificateProfile) => void }) {
  const { t, err } = useI18n();
  const { refresh, info } = useApp();
  const toast = useToast();
  const [name, setName] = useState("Imported-1");
  const [author, setAuthor] = useState<string | null>(null);
  const [distributor, setDistributor] = useState<string | null>(null);
  const [authorPassword, setAuthorPassword] = useState("");
  const [distributorPassword, setDistributorPassword] = useState("");
  const [same, setSame] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ApiError | null>(null);

  const pick = async (set: (v: string) => void) => { const p = await pickFile(info.mobile ? undefined : ["p12", "pfx"]); if (p) set(p); };
  const valid = validProfileName(name.trim()) && !!author && !!distributor && !!authorPassword && (same || !!distributorPassword);
  const run = async () => {
    setBusy(true); setError(null);
    try {
      const profile = await api.importCertificate({ profileName: name.trim(), authorSource: author!, distributorSource: distributor!, authorPassword, distributorPassword: same ? authorPassword : distributorPassword });
      await refresh();
      toast("ok", t("certImportedOk", { name: profile.name }));
      onImported(profile);
    } catch (e) { setError(toApiError(e)); }
    finally { setBusy(false); }
  };
  const fileButton = (value: string | null, set: (v: string) => void) => (
    <button type="button" onClick={() => pick(set)} className="input flex items-center gap-2 text-left">
      <FileKey size={16} className="shrink-0 text-[var(--muted)]" aria-hidden />
      <span className={`min-w-0 flex-1 truncate ${value ? "" : "text-[var(--faint)]"}`}>{value ? (value.startsWith("content://") ? t("certFileChosen") : value.split(/[\\/]/).pop()) : t("certChooseFile")}</span>
    </button>
  );

  return (
    <Modal title={t("certImportTitle")} onClose={onClose} wide
      footer={<><Button variant="ghost" onClick={onClose}>{t("cancel")}</Button><Button variant="primary" busy={busy} disabled={!valid} onClick={() => void run()}>{t("certImportButton")}</Button></>}>
      <div className="space-y-4">
        <p className="text-[var(--muted)]">{t("certImportBody")}</p>
        <Field label={t("certProfileName")} hint={t("certProfileHint")} error={!validProfileName(name.trim()) ? t("certInvalidProfile") : null}><input className="input" value={name} onChange={(e) => setName(e.target.value)} autoComplete="off" spellCheck={false} /></Field>
        <div className="grid gap-4 sm:grid-cols-2">
          <Field label={t("certAuthorFile")}>{fileButton(author, setAuthor)}</Field>
          <Field label={t("certFilePassword")}><input className="input" type="password" value={authorPassword} onChange={(e) => setAuthorPassword(e.target.value)} autoComplete="off" /></Field>
          <Field label={t("certDistributorFile")}>{fileButton(distributor, setDistributor)}</Field>
          {!same && <Field label={t("certFilePassword")}><input className="input" type="password" value={distributorPassword} onChange={(e) => setDistributorPassword(e.target.value)} autoComplete="off" /></Field>}
        </div>
        <Toggle checked={same} onChange={setSame} label={t("certSamePassword")} />
        {error && <Notice tone="err" title={err(error)}>{error.detail && <Details label={t("technicalDetails")}>{error.detail}</Details>}</Notice>}
      </div>
    </Modal>
  );
}
