export function formatBytes(n: number, lang: "en" | "ru" = "en"): string {
  const units = lang === "ru" ? ["Б", "КБ", "МБ", "ГБ"] : ["B", "KB", "MB", "GB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${i === 0 ? v : v.toFixed(v < 10 ? 1 : 0)} ${units[i]}`;
}

export function opId(): string {
  return crypto.randomUUID();
}

const IPV4 = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/;
/** Light client-side check for instant feedback only; the Rust backend validates for real. */
export function looksLikeIpv4(s: string): boolean {
  const m = IPV4.exec(s.trim());
  return !!m && m.slice(1).every((o) => Number(o) <= 255);
}
