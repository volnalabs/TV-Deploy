import { Check, CheckCircle2, Tv2 } from "lucide-react";
import { useState, type ReactNode } from "react";
import { useApp } from "../app/context";
import { Button, IconTile, Notice, PageHeader, Pill, Row } from "../components/ui";
import { useI18n } from "../hooks/i18n";
import type { OpenedPackage } from "../types";
import { InstallStep } from "./InstallStep";
import { PackageStep } from "./PackageStep";
import { ToolsPanel } from "./Setup";
import { TvFinder } from "./TvFinder";

function StepCard({ n, title, done, locked, children }: { n: number; title: string; done?: boolean; locked?: string | null; children: ReactNode }) {
  return (
    <section className={`card fade-up p-5 transition-opacity ${locked ? "opacity-60" : ""}`} style={{ animationDelay: `${(n - 1) * 60}ms` }}>
      <header className="mb-4 flex items-center gap-3">
        <span className={`grid h-7 w-7 shrink-0 place-items-center rounded-full text-[13px] font-semibold transition-colors ${done ? "brand-gradient text-white" : "bg-[var(--surface-2)] text-[var(--muted)]"}`}>
          {done ? <Check size={15} aria-hidden /> : n}
        </span>
        <h2 className="text-[16px] font-semibold tracking-tight">{title}</h2>
      </header>
      {locked ? <p className="text-[var(--muted)]">{locked}</p> : children}
    </section>
  );
}

export function InstallPage() {
  const { t } = useI18n();
  const { tv, setTv, toolsMissing } = useApp();
  const [pkg, setPkg] = useState<OpenedPackage | null>(null);

  return (
    <div>
      <PageHeader title={t("installTitle")} subtitle={t("installSubtitle")} />
      <div className="space-y-4">
        <StepCard n={1} title={t("step1")} done={!!tv}>
          {toolsMissing ? <ToolsPanel /> : tv ? (
            <div className="fade-up">
              <div className="flex items-center gap-3">
                <IconTile><Tv2 size={20} aria-hidden /></IconTile>
                <div className="min-w-0 flex-1">
                  <p className="truncate font-semibold">{tv.name}</p>
                  <Pill tone="ok"><CheckCircle2 size={12} aria-hidden />{t("tvConnected")}</Pill>
                </div>
                <Button size="sm" onClick={() => setTv(null)}>{t("tvChange")}</Button>
              </div>
              <dl className="m-0 mt-3">
                <Row label="IP">{tv.ip}:{tv.port}</Row>
                {tv.model && <Row label={t("tvModel")}>{tv.model}</Row>}
                {tv.duid && <Row label={t("tvDuid")}>{tv.duid}</Row>}
              </dl>
              {!tv.duid && <div className="mt-3"><Notice tone="warn">{t("tvNoDuid")}</Notice></div>}
            </div>
          ) : <TvFinder onConnected={setTv} />}
        </StepCard>
        <StepCard n={2} title={t("step2")} done={!!pkg}>
          <PackageStep pkg={pkg} onPackage={setPkg} />
        </StepCard>
        <StepCard n={3} title={t("step3")} locked={!tv ? t("installNeedTv") : !pkg ? t("installNeedApp") : null}>
          {tv && pkg && <InstallStep key={`${tv.ip}-${pkg.path}`} tv={tv} pkg={pkg} onReset={() => setPkg(null)} />}
        </StepCard>
      </div>
    </div>
  );
}
