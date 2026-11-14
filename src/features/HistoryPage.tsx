import { CheckCircle2, CircleSlash, History, XCircle } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Button, Details, EmptyState, Modal, PageHeader } from "../components/ui";
import { useI18n } from "../hooks/i18n";
import { formatDate } from "../lib/format";
import { api } from "../services/api";
import type { HistoryEntry } from "../types";

export function HistoryPage() {
  const { t, lang, err } = useI18n();
  const [entries, setEntries] = useState<HistoryEntry[] | null>(null);
  const [confirm, setConfirm] = useState(false);
  const load = useCallback(() => void api.getHistory().then(setEntries).catch(() => setEntries([])), []);
  useEffect(load, [load]);
  if (!entries) return null;

  return (
    <div>
      <PageHeader title={t("historyTitle")} subtitle={t("historySubtitle")} action={entries.length > 0 ? <Button onClick={() => setConfirm(true)}>{t("historyClear")}</Button> : undefined} />
      {entries.length === 0 ? (
        <EmptyState icon={<History size={26} aria-hidden />} title={t("historyEmptyTitle")} body={t("historyEmptyBody")} />
      ) : (
        <ul className="card m-0 list-none divide-y divide-[var(--border)] p-0">
          {entries.map((e, i) => {
            const Icon = e.result === "success" ? CheckCircle2 : e.result === "failed" ? XCircle : CircleSlash;
            const color = e.result === "success" ? "var(--ok)" : e.result === "failed" ? "var(--err)" : "var(--muted)";
            return (
              <li key={`${e.time}-${i}`} className="fade-up px-4 py-3.5">
                <div className="flex items-start gap-3">
                  <Icon size={20} className="mt-0.5 shrink-0" style={{ color }} aria-label={t(`result_${e.result}`)} />
                  <div className="min-w-0 flex-1">
                    <p className="truncate font-medium">{e.app_name ?? e.file_name}{e.version ? <span className="font-normal text-[var(--muted)]"> · {e.version}</span> : null}</p>
                    <p className="truncate text-[13px] text-[var(--muted)]">{e.tv_name} · {e.tv_ip}</p>
                    {e.error_code && <p className="mt-1 text-[13px]" style={{ color }}>{err(e.error_code)}</p>}
                    {e.detail && <Details label={t("technicalDetails")}>{e.detail}</Details>}
                  </div>
                  <span className="shrink-0 text-xs text-[var(--muted)]">{formatDate(e.time, lang, true)}</span>
                </div>
              </li>
            );
          })}
        </ul>
      )}
      {confirm && (
        <Modal title={t("historyClear")} onClose={() => setConfirm(false)}
          footer={<><Button variant="ghost" onClick={() => setConfirm(false)}>{t("cancel")}</Button><Button variant="danger" onClick={() => void api.clearHistory().then(() => { setConfirm(false); load(); })}>{t("historyClear")}</Button></>}>
          <p>{t("historyClearConfirm")}</p>
        </Modal>
      )}
    </div>
  );
}
