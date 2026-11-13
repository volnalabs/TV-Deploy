import { Loader2 } from "lucide-react";
import type { ButtonHTMLAttributes, ReactNode } from "react";

type Variant = "primary" | "secondary" | "ghost";

export function Button({ variant = "secondary", busy, className = "", children, disabled, ...rest }: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant; busy?: boolean }) {
  const base = "inline-flex items-center justify-center gap-2 h-9 px-3.5 rounded-lg text-[13px] font-medium transition-colors duration-150 disabled:cursor-not-allowed disabled:opacity-50 cursor-pointer select-none";
  const styles: Record<Variant, string> = {
    primary: "bg-[var(--accent)] text-[var(--accent-ink)] hover:brightness-110",
    secondary: "bg-[var(--surface)] text-[var(--text)] border border-[var(--border)] hover:bg-[var(--surface-2)]",
    ghost: "text-[var(--muted)] hover:text-[var(--text)] hover:bg-[var(--surface-2)]",
  };
  return (
    <button className={`${base} ${styles[variant]} ${className}`} disabled={disabled || busy} {...rest}>
      {busy && <Loader2 size={15} className="spin" aria-hidden />}
      {children}
    </button>
  );
}

export function IconButton({ label, children, ...rest }: ButtonHTMLAttributes<HTMLButtonElement> & { label: string }) {
  return (
    <button
      aria-label={label}
      title={label}
      className="grid h-9 w-9 place-items-center rounded-lg text-[var(--muted)] transition-colors hover:bg-[var(--surface-2)] hover:text-[var(--text)] cursor-pointer"
      {...rest}
    >
      {children}
    </button>
  );
}

export function Segmented<T extends string>({ value, options, onChange, label }: { value: T; options: { value: T; label: string }[]; onChange: (v: T) => void; label: string }) {
  return (
    <div role="radiogroup" aria-label={label} className="inline-flex rounded-lg bg-[var(--surface-2)] p-0.5">
      {options.map((o) => (
        <button
          key={o.value}
          role="radio"
          aria-checked={o.value === value}
          onClick={() => onChange(o.value)}
          className={`h-8 cursor-pointer rounded-md px-3 text-[13px] transition-colors ${o.value === value ? "bg-[var(--surface)] font-medium text-[var(--text)] shadow-sm" : "text-[var(--muted)] hover:text-[var(--text)]"}`}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

export function Pill({ tone, children }: { tone: "ok" | "warn" | "err" | "neutral"; children: ReactNode }) {
  const map = {
    ok: "bg-[var(--ok-soft)] text-[var(--ok)]",
    warn: "bg-[var(--warn-soft)] text-[var(--warn)]",
    err: "bg-[var(--err-soft)] text-[var(--err)]",
    neutral: "bg-[var(--surface-2)] text-[var(--muted)]",
  } as const;
  return <span className={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-medium ${map[tone]}`}>{children}</span>;
}

/** One numbered step of the main flow: a thin-ruled section, not a heavy card. */
export function Step({ n, title, aside, children, done }: { n: number; title: string; aside?: ReactNode; children: ReactNode; done?: boolean }) {
  return (
    <section className="fade-up border-t border-[var(--border)] py-7 first:border-t-0 first:pt-0">
      <header className="mb-4 flex items-center gap-3">
        <span className={`grid h-6 w-6 place-items-center rounded-full text-xs font-semibold ${done ? "bg-[var(--accent)] text-[var(--accent-ink)]" : "bg-[var(--surface-2)] text-[var(--muted)]"}`}>{n}</span>
        <h2 className="text-[15px] font-semibold tracking-tight">{title}</h2>
        <div className="ml-auto">{aside}</div>
      </header>
      {children}
    </section>
  );
}

export function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex items-baseline justify-between gap-6 border-b border-[var(--border)] py-2 last:border-b-0">
      <dt className="text-[var(--muted)]">{label}</dt>
      <dd className="selectable m-0 truncate text-right font-medium">{children}</dd>
    </div>
  );
}
