import { createContext, useContext, useMemo, type ReactNode } from "react";
import { hasKey, resolveLang, translate, type Key, type Lang } from "../i18n";
import type { ApiError } from "../types";

interface Ctx {
  lang: Lang;
  t: (key: Key, params?: Record<string, string | number>) => string;
  /** Human text for a backend error code, with a generic fallback for unknown codes. */
  err: (error: ApiError | string) => string;
}
const I18nContext = createContext<Ctx>({ lang: "en", t: (k) => k, err: (e) => (typeof e === "string" ? e : e.code) });

export function I18nProvider({ pref, children }: { pref: "auto" | Lang; children: ReactNode }) {
  const value = useMemo<Ctx>(() => {
    const lang = resolveLang(pref);
    const t: Ctx["t"] = (key, params) => translate(lang, key, params);
    return {
      lang,
      t,
      err: (error) => {
        const key = `err_${typeof error === "string" ? error : error.code}`;
        return hasKey(key) ? t(key) : t("err_generic");
      },
    };
  }, [pref]);
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export const useI18n = () => useContext(I18nContext);
