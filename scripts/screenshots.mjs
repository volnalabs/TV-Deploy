#!/usr/bin/env node
// Regenerates the README screenshots from the real interface.
//
//   npx vite build && npx vite preview --port 4173 &
//   node scripts/screenshots.mjs            (needs Playwright with Chromium)
//
// The interface is the application's own frontend; the backend is the in-memory demo
// backend (src/services/demo.ts) with an emulated TV, so no television is required.
import { createRequire } from "node:module";

// `require` so a globally installed Playwright (NODE_PATH) is found as well.
const { chromium } = createRequire(import.meta.url)("playwright");

const BASE = process.env.BASE_URL ?? "http://localhost:4173";
const OUT = "docs/img";
const browser = await chromium.launch();

async function page({ mobile = false, theme = "dark", query = "" }) {
  const p = await browser.newPage({
    viewport: mobile ? { width: 412, height: 860 } : { width: 1180, height: 800 },
    deviceScaleFactor: 2,
    colorScheme: theme,
  });
  await p.goto(`${BASE}/?lang=ru&theme=${theme}${mobile ? "&platform=android" : ""}${query}`);
  await p.waitForTimeout(700);
  return p;
}
const shot = async (p, name) => { await p.waitForTimeout(450); await p.screenshot({ path: `${OUT}/${name}.png` }); console.log(name); };
const click = (p, text) => p.getByRole("button", { name: text, exact: false }).first().click();

async function flow(p, prefix) {
  await click(p, "Найти телевизоры");
  await p.waitForTimeout(1500);
  await shot(p, `${prefix}-find`);
  await p.getByRole("button", { name: "Подключить" }).nth(1).click();
  await p.waitForTimeout(1200);
  await click(p, "Выберите приложение");
  await p.waitForTimeout(900);
  await shot(p, `${prefix}-ready`);
  await click(p, "Установить на телевизор");
  await p.waitForTimeout(1900);
  await shot(p, `${prefix}-progress`);
  await p.getByText("Установлено", { exact: true }).waitFor({ timeout: 15000 });
  await shot(p, `${prefix}-success`);
}

// Desktop
let p = await page({ theme: "dark" });
await flow(p, "desktop");
await click(p, "Сертификаты");
await shot(p, "desktop-certs");
await click(p, "Создать");
await shot(p, "desktop-cert-new");
await p.keyboard.press("Escape");
await click(p, "История");
await shot(p, "desktop-history");
await click(p, "Телевизоры");
await shot(p, "desktop-tvs");
await p.close();

p = await page({ theme: "light", query: "&demo=fresh" });
await shot(p, "desktop-welcome");
await click(p, "Продолжить");
// The wizard sits above the main page, which shows the same panel: take the top one.
await p.getByRole("button", { name: "Установить автоматически" }).last().click();
await p.waitForTimeout(2600);
await shot(p, "desktop-sdk");
await p.close();

// Android
p = await page({ mobile: true, theme: "light", query: "&demo=fresh" });
await shot(p, "android-welcome");
await click(p, "Продолжить");
await shot(p, "android-devmode");
await p.close();
p = await page({ mobile: true, theme: "dark" });
await flow(p, "android");
await p.getByRole("button", { name: "Сертификаты" }).click();
await shot(p, "android-certs");
await p.close();

await browser.close();
