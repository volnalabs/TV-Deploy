#!/usr/bin/env node
// Generates the README artwork (docs/readme/*.svg): hero banners, download cards, platform
// cards, feature and section icons, step badges, divider. Original artwork for EasyTizen.
//   node scripts/readme-assets.mjs
import { mkdirSync, writeFileSync } from "node:fs";

const OUT = "docs/readme";
mkdirSync(OUT, { recursive: true });
const C = { bg0: "#0C0D12", bg1: "#151823", surface: "#14161D", tile: "#1E2230", border: "#272B38", text: "#EEF0F6", muted: "#A0A6B8", faint: "#6B7286", a1: "#3F5BF6", a2: "#12B8A6", ink: "#0C0D12" };
const FONT = "Inter, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif";
const esc = (s) => String(s).replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll('"', "&quot;");
const grad = (id = "g") => `<linearGradient id="${id}" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="${C.a1}"/><stop offset="1" stop-color="${C.a2}"/></linearGradient>`;
const text = (x, y, s, size, weight, fill, anchor = "start", extra = "") => `<text x="${x}" y="${y}" font-family="${FONT}" font-size="${size}" font-weight="${weight}" fill="${fill}" text-anchor="${anchor}"${extra}>${esc(s)}</text>`;
const svg = (w, h, label, body) => `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" role="img" aria-label="${esc(label)}"><title>${esc(label)}</title>${body}</svg>\n`;
const save = (name, content) => writeFileSync(`${OUT}/${name}.svg`, content);

// 24×24 line icons, drawn for this project.
const I = {
  download: "M12 4v11M7.5 10.5L12 15l4.5-4.5M5 19.5h14",
  upload: "M12 15V4M7.5 8.5L12 4l4.5 4.5M5 19.5h14",
  tv: "M4 5h16v11H4zM9 20h6",
  tvplay: "M4 5h16v11H4zM10.3 8.3v4.4l3.7-2.2zM9 20h6",
  search: "M11 4a7 7 0 1 0 0 14a7 7 0 0 0 0-14zM20 20l-4-4",
  globe: "M12 4a8 8 0 1 0 0 16a8 8 0 0 0 0-16zM4 12h16M12 4c2.6 2.4 2.6 13.6 0 16M12 4c-2.6 2.4-2.6 13.6 0 16",
  id: "M4 6h16v12H4zM7.5 10.5h4M7.5 14h9",
  key: "M15 4a5 5 0 1 0 0 10a5 5 0 0 0 0-10zM11.4 12.6L4 20M6.5 17.5l2 2",
  shield: "M12 3l7 3v5.5c0 4.2-2.9 7.6-7 9c-4.1-1.4-7-4.8-7-9V6zM9 12l2.2 2.2L15 10",
  pen: "M4 20h4L19 9l-4-4L4 16zM13.5 6.5l4 4",
  devices: "M3 5h12v8H3zM7 17h4M17 9h4v10h-4z",
  history: "M12 7.5V12l3 2M4.5 12a7.5 7.5 0 1 0 2.2-5.3L4.5 9M4.5 5v4h4",
  update: "M19.5 12a7.5 7.5 0 0 1-13 5.1M4.5 12a7.5 7.5 0 0 1 13-5.1M17.5 3.5v3.6h-3.6M6.5 20.5v-3.6h3.6",
  box: "M5 7.5l7-3.5 7 3.5v9L12 20l-7-3.5zM5 7.5l7 3.5 7-3.5M12 11v9",
  phone: "M8.5 3h7A1.5 1.5 0 0 1 17 4.5v15a1.5 1.5 0 0 1-1.5 1.5h-7A1.5 1.5 0 0 1 7 19.5v-15A1.5 1.5 0 0 1 8.5 3zM11 18h2",
  windows: "M4 5.6l7-1v6.9H4zM13 4.3l7-1v8.2h-7zM4 13.5h7v6.9l-7-1zM13 13.5h7v8.2l-7-1z",
  laptop: "M5 6h14v9H5zM3 18.5h18",
  terminal: "M4 5h16v14H4zM7.5 9.5l3 2.5-3 2.5M12.5 15h4",
  faq: "M12 3.5a8.5 8.5 0 1 0 0 17a8.5 8.5 0 0 0 0-17zM9.6 9.6a2.5 2.5 0 1 1 3.5 2.3c-.7.3-1.1.8-1.1 1.6M12 16.6v.2",
  code: "M8.5 7.5L4 12l4.5 4.5M15.5 7.5L20 12l-4.5 4.5M13.5 5l-3 14",
  doc: "M7 3.5h7l4 4v13H7zM14 3.5v4h4M10 12h5M10 15.5h5",
  image: "M4 5h16v14H4zM4 15.5l4.5-4.5 3.5 3.5 3-3 5 5M9 9.2v.1",
  star: "M12 3.5l2.6 5.4 5.9.8-4.3 4.1 1.05 5.9L12 16.9l-5.25 2.8 1.05-5.9-4.3-4.1 5.9-.8z",
  list: "M5 7h14M5 12h14M5 17h14",
  chevron: "M9.5 6l6 6-6 6",
  wrench: "M14.5 5.5a4 4 0 0 0-5 5L4 16l4 4 5.5-5.5a4 4 0 0 0 5-5l-2.6 2.6-2.4-.6-.6-2.4z",
};
const icon = (name, x, y, scale, stroke, width = 1.9) => `<g transform="translate(${x} ${y}) scale(${scale})"><path d="${I[name]}" fill="none" stroke="${stroke}" stroke-width="${width}" stroke-linecap="round" stroke-linejoin="round"/></g>`;
const logo = (x, y, size) => {
  const k = size / 64;
  return `<g transform="translate(${x} ${y}) scale(${k})"><rect width="64" height="64" rx="15" fill="url(#g)"/><rect x="12" y="16" width="40" height="26" rx="5" fill="none" stroke="#fff" stroke-width="4"/><path d="M24 50h16" stroke="#fff" stroke-width="4" stroke-linecap="round"/><path d="M32 21.5v12M26.5 28.5l5.5 5.5 5.5-5.5" fill="none" stroke="#fff" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"/></g>`;
};

// ---- hero ----
function hero(lang) {
  const t = lang === "ru"
    ? { label: "EasyTizen — простой способ устанавливать приложения на телевизоры Samsung", l1: "Простой способ устанавливать приложения", l2: "на телевизоры Samsung", sub: "С компьютера или прямо с телефона. Без Tizen Studio и командной строки.", steps: ["Найти ТВ", "Подключить", "Выбрать WGT", "Подписать", "Установить"] }
    : { label: "EasyTizen — the easy way to install apps on Samsung Smart TVs", l1: "The easy way to install apps", l2: "on Samsung Smart TVs", sub: "From your computer or straight from your phone. No Tizen Studio, no command line.", steps: ["Find TV", "Connect", "Choose WGT", "Sign", "Install"] };
  const chips = ["Android", "Windows", "macOS", "Linux"];
  let cx = 74;
  const chipSvg = chips.map((c) => { const w = c.length * 10.5 + 34; const s = `<rect x="${cx}" y="352" width="${w}" height="34" rx="17" fill="${C.surface}" stroke="${C.border}"/>${text(cx + w / 2, 375, c, 16, 500, C.muted, "middle")}`; cx += w + 8; return s; }).join("");
  // Right side: a TV receiving an app, with the five-step flow underneath.
  const tv = `<rect x="800" y="64" width="400" height="232" rx="18" fill="#0F1118" stroke="#323848" stroke-width="2"/><rect x="960" y="296" width="80" height="6" rx="3" fill="#323848"/><rect x="996" y="296" width="8" height="18" fill="#323848"/><rect x="930" y="314" width="140" height="6" rx="3" fill="#323848"/>`
    + `<rect x="822" y="86" width="356" height="188" rx="8" fill="url(#screen)"/>`
    + [0, 1, 2, 3].map((i) => `<rect x="${846 + i * 80}" y="196" width="64" height="64" rx="16" fill="${i === 1 ? "url(#g)" : "#232838"}" ${i === 1 ? "" : `stroke="#2E3446"`}/>`).join("")
    + icon("download", 846 + 80 + 17, 196 + 17, 1.25, "#fff", 2.2)
    + `<rect x="846" y="112" width="150" height="14" rx="7" fill="#2A3042"/><rect x="846" y="138" width="230" height="10" rx="5" fill="#222738"/><rect x="846" y="158" width="190" height="10" rx="5" fill="#222738"/>`
    + `<circle cx="1158" cy="106" r="5" fill="${C.a2}"/>`;
  let fx = 74;
  const flow = t.steps.map((s, i) => { const x = fx; fx += s.length * 8.6 + 46; return `<circle cx="${x + 9}" cy="318" r="9" fill="${i === 4 ? "url(#g)" : C.tile}" stroke="${i === 4 ? "none" : C.border}"/>${text(x + 9, 322.5, i + 1, 11, 700, i === 4 ? "#fff" : C.muted, "middle")}${text(x + 24, 323, s, 14, 500, C.muted)}`; }).join("");
  return svg(1280, 420, t.label,
    `<defs>${grad()}<radialGradient id="glow" cx="80%" cy="10%" r="75%"><stop offset="0" stop-color="${C.a1}" stop-opacity=".34"/><stop offset=".5" stop-color="${C.a2}" stop-opacity=".08"/><stop offset="1" stop-color="${C.a2}" stop-opacity="0"/></radialGradient><linearGradient id="base" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${C.bg1}"/><stop offset="1" stop-color="${C.bg0}"/></linearGradient><linearGradient id="screen" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#171B28"/><stop offset="1" stop-color="#11141D"/></linearGradient><clipPath id="clip"><rect width="1280" height="420" rx="28"/></clipPath></defs>`
    + `<g clip-path="url(#clip)"><rect width="1280" height="420" fill="url(#base)"/><rect width="1280" height="420" fill="url(#glow)"/></g><rect x=".5" y=".5" width="1279" height="419" rx="28" fill="none" stroke="${C.border}"/>`
    + logo(74, 56, 76) + text(168, 112, "EasyTizen", 54, 700, C.text, "start", ' letter-spacing="-1.5"')
    + text(74, 192, t.l1, 29, 600, C.text, "start", ' letter-spacing="-.5"') + text(74, 230, t.l2, 29, 600, C.text, "start", ' letter-spacing="-.5"')
    + text(74, 272, t.sub, 17, 400, C.muted) + flow + chipSvg + `<g transform="translate(118 12) scale(.9)">${tv}</g>`);
}
save("hero-ru", hero("ru"));
save("hero-en", hero("en"));

// ---- divider, steps, section and feature icons ----
save("divider", svg(1280, 24, "", `<defs><linearGradient id="l" x1="0" x2="1"><stop offset="0" stop-color="${C.a1}" stop-opacity="0"/><stop offset=".5" stop-color="${C.a2}" stop-opacity=".7"/><stop offset="1" stop-color="${C.a1}" stop-opacity="0"/></linearGradient>${grad()}</defs><rect x="0" y="11" width="1280" height="2" fill="url(#l)"/><rect x="632" y="4" width="16" height="16" rx="5" fill="url(#g)"/>`));
for (let n = 1; n <= 6; n++) save(`step-${n}`, svg(36, 36, `${n}`, `<defs>${grad()}</defs><rect width="36" height="36" rx="11" fill="url(#g)"/>${text(18, 25, n, 19, 700, "#fff", "middle")}`));
const sections = { download: "download", features: "star", look: "image", install: "box", android: "phone", windows: "windows", macos: "laptop", linux: "terminal", tv: "tv", first: "tvplay", faq: "faq", dev: "code", license: "doc" };
for (const [name, ic] of Object.entries(sections)) save(`s-${name}`, svg(44, 44, "", `<defs>${grad()}</defs><rect width="44" height="44" rx="13" fill="url(#g)"/>${icon(ic, 10, 10, 1, "#fff", 2)}`));
const features = { sdk: "box", scan: "search", ip: "globe", duid: "id", cert: "key", import: "upload", check: "shield", sign: "pen", install: "tvplay", devices: "devices", history: "history", update: "update", phone: "phone", diag: "wrench" };
for (const [name, ic] of Object.entries(features)) save(`f-${name}`, svg(64, 64, "", `<defs>${grad()}</defs><rect width="64" height="64" rx="18" fill="${C.surface}"/><rect x=".5" y=".5" width="63" height="63" rx="17.5" fill="none" stroke="${C.border}"/><g transform="translate(16 16) scale(1.3333)"><path d="${I[ic]}" fill="none" stroke="url(#g)" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></g>`));

// ---- download cards (file name + who it is for) ----
const downloads = [
  ["android", "phone", "EasyTizen-Android.apk", "Android 7.0+ · телефоны и планшеты", "Android 7.0+ · phones and tablets"],
  ["windows", "windows", "EasyTizen-Windows-x64-Setup.exe", "Windows 10 и 11 · 64-bit", "Windows 10 and 11 · 64-bit"],
  ["mac-arm", "laptop", "EasyTizen-macOS-ARM64.dmg", "macOS · процессоры Apple (M1 и новее)", "macOS · Apple silicon (M1 and newer)"],
  ["mac-intel", "laptop", "EasyTizen-macOS-Intel.dmg", "macOS · процессоры Intel", "macOS · Intel processors"],
  ["appimage", "terminal", "EasyTizen-Linux-x64.AppImage", "Linux · любой дистрибутив, 64-bit", "Linux · any distribution, 64-bit"],
  ["deb", "terminal", "EasyTizen-Linux-x64.deb", "Debian · Ubuntu · Mint, 64-bit", "Debian · Ubuntu · Mint, 64-bit"],
];
for (const [name, ic, file, ru, en] of downloads) {
  for (const [lang, sub] of [["ru", ru], ["en", en]]) {
    save(`d-${name}-${lang}`, svg(600, 96, `${lang === "ru" ? "Скачать" : "Download"} ${file}`,
      `<defs>${grad()}</defs><rect x="1" y="1" width="598" height="94" rx="20" fill="${C.surface}" stroke="${C.border}" stroke-width="2"/><rect x="18" y="18" width="60" height="60" rx="16" fill="url(#g)"/>${icon(ic, 33, 33, 1.25, "#fff", 2)}`
      + text(98, 44, file, 21, 700, C.text) + text(98, 72, sub, 16.5, 500, C.muted)
      + `<circle cx="556" cy="48" r="18" fill="${C.tile}"/>${icon("download", 545, 37, .92, C.text, 2.1)}`));
  }
}
// ---- navigation cards ----
const cards = [
  ["android", "phone", "Android", "APK · без компьютера · 2 мин", "Android", "APK · no computer needed · 2 min"],
  ["windows", "windows", "Windows", "Установщик .exe · 2 мин", "Windows", ".exe installer · 2 min"],
  ["macos", "laptop", "macOS", "Apple silicon и Intel · 3 мин", "macOS", "Apple silicon and Intel · 3 min"],
  ["linux", "terminal", "Linux", "AppImage или .deb · 2 мин", "Linux", "AppImage or .deb · 2 min"],
  ["tv", "tv", "Телевизор Samsung", "Режим разработчика · 3 мин", "Samsung TV", "Developer Mode · 3 min"],
  ["first", "tvplay", "Первая установка", "ТВ → сертификат → WGT", "First installation", "TV → certificate → WGT"],
  ["all", "list", "Все версии и исходный код", "Страница релизов на GitHub", "All versions and source code", "Releases page on GitHub"],
];
for (const [name, ic, ruTitle, ruSub, enTitle, enSub] of cards) {
  for (const [lang, title, sub] of [["ru", ruTitle, ruSub], ["en", enTitle, enSub]]) {
    save(`p-${name}-${lang}`, svg(600, 112, title,
      `<rect x="1" y="1" width="598" height="110" rx="22" fill="${C.surface}" stroke="${C.border}" stroke-width="2"/><rect x="22" y="22" width="68" height="68" rx="19" fill="${C.tile}"/>${icon(ic, 38, 38, 1.5, C.text, 1.7)}`
      + text(112, 52, title, 25, 600, C.text) + text(112, 82, sub, 17.5, 400, C.muted) + icon("chevron", 548, 42, 1.17, C.faint, 2)));
  }
}
console.log("README artwork written to", OUT);
