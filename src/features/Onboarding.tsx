import { ArrowRight, KeyRound, Search, ShieldCheck, Smartphone } from "lucide-react";
import { useState } from "react";
import { useApp } from "../app/context";
import { Logo } from "../components/Logo";
import { Button, Segmented } from "../components/ui";
import { useI18n } from "../hooks/i18n";
import { api } from "../services/api";
import { DevModeGuide } from "./DevModeGuide";
import { EngineChoice } from "./EngineChoice";
import { ToolsPanel } from "./Setup";

/** First-run guide: welcome → tools (desktop) → Developer Mode on the TV → done. */
export function Onboarding({ onDone }: { onDone: () => void }) {
  const { t } = useI18n();
  const { info, settings, refresh } = useApp();
  const steps = info.mobile ? (["welcome", "tv", "done"] as const) : (["welcome", "tools", "tv", "done"] as const);
  const [index, setIndex] = useState(0);
  const step = steps[index];
  const finish = async () => { await api.completeOnboarding(); await refresh(); onDone(); };
  const next = () => (index === steps.length - 1 ? void finish() : setIndex(index + 1));
  const setLanguage = async (language: "auto" | "ru" | "en") => {
    await api.savePreferences({ theme: settings.theme, language, default_port: settings.default_port, timeout_ms: settings.timeout_ms, check_updates: settings.check_updates });
    await refresh();
  };
  const points = [[Search, t("obPoint1")], [KeyRound, t("obPoint2")], [ShieldCheck, t("obPoint3")]] as const;

  return (
    <div className="fade-in safe-top safe-bottom fixed inset-0 z-30 flex flex-col bg-[var(--bg)]">
      <div className="flex items-center gap-3 px-5 py-4">
        <Logo size={26} /><span className="font-semibold tracking-tight">EasyTizen</span>
        <div className="ml-auto flex items-center gap-1.5" aria-hidden>
          {steps.map((s, i) => <span key={s} className={`h-1.5 rounded-full transition-all ${i === index ? "w-6 bg-[var(--accent)]" : i < index ? "w-1.5 bg-[var(--accent)]" : "w-1.5 bg-[var(--surface-3)]"}`} />)}
        </div>
      </div>
      <div className="min-h-0 flex-1 overflow-auto px-5">
        <div key={step} className="fade-up mx-auto max-w-xl py-4">
          {step === "welcome" && (
            <div className="pt-2 text-center sm:pt-8">
              <div className="mx-auto w-fit drop-shadow-xl"><Logo size={88} /></div>
              <h1 className="mt-6 text-[26px] leading-tight font-semibold tracking-tight">{t("obWelcome")}</h1>
              <p className="mx-auto mt-2 max-w-md text-[15px] text-[var(--muted)]">{t("obWelcomeBody")}</p>
              <ul className="mx-auto mt-6 max-w-sm list-none space-y-2.5 p-0 text-left">
                {points.map(([Icon, text]) => (
                  <li key={text} className="flex items-center gap-3"><span className="grid h-9 w-9 shrink-0 place-items-center rounded-xl bg-[var(--accent-soft)] text-[var(--accent)]"><Icon size={18} aria-hidden /></span>{text}</li>
                ))}
                {info.mobile && <li className="flex items-center gap-3"><span className="grid h-9 w-9 shrink-0 place-items-center rounded-xl bg-[var(--ok-soft)] text-[var(--ok)]"><Smartphone size={18} aria-hidden /></span>{t("obAndroidNote")}</li>}
              </ul>
              <div className="mt-7"><Segmented label={t("language")} value={settings.language} onChange={(v) => void setLanguage(v)} options={[{ value: "auto", label: t("langAuto") }, { value: "ru", label: "Русский" }, { value: "en", label: "English" }]} /></div>
            </div>
          )}
          {step === "tools" && (
            <div className="space-y-4">
              <h1 className="text-[22px] font-semibold tracking-tight">{t("obToolsTitle")}</h1>
              <EngineChoice />
              {settings.engine === "sdk" && <div className="card p-5"><ToolsPanel /></div>}
            </div>
          )}
          {step === "tv" && (
            <div className="space-y-4">
              <h1 className="text-[22px] font-semibold tracking-tight">{t("obTvTitle")}</h1>
              <div className="card p-5"><DevModeGuide /></div>
            </div>
          )}
          {step === "done" && (
            <div className="pt-6 text-center sm:pt-12">
              <span className="mx-auto grid h-20 w-20 place-items-center rounded-full bg-[var(--ok-soft)] text-[var(--ok)]">
                <svg width="36" height="36" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" aria-hidden><path className="check-draw" d="M5 12.5l4.5 4.5L19 7.5" /></svg>
              </span>
              <h1 className="mt-6 text-[24px] font-semibold tracking-tight">{t("obDoneTitle")}</h1>
              <p className="mx-auto mt-2 max-w-md text-[15px] text-[var(--muted)]">{t("obDoneBody")}</p>
            </div>
          )}
        </div>
      </div>
      <div className="flex items-center gap-2 border-t border-[var(--border)] px-5 py-3.5">
        {index > 0 ? <Button variant="ghost" onClick={() => setIndex(index - 1)}>{t("back")}</Button> : <Button variant="ghost" onClick={() => void finish()}>{t("skip")}</Button>}
        <div className="ml-auto flex gap-2">
          {step === "tools" && <Button variant="ghost" onClick={next}>{t("obLater")}</Button>}
          <Button variant="primary" onClick={next}>{index === steps.length - 1 ? t("obStart") : t("next")}<ArrowRight size={16} aria-hidden /></Button>
        </div>
      </div>
    </div>
  );
}
