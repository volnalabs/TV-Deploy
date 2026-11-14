import { AlertTriangle } from "lucide-react";
import { useI18n } from "../hooks/i18n";
import type { Key } from "../i18n";
import type { Diagnosis } from "../types";
import { Details } from "./ui";

const CHECKS: Partial<Record<Diagnosis, Key[]>> = {
  tv_unreachable: ["dx_tv_unreachable_c1", "dx_tv_unreachable_c2", "dx_tv_unreachable_c3"],
  port_closed: ["dx_port_closed_c1", "dx_port_closed_c2", "dx_port_closed_c3"],
};

/** Plain-language explanation of what went wrong, with concrete things to check. */
export function DiagnosisPanel({ diagnosis, technical, hostIp }: { diagnosis: Diagnosis; technical?: string | null; hostIp?: string | null }) {
  const { t } = useI18n();
  const params = { hostIp: hostIp ?? "—" };
  return (
    <div role="alert" className="fade-up rounded-xl bg-[var(--warn-soft)] p-4">
      <div className="flex gap-3">
        <AlertTriangle size={18} className="mt-0.5 shrink-0 text-[var(--warn)]" aria-hidden />
        <div className="min-w-0 flex-1">
          <p className="font-semibold">{t(`dx_${diagnosis}` as Key)}</p>
          <p className="mt-1 text-[var(--muted)]">{t(`dx_${diagnosis}_body` as Key, params)}</p>
          {CHECKS[diagnosis] && (
            <ul className="m-0 mt-2 list-disc space-y-1 pl-5 text-[var(--muted)]">
              {CHECKS[diagnosis]!.map((k) => <li key={k}>{t(k, params)}</li>)}
            </ul>
          )}
          {technical && <Details label={t("technicalDetails")}>{technical}</Details>}
        </div>
      </div>
    </div>
  );
}
