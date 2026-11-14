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
  return typeof crypto !== "undefined" && "randomUUID" in crypto ? crypto.randomUUID() : `op-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

const IPV4 = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/;
/** Light client-side check for instant feedback only; the Rust backend validates for real. */
export function looksLikeIpv4(s: string): boolean {
  const m = IPV4.exec(s.trim());
  return !!m && m.slice(1).every((o) => Number(o) <= 255);
}

export function formatDate(unixSeconds: number, lang: "en" | "ru", withTime = false): string {
  const options: Intl.DateTimeFormatOptions = withTime
    ? { day: "numeric", month: "short", year: "numeric", hour: "2-digit", minute: "2-digit" }
    : { day: "numeric", month: "short", year: "numeric" };
  return new Intl.DateTimeFormat(lang === "ru" ? "ru-RU" : "en-GB", options).format(new Date(unixSeconds * 1000));
}

/** Same rule the backend enforces for certificate passwords. */
export function passwordProblem(p: string): boolean {
  return p.length < 8 || !/[A-Z]/.test(p) || !/[a-z]/.test(p) || !/\d/.test(p);
}

export function validProfileName(name: string): boolean {
  return /^[A-Za-z0-9_-]{3,64}$/.test(name);
}

/** Days until a certificate expires (negative when expired); null when unknown. */
export function daysLeft(expiresAt: number | null, now = Date.now()): number | null {
  return expiresAt == null ? null : Math.floor((expiresAt * 1000 - now) / 86_400_000);
}
