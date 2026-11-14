import { KeyRound, Plus, ShieldCheck, Trash2, Upload } from "lucide-react";
import { useState } from "react";
import { useApp } from "../app/context";
import { Button, EmptyState, IconButton, IconTile, Modal, PageHeader, Pill, useToast } from "../components/ui";
import { useI18n } from "../hooks/i18n";
import { daysLeft, formatDate } from "../lib/format";
import { api, toApiError } from "../services/api";
import type { CertificateProfile } from "../types";
import { CreateCertificateDialog, ImportCertificateDialog } from "./CertDialogs";

export function CertificatesPage() {
  const { t, lang, err } = useI18n();
  const { settings, refresh, info, tv } = useApp();
  const toast = useToast();
  const [dialog, setDialog] = useState<"create" | "import" | null>(null);
  const [removing, setRemoving] = useState<CertificateProfile | null>(null);
  const [busy, setBusy] = useState(false);
  const engine = info.mobile ? "builtin" : settings.engine;

  const tvName = (duid: string) => settings.devices.find((d) => d.duid?.toLowerCase() === duid.toLowerCase())?.name ?? t("certUnknownTv", { tail: duid.slice(-4) });
  const remove = async () => {
    if (!removing) return;
    setBusy(true);
    try { await api.removeCertificate(removing.name); await refresh(); toast("ok", t("certRemoved")); setRemoving(null); }
    catch (e) { toast("err", err(toApiError(e))); }
    finally { setBusy(false); }
  };
  const actions = (
    <>
      <Button onClick={() => setDialog("import")}><Upload size={16} aria-hidden />{t("certsImport")}</Button>
      <Button variant="primary" onClick={() => setDialog("create")}><Plus size={16} aria-hidden />{t("certsCreate")}</Button>
    </>
  );

  return (
    <div>
      <PageHeader title={t("certsTitle")} subtitle={t("certsSubtitle")} action={settings.certificate_profiles.length > 0 ? actions : undefined} />
      {settings.certificate_profiles.length === 0 ? (
        <EmptyState icon={<KeyRound size={26} aria-hidden />} title={t("certsEmptyTitle")} body={t("certsEmptyBody")} action={actions} />
      ) : (
        <ul className="m-0 list-none space-y-3 p-0">
          {settings.certificate_profiles.map((p) => {
            const days = daysLeft(p.expires_at);
            const other = p.engine !== engine;
            return (
              <li key={p.name} className={`card fade-up p-4 ${other ? "opacity-70" : ""}`}>
                <div className="flex items-start gap-3">
                  <IconTile tone={days != null && days < 0 ? "err" : "accent"}><ShieldCheck size={20} aria-hidden /></IconTile>
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <p className="selectable truncate font-semibold">{p.name}</p>
                      {!info.mobile && <Pill tone="neutral">{p.engine === "sdk" ? t("certEngineSdk") : t("certEngineBuiltin")}</Pill>}
                      {p.imported && <Pill tone="neutral">{t("certImported")}</Pill>}
                      {days != null && (days < 0 ? <Pill tone="err">{t("certExpired", { date: formatDate(p.expires_at!, lang) })}</Pill> : days <= 30 ? <Pill tone="warn">{t("certExpiresSoon", { days })}</Pill> : null)}
                    </div>
                    <p className="mt-1 text-[13px] text-[var(--muted)]">{t("certAuthor")}: <span className="text-[var(--text)]">{p.identity}</span></p>
                    <p className="text-[13px] text-[var(--muted)]">{t("certTvs")}: <span className="text-[var(--text)]">{p.duids.map(tvName).join(", ")}</span></p>
                    {days != null && days > 30 && <p className="text-[13px] text-[var(--muted)]">{t("certExpires", { date: formatDate(p.expires_at!, lang) })}</p>}
                    {other && <p className="mt-1.5 text-xs text-[var(--warn)]">{t("certOtherEngine")}</p>}
                  </div>
                  <IconButton label={t("remove")} onClick={() => setRemoving(p)}><Trash2 size={16} aria-hidden /></IconButton>
                </div>
              </li>
            );
          })}
        </ul>
      )}
      {dialog === "create" && <CreateCertificateDialog preselectDuid={tv?.duid} onClose={() => setDialog(null)} onCreated={() => setDialog(null)} />}
      {dialog === "import" && <ImportCertificateDialog onClose={() => setDialog(null)} onImported={() => setDialog(null)} />}
      {removing && (
        <Modal title={t("remove")} onClose={() => setRemoving(null)}
          footer={<><Button variant="ghost" onClick={() => setRemoving(null)}>{t("cancel")}</Button><Button variant="danger" busy={busy} onClick={() => void remove()}>{t("remove")}</Button></>}>
          <p>{t("certRemoveConfirm", { name: removing.name })}</p>
        </Modal>
      )}
    </div>
  );
}
