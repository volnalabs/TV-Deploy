import { AlertTriangle, ChevronDown } from "lucide-react";
import { useState } from "react";
import { useI18n } from "../hooks/i18n";
import type { Key } from "../i18n";
import type { Diagnosis } from "../types";

const CHECKS: Partial<Record<Diagnosis, Key[]>> = {
  tv_unreachable: ["dx_tv_unreachable_c1", "dx_tv_unreachable_c2", "dx_tv_unreachable_c3"],
  port_closed: ["dx_port_closed_c1", "dx_port_closed_c2", "dx_port_closed_c3"],
};

/** Plain-language explanation of what went wrong, with concrete things to check. */
export function DiagnosisPanel({ diagnosis, technical, hostIp }: { diagnosis: Diagnosis; technical?: string | null; hostIp?: string | null }) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const title = `dx_${diagnosis}` as Key;
  const body = `dx_${diagnosis}_body` as Key;
  return (
    <div role="alert" className="fade-up rounded-xl border border-[var(--border)] bg-[var(--surface)] p-4">
      <div className="flex gap-3">
        <AlertTriangle size={18} className="mt-0.5 shrink-0 text-[var(--warn)]" aria-hidden />
        <div className="min-w-0 flex-1">
          <p className="m-0 font-semibold">{t(title)}</p>
          <p className="m-0 mt-1 text-[var(--muted)]">{t(body)}</p>
          {CHECKS[diagnosis] && (
            <ul className="m-0 mt-2 list-disc space-y-1 pl-5 text-[var(--muted)]">
              {CHECKS[diagnosis]!.map((k) => (
                <li key={k}>{t(k, { hostIp: hostIp ?? "—" })}</li>
              ))}
            </ul>
          )}
          {technical && (
            <div className="mt-3">
              <button onClick={() => setOpen(!open)} className="flex cursor-pointer items-center gap-1 text-xs text-[var(--muted)] hover:text-[var(--text)]" aria-expanded={open}>
                <ChevronDown size={14} className={open ? "rotate-180 transition-transform" : "transition-transform"} aria-hidden />
                {t("technicalDetails")}
              </button>
              {open && <pre className="selectable mt-2 max-h-40 overflow-auto whitespace-pre-wrap rounded-lg bg-[var(--surface-2)] p-3 text-xs">{technical}</pre>}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
