import { createContext, useContext, useMemo, type ReactNode } from "react";
import { resolveLang, translate, type Key, type Lang } from "../i18n";

interface Ctx {
  lang: Lang;
  t: (key: Key, params?: Record<string, string | number>) => string;
}
const I18nContext = createContext<Ctx>({ lang: "en", t: (k) => k });

export function I18nProvider({ pref, children }: { pref: "auto" | Lang; children: ReactNode }) {
  const value = useMemo<Ctx>(() => {
    const lang = resolveLang(pref);
    return { lang, t: (key, params) => translate(lang, key, params) };
  }, [pref]);
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export const useI18n = () => useContext(I18nContext);
