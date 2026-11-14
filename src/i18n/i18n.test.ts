import { describe, expect, it } from "vitest";
import { en } from "./en";
import { ru } from "./ru";
import { resolveLang, translate } from "./index";

describe("i18n", () => {
  it("ru has exactly the same keys as en, none empty", () => {
    expect(Object.keys(ru).sort()).toEqual(Object.keys(en).sort());
    for (const v of Object.values(ru)) expect(v.trim().length).toBeGreaterThan(0);
  });
  it("placeholders match between languages", () => {
    const ph = (s: string) => (s.match(/\{\w+\}/g) ?? []).sort().join();
    for (const k of Object.keys(en) as (keyof typeof en)[]) expect(ph(ru[k])).toBe(ph(en[k]));
  });
  it("resolves language", () => {
    expect(resolveLang("auto", "ru-RU")).toBe("ru");
    expect(resolveLang("auto", "de-DE")).toBe("en");
    expect(resolveLang("en", "ru-RU")).toBe("en");
  });
  it("interpolates", () => {
    expect(translate("en", "version", { v: "1.0" })).toBe("Version 1.0");
    expect(translate("ru", "certCreated", { name: "A" })).toBe("Сертификат «A» готов.");
  });
});
