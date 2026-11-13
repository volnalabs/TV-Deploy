import { en, type Key } from "./en";
import { ru } from "./ru";

export type Lang = "en" | "ru";
export type { Key };

/** Add a language: create `xx.ts` exporting `Record<Key, string>` and register it here. */
const dictionaries: Record<Lang, Record<Key, string>> = { en, ru };

export function resolveLang(pref: "auto" | Lang, navLang: string = typeof navigator !== "undefined" ? navigator.language : "en"): Lang {
  if (pref !== "auto") return pref;
  return navLang.toLowerCase().startsWith("ru") ? "ru" : "en";
}

export function translate(lang: Lang, key: Key, params?: Record<string, string | number>): string {
  let s = dictionaries[lang][key] ?? en[key];
  if (params) for (const [k, v] of Object.entries(params)) s = s.replaceAll(`{${k}}`, String(v));
  return s;
}

export function hasKey(k: string): k is Key {
  return k in en;
}
