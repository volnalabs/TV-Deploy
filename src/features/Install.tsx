import { Download, Info } from "lucide-react";
import { Button } from "../components/ui";
import { useI18n } from "../hooks/i18n";
import type { PackageReport, TvInfo } from "../types";

/**
 * Signing and installation are NOT implemented in this iteration. The button is therefore
 * always disabled and says so honestly — it never pretends to install.
 */
export function InstallPanel({ tv, report }: { tv: TvInfo | null; report: PackageReport | null }) {
  const { t } = useI18n();
  const hint = !tv ? t("installNeedTv") : !report ? t("installNeedApp") : null;
  return (
    <div>
      {tv && <p className="m-0 mb-3 text-[var(--muted)]">{t("tvChosen")}: <strong className="text-[var(--text)]">{tv.name}</strong> ({tv.ip})</p>}
      <div className="flex items-center gap-3">
        <Button variant="primary" disabled>
          <Download size={15} aria-hidden />
          {t("installButton")}
        </Button>
        {hint && <span className="text-[var(--muted)]">{hint}</span>}
      </div>
      <p className="m-0 mt-3 flex items-start gap-2 text-xs text-[var(--muted)]"><Info size={14} className="mt-0.5 shrink-0" aria-hidden />{t("installSoon")}</p>
    </div>
  );
}
