import { Copy } from "lucide-react";
import { useApp } from "../app/context";
import { copyText, useToast } from "../components/ui";
import { useI18n } from "../hooks/i18n";
import type { Key } from "../i18n";

/** Step-by-step Developer Mode instructions, with this device's address filled in. */
export function DevModeGuide() {
  const { t } = useI18n();
  const { info } = useApp();
  const toast = useToast();
  const steps: Key[] = ["devStep1", "devStep2", "devStep3", "devStep4", "devStep5"];
  return (
    <div>
      <ol className="m-0 list-none space-y-2.5 p-0">
        {steps.map((key, i) => (
          <li key={key} className="flex gap-3">
            <span className="mt-0.5 grid h-6 w-6 shrink-0 place-items-center rounded-full bg-[var(--accent-soft)] text-xs font-semibold text-[var(--accent)]">{i + 1}</span>
            <div className="min-w-0 flex-1">
              <p>{t(key)}</p>
              {key === "devStep4" && (
                info.host_ip ? (
                  <button type="button" onClick={() => copyText(info.host_ip!).then((ok) => ok && toast("ok", t("copied")))}
                    className="selectable mt-1.5 inline-flex items-center gap-2 rounded-lg bg-[var(--surface-2)] px-3 py-1.5 font-mono text-[15px] font-semibold tracking-wide hover:bg-[var(--surface-3)]">
                    {info.host_ip}<Copy size={14} className="text-[var(--muted)]" aria-label={t("copy")} />
                  </button>
                ) : <p className="mt-1 text-[var(--warn)]">{t("devNoIp")}</p>
              )}
            </div>
          </li>
        ))}
      </ol>
      <p className="mt-3 text-xs text-[var(--muted)]">{t("devNote")}</p>
    </div>
  );
}
