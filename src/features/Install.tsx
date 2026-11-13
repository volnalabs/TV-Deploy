import { Download, KeyRound, LockKeyhole, ShieldCheck } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Button, Pill } from "../components/ui";
import { useI18n } from "../hooks/i18n";
import { api, toApiError } from "../services/api";
import type { CertificateProfile, PackageReport, TvInfo } from "../types";

type InstallPanelProps = {
  tv: TvInfo | null;
  report: PackageReport | null;
  packagePath: string | null;
  profiles: CertificateProfile[];
  onProfilesChanged: () => void;
};

const inputClass =
  "mt-1 h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--text)] outline-none focus:border-[var(--accent)]";

export function InstallPanel({
  tv,
  report,
  packagePath,
  profiles,
  onProfilesChanged,
}: InstallPanelProps) {
  const { t } = useI18n();
  const [selectedProfile, setSelectedProfile] = useState("");
  const [showCreator, setShowCreator] = useState(false);
  const [profileName, setProfileName] = useState("TVDeploy");
  const [identity, setIdentity] = useState("TV Deploy User");
  const [password, setPassword] = useState("");
  const [passwordConfirm, setPasswordConfirm] = useState("");
  const [runAfterInstall, setRunAfterInstall] = useState(false);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const tvProfiles = useMemo(
    () => profiles.filter((p) => !!tv?.duid && p.duid === tv.duid),
    [profiles, tv?.duid],
  );

  useEffect(() => {
    if (tv) {
      setProfileName(`TVDeploy-${tv.ip.split(".").at(-1) ?? "TV"}`);
    }
  }, [tv?.ip]);

  useEffect(() => {
    if (tvProfiles.length > 0 && !tvProfiles.some((p) => p.name === selectedProfile)) {
      setSelectedProfile(tvProfiles[0].name);
    } else if (tvProfiles.length === 0) {
      setSelectedProfile("");
    }
  }, [tvProfiles, selectedProfile]);

  useEffect(() => {
    setStatus(null);
    setError(null);
  }, [tv?.ip, tv?.duid, report?.file_name]);

  const createCertificate = async () => {
    if (!tv?.duid) {
      setError(t("certNeedDuid"));
      return;
    }
    if (password !== passwordConfirm) {
      setError(t("certPasswordMismatch"));
      return;
    }
    if (!/^[A-Za-z0-9_-]{3,64}$/.test(profileName.trim())) {
      setError(t("certInvalidProfile"));
      return;
    }
    setBusy(true);
    setError(null);
    setStatus(t("certStartLogin"));
    try {
      const created = await api.createSamsungCertificate({
        ip: tv.ip,
        port: tv.port,
        duid: tv.duid,
        profileName: profileName.trim(),
        identity: identity.trim(),
        password,
      });
      setSelectedProfile(created.name);
      setPassword("");
      setPasswordConfirm("");
      setShowCreator(false);
      setStatus(t("certSuccess"));
      onProfilesChanged();
    } catch (e) {
      const parsed = toApiError(e);
      setError(parsed.detail || parsed.code || t("certCreateError"));
      setStatus(null);
    } finally {
      setBusy(false);
    }
  };

  const install = async () => {
    if (!tv || !report || !packagePath || !selectedProfile || !tv.duid) return;
    setBusy(true);
    setError(null);
    setStatus(t("installWorking"));
    try {
      const result = await api.installPackage({
        packagePath,
        profileName: selectedProfile,
        ip: tv.ip,
        port: tv.port,
        duid: tv.duid,
        runAfterInstall,
      });
      const appLabel = result.app_name || report.app_name || report.file_name;
      setStatus(t("installSuccess", { name: appLabel }));
    } catch (e) {
      const parsed = toApiError(e);
      setError(parsed.detail || parsed.code || t("installFailure"));
      setStatus(null);
    } finally {
      setBusy(false);
    }
  };

  const canInstall = !!tv && !!tv.duid && !!report && !!packagePath && !!selectedProfile && !busy;

  return (
    <div className="space-y-4">
      {!tv && <p className="m-0 text-sm text-[var(--muted)]">{t("installNeedTv")}</p>}
      {tv && !tv.duid && (
        <div className="rounded-lg border border-[var(--warn)]/30 bg-[var(--surface)] p-3 text-sm text-[var(--warn)]">
          {t("certNeedDuid")}
        </div>
      )}
      {!report && <p className="m-0 text-sm text-[var(--muted)]">{t("installNeedApp")}</p>}

      {tv && report && tv.duid && (
        <div className="rounded-xl border border-[var(--border)] bg-[var(--surface)] p-4">
          <div className="flex items-center gap-3">
            <span className="grid h-9 w-9 place-items-center rounded-lg bg-[var(--accent-soft)] text-[var(--accent)]">
              <KeyRound size={18} aria-hidden />
            </span>
            <div className="min-w-0 flex-1">
              <p className="m-0 font-semibold">{t("certTitle")}</p>
              <p className="m-0 mt-0.5 text-xs text-[var(--muted)]">{t("certSubtitle")}</p>
            </div>
            {tvProfiles.length > 0 && <Pill tone="ok"><ShieldCheck size={12} aria-hidden />{t("certReady")}</Pill>}
          </div>

          {tvProfiles.length > 0 && !showCreator && (
            <div className="mt-4">
              <label className="block text-xs font-medium text-[var(--muted)]" htmlFor="tvdeploy-profile">
                {t("certSelect")}
              </label>
              <select
                id="tvdeploy-profile"
                value={selectedProfile}
                onChange={(e) => setSelectedProfile(e.target.value)}
                className={inputClass}
              >
                {tvProfiles.map((p) => <option key={p.name} value={p.name}>{p.name} · {p.identity}</option>)}
              </select>
              <button
                type="button"
                className="mt-3 text-xs font-medium text-[var(--accent)] hover:underline"
                onClick={() => setShowCreator(true)}
              >
                {t("certCreateAnother")}
              </button>
            </div>
          )}

          {(tvProfiles.length === 0 || showCreator) && (
            <div className="mt-4 space-y-3">
              <p className="m-0 text-sm text-[var(--muted)]">{t("certForTv", { ip: tv.ip })}</p>
              <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                <label className="block text-xs font-medium text-[var(--muted)]">
                  {t("certProfileName")}
                  <input value={profileName} onChange={(e) => setProfileName(e.target.value)} className={inputClass} autoComplete="off" />
                </label>
                <label className="block text-xs font-medium text-[var(--muted)]">
                  {t("certIdentity")}
                  <input value={identity} onChange={(e) => setIdentity(e.target.value)} className={inputClass} autoComplete="name" />
                </label>
              </div>
              <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                <label className="block text-xs font-medium text-[var(--muted)]">
                  {t("certLocalPassword")}
                  <input type="password" value={password} onChange={(e) => setPassword(e.target.value)} className={inputClass} autoComplete="new-password" />
                </label>
                <label className="block text-xs font-medium text-[var(--muted)]">
                  {t("certConfirmPassword")}
                  <input type="password" value={passwordConfirm} onChange={(e) => setPasswordConfirm(e.target.value)} className={inputClass} autoComplete="new-password" />
                </label>
              </div>
              <p className="m-0 flex items-start gap-2 text-xs text-[var(--muted)]">
                <LockKeyhole size={14} className="mt-0.5 shrink-0" aria-hidden />
                {t("certPasswordHint")}
              </p>
              <div className="flex flex-wrap items-center gap-2">
                <Button variant="primary" busy={busy} disabled={!password || !passwordConfirm || !identity.trim()} onClick={() => void createCertificate()}>
                  <KeyRound size={14} aria-hidden />{t("certCreate")}
                </Button>
                {showCreator && tvProfiles.length > 0 && (
                  <Button variant="ghost" disabled={busy} onClick={() => setShowCreator(false)}>{t("cancel")}</Button>
                )}
              </div>
            </div>
          )}
        </div>
      )}

      {tv && report && tv.duid && tvProfiles.length > 0 && (
        <div className="rounded-xl border border-[var(--border)] bg-[var(--surface)] p-4">
          <div className="flex items-center gap-3">
            <span className="grid h-9 w-9 place-items-center rounded-lg bg-[var(--accent-soft)] text-[var(--accent)]">
              <Download size={18} aria-hidden />
            </span>
            <div className="min-w-0 flex-1">
              <p className="m-0 font-semibold">{t("step3")}</p>
              <p className="m-0 mt-0.5 truncate text-xs text-[var(--muted)]">{report.app_name || report.file_name} · {tv.name}</p>
            </div>
          </div>
          <p className="m-0 mt-3 text-xs text-[var(--muted)]">{t("installSignNotice")}</p>
          <label className="mt-3 flex items-center gap-2 text-sm text-[var(--muted)]">
            <input type="checkbox" checked={runAfterInstall} onChange={(e) => setRunAfterInstall(e.target.checked)} />
            {t("installRunAfter")}
          </label>
          <div className="mt-4 flex flex-wrap items-center gap-3">
            <Button variant="primary" busy={busy} disabled={!canInstall} onClick={() => void install()}>
              <Download size={15} aria-hidden />{t("installButton")}
            </Button>
            {selectedProfile && <span className="text-xs text-[var(--muted)]">{t("certProfile")}: {selectedProfile}</span>}
          </div>
        </div>
      )}

      {status && <p role="status" className="m-0 rounded-lg border border-[var(--ok)]/30 bg-[var(--surface)] p-3 text-sm text-[var(--ok)]">{status}</p>}
      {error && <p role="alert" className="m-0 rounded-lg border border-[var(--err)]/30 bg-[var(--surface)] p-3 text-sm text-[var(--err)]">{error}</p>}
    </div>
  );
}
