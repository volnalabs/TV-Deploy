import { AlertTriangle, CheckCircle2, ChevronDown, Info, Loader2, X } from "lucide-react";
import { createContext, useCallback, useContext, useEffect, useRef, useState, type ButtonHTMLAttributes, type ReactNode } from "react";

type Variant = "primary" | "secondary" | "ghost" | "danger";

export function Button({ variant = "secondary", busy, size = "md", className = "", children, disabled, type = "button", ...rest }: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant; busy?: boolean; size?: "sm" | "md" | "lg" }) {
  const sizes = { sm: "h-8 px-3 text-[13px] rounded-lg", md: "h-10 px-4 text-[13.5px] rounded-[10px]", lg: "h-12 px-5 text-[15px] rounded-xl" };
  const styles: Record<Variant, string> = {
    primary: "bg-[var(--accent)] text-[var(--accent-ink)] hover:brightness-110 active:brightness-95 shadow-[0_1px_0_rgb(255_255_255/0.15)_inset,0_6px_16px_-6px_var(--accent)]",
    secondary: "bg-[var(--surface)] text-[var(--text)] border border-[var(--border-strong)] hover:bg-[var(--surface-2)]",
    ghost: "text-[var(--muted)] hover:text-[var(--text)] hover:bg-[var(--surface-2)]",
    danger: "bg-[var(--err-soft)] text-[var(--err)] hover:brightness-95",
  };
  return (
    <button type={type} className={`inline-flex shrink-0 items-center justify-center gap-2 font-medium transition-[filter,background-color,transform] duration-150 select-none active:scale-[0.98] disabled:opacity-50 disabled:active:scale-100 ${sizes[size]} ${styles[variant]} ${className}`} disabled={disabled || busy} {...rest}>
      {busy && <Loader2 size={16} className="spin" aria-hidden />}
      {children}
    </button>
  );
}

export function IconButton({ label, children, className = "", ...rest }: ButtonHTMLAttributes<HTMLButtonElement> & { label: string }) {
  return (
    <button type="button" aria-label={label} title={label} className={`grid h-9 w-9 shrink-0 place-items-center rounded-lg text-[var(--muted)] transition-colors hover:bg-[var(--surface-2)] hover:text-[var(--text)] ${className}`} {...rest}>
      {children}
    </button>
  );
}

export function Segmented<T extends string>({ value, options, onChange, label }: { value: T; options: { value: T; label: string }[]; onChange: (v: T) => void; label: string }) {
  return (
    <div role="radiogroup" aria-label={label} className="inline-flex max-w-full rounded-[10px] bg-[var(--surface-2)] p-1">
      {options.map((o) => (
        <button key={o.value} type="button" role="radio" aria-checked={o.value === value} onClick={() => onChange(o.value)}
          className={`h-8 min-w-0 truncate rounded-lg px-3 text-[13px] transition-all ${o.value === value ? "bg-[var(--surface)] font-medium text-[var(--text)] shadow-sm" : "text-[var(--muted)] hover:text-[var(--text)]"}`}>
          {o.label}
        </button>
      ))}
    </div>
  );
}

export function Toggle({ checked, onChange, label }: { checked: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <label className="flex cursor-pointer items-center gap-3">
      <button type="button" role="switch" aria-checked={checked} aria-label={label} onClick={() => onChange(!checked)}
        className={`relative h-6 w-10 shrink-0 rounded-full transition-colors ${checked ? "bg-[var(--accent)]" : "bg-[var(--surface-3)]"}`}>
        <span className={`absolute top-0.5 left-0.5 h-5 w-5 rounded-full bg-white shadow transition-transform ${checked ? "translate-x-4" : ""}`} />
      </button>
      <span className="min-w-0">{label}</span>
    </label>
  );
}

export function Pill({ tone, children }: { tone: "ok" | "warn" | "err" | "neutral" | "accent"; children: ReactNode }) {
  const map = {
    ok: "bg-[var(--ok-soft)] text-[var(--ok)]",
    warn: "bg-[var(--warn-soft)] text-[var(--warn)]",
    err: "bg-[var(--err-soft)] text-[var(--err)]",
    neutral: "bg-[var(--surface-2)] text-[var(--muted)]",
    accent: "bg-[var(--accent-soft)] text-[var(--accent)]",
  } as const;
  return <span className={`inline-flex shrink-0 items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-medium whitespace-nowrap ${map[tone]}`}>{children}</span>;
}

export function IconTile({ children, tone = "accent" }: { children: ReactNode; tone?: "accent" | "ok" | "err" | "neutral" }) {
  const map = { accent: "bg-[var(--accent-soft)] text-[var(--accent)]", ok: "bg-[var(--ok-soft)] text-[var(--ok)]", err: "bg-[var(--err-soft)] text-[var(--err)]", neutral: "bg-[var(--surface-2)] text-[var(--muted)]" };
  return <span className={`grid h-10 w-10 shrink-0 place-items-center rounded-xl ${map[tone]}`}>{children}</span>;
}

export function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex items-baseline justify-between gap-6 border-b border-[var(--border)] py-2 last:border-b-0">
      <dt className="shrink-0 text-[var(--muted)]">{label}</dt>
      <dd className="selectable m-0 min-w-0 truncate text-right font-medium">{children}</dd>
    </div>
  );
}

export function Field({ label, hint, error, children }: { label: string; hint?: string; error?: string | null; children: ReactNode }) {
  return (
    <label className="block">
      <span className="mb-1.5 block text-[13px] font-medium">{label}</span>
      {children}
      {error ? <span role="alert" className="mt-1.5 block text-xs text-[var(--err)]">{error}</span> : hint ? <span className="mt-1.5 block text-xs text-[var(--muted)]">{hint}</span> : null}
    </label>
  );
}

/** Determinate when `value` is a number (0–1), animated stripe otherwise. */
export function Progress({ value, label }: { value: number | null; label?: string }) {
  return (
    <div role="progressbar" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={value == null ? undefined : Math.round(value * 100)} className="h-1.5 w-full overflow-hidden rounded-full bg-[var(--surface-3)]">
      {value == null
        ? <div className="indeterminate brand-gradient h-full w-2/5 rounded-full" />
        : <div className="brand-gradient h-full rounded-full transition-[width] duration-300" style={{ width: `${Math.max(2, Math.min(100, value * 100))}%` }} />}
    </div>
  );
}

export function Notice({ tone, title, children, action }: { tone: "info" | "warn" | "err" | "ok"; title?: string; children?: ReactNode; action?: ReactNode }) {
  const map = { info: ["var(--accent)", "var(--accent-soft)"], warn: ["var(--warn)", "var(--warn-soft)"], err: ["var(--err)", "var(--err-soft)"], ok: ["var(--ok)", "var(--ok-soft)"] } as const;
  const Icon = tone === "ok" ? CheckCircle2 : tone === "info" ? Info : AlertTriangle;
  return (
    <div role={tone === "err" ? "alert" : "status"} className="fade-up flex gap-3 rounded-xl p-3.5" style={{ background: map[tone][1] }}>
      <Icon size={18} className="mt-0.5 shrink-0" style={{ color: map[tone][0] }} aria-hidden />
      <div className="min-w-0 flex-1">
        {title && <p className="font-semibold">{title}</p>}
        {children && <div className={`text-[13.5px] ${title ? "mt-0.5 text-[var(--muted)]" : ""}`}>{children}</div>}
        {action && <div className="mt-2.5 flex flex-wrap gap-2">{action}</div>}
      </div>
    </div>
  );
}

export function Details({ label, children }: { label: string; children: ReactNode }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="mt-2">
      <button type="button" onClick={() => setOpen(!open)} aria-expanded={open} className="flex items-center gap-1 text-xs text-[var(--muted)] hover:text-[var(--text)]">
        <ChevronDown size={14} className={`transition-transform ${open ? "rotate-180" : ""}`} aria-hidden />
        {label}
      </button>
      {open && <pre className="selectable fade-in mt-2 max-h-44 overflow-auto rounded-lg bg-[var(--surface-2)] p-3 text-xs whitespace-pre-wrap text-[var(--text)]">{children}</pre>}
    </div>
  );
}

export function EmptyState({ icon, title, body, action }: { icon: ReactNode; title: string; body: string; action?: ReactNode }) {
  return (
    <div className="fade-up grid place-items-center rounded-[var(--radius)] border border-dashed border-[var(--border-strong)] px-6 py-12 text-center">
      <span className="relative grid h-14 w-14 place-items-center rounded-2xl bg-[var(--accent-soft)] text-[var(--accent)]">{icon}</span>
      <p className="mt-4 text-[15px] font-semibold">{title}</p>
      <p className="mt-1 max-w-sm text-[var(--muted)]">{body}</p>
      {action && <div className="mt-5 flex flex-wrap justify-center gap-2">{action}</div>}
    </div>
  );
}

export function PageHeader({ title, subtitle, action }: { title: string; subtitle?: string; action?: ReactNode }) {
  return (
    <header className="mb-6 flex flex-wrap items-start gap-x-4 gap-y-3">
      <div className="min-w-0 flex-1 basis-64">
        <h1 className="text-[22px] leading-tight font-semibold tracking-tight">{title}</h1>
        {subtitle && <p className="mt-1 text-[var(--muted)]">{subtitle}</p>}
      </div>
      {action && <div className="flex shrink-0 gap-2">{action}</div>}
    </header>
  );
}

export function Modal({ title, onClose, children, footer, wide }: { title: string; onClose: () => void; children: ReactNode; footer?: ReactNode; wide?: boolean }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    ref.current?.focus();
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);
  return (
    <div className="fade-in fixed inset-0 z-40 grid place-items-end bg-black/45 sm:place-items-center sm:p-6" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div ref={ref} tabIndex={-1} role="dialog" aria-modal="true" aria-label={title}
        className={`pop-in safe-bottom flex max-h-[92dvh] w-full flex-col rounded-t-2xl border border-[var(--border)] bg-[var(--surface)] shadow-[var(--shadow-pop)] outline-none sm:rounded-2xl ${wide ? "sm:max-w-xl" : "sm:max-w-md"}`}>
        <div className="flex items-center gap-3 px-5 pt-4 pb-2">
          <h2 className="min-w-0 flex-1 truncate text-[17px] font-semibold tracking-tight">{title}</h2>
          <IconButton label="✕" onClick={onClose}><X size={18} aria-hidden /></IconButton>
        </div>
        <div className="min-h-0 flex-1 overflow-auto px-5 pb-4">{children}</div>
        {footer && <div className="flex flex-wrap justify-end gap-2 border-t border-[var(--border)] px-5 py-3.5">{footer}</div>}
      </div>
    </div>
  );
}

// ---------- toasts ----------

interface Toast { id: number; tone: "ok" | "err" | "info"; text: string }
const ToastContext = createContext<(tone: Toast["tone"], text: string) => void>(() => undefined);
export const useToast = () => useContext(ToastContext);

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const push = useCallback((tone: Toast["tone"], text: string) => {
    const id = Date.now() + Math.random();
    setToasts((t) => [...t.slice(-2), { id, tone, text }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), tone === "err" ? 6000 : 3500);
  }, []);
  return (
    <ToastContext.Provider value={push}>
      {children}
      <div aria-live="polite" className="pointer-events-none fixed inset-x-0 bottom-20 z-50 flex flex-col items-center gap-2 px-4 lg:bottom-6">
        {toasts.map((t) => (
          <div key={t.id} role="status" className="pop-in pointer-events-auto flex max-w-md items-start gap-2.5 rounded-xl border border-[var(--border)] bg-[var(--surface)] px-4 py-3 shadow-[var(--shadow-pop)]">
            {t.tone === "ok" ? <CheckCircle2 size={18} className="mt-0.5 shrink-0 text-[var(--ok)]" aria-hidden /> : t.tone === "err" ? <AlertTriangle size={18} className="mt-0.5 shrink-0 text-[var(--err)]" aria-hidden /> : <Info size={18} className="mt-0.5 shrink-0 text-[var(--accent)]" aria-hidden />}
            <span className="min-w-0">{t.text}</span>
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
}

/** Copies text; falls back for webviews without the async clipboard API. */
export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    const area = document.createElement("textarea");
    area.value = text;
    area.style.position = "fixed";
    area.style.opacity = "0";
    document.body.appendChild(area);
    area.select();
    const ok = document.execCommand("copy");
    area.remove();
    return ok;
  }
}
