import { clsx } from "clsx";
import type { ReactNode } from "react";
import { useT } from "../lib/i18n";

type Tone = "ok" | "warn" | "danger" | "info" | "dim" | "brand";

const toneClass: Record<Tone, string> = {
  ok: "text-state-ok border-state-ok/30 bg-state-ok/10",
  warn: "text-state-warn border-state-warn/30 bg-state-warn/10",
  danger: "text-state-danger border-state-danger/30 bg-state-danger/10",
  info: "text-state-info border-state-info/30 bg-state-info/10",
  dim: "text-text-muted border-line bg-surface-2",
  brand: "text-brand-300 border-brand-500/40 bg-brand-500/15",
};

export function Badge({
  children,
  tone = "dim",
  className,
  title,
}: {
  children: ReactNode;
  tone?: Tone;
  className?: string;
  /** 悬停说明：徽标本身很短（「独有」「只读」），语义靠这里补 */
  title?: string;
}) {
  return (
    <span
      title={title}
      className={clsx(
        "inline-flex items-center gap-1 rounded-md border px-1.5 py-[2px] text-[11px] leading-none font-medium",
        toneClass[tone],
        className
      )}
    >
      {children}
    </span>
  );
}

export function Dot({ tone = "ok" }: { tone?: Tone }) {
  const color =
    tone === "ok"
      ? "bg-state-ok"
      : tone === "warn"
        ? "bg-state-warn"
        : tone === "danger"
          ? "bg-state-danger"
          : tone === "info"
            ? "bg-state-info"
            : tone === "brand"
              ? "bg-brand-400"
              : "bg-text-dim";
  return <span className={clsx("inline-block h-1.5 w-1.5 rounded-full", color)} />;
}

export function Button({
  children,
  onClick,
  variant = "ghost",
  size = "md",
  disabled,
  title,
  className,
}: {
  children: ReactNode;
  onClick?: () => void;
  variant?: "primary" | "ghost" | "outline" | "danger";
  size?: "sm" | "md";
  disabled?: boolean;
  title?: string;
  className?: string;
}) {
  const base =
    "inline-flex items-center justify-center gap-1.5 rounded-md font-medium transition-colors duration-150 outline-none focus-visible:ring-2 focus-visible:ring-brand-500/40";
  const sizes = size === "sm" ? "px-2.5 py-1 text-[12px]" : "px-3.5 py-2 text-[13px]";
  const variants = {
    primary:
      "bg-brand-500/90 text-brand-on shadow-glow hover:bg-brand-500 hover:shadow-[0_0_0_1px_rgb(var(--brand-500)/0.55),0_12px_26px_-16px_rgb(var(--brand-500)/0.85)]",
    ghost: "text-text-muted hover:text-text-strong hover:bg-surface-hover",
    outline:
      "border border-line text-text-muted hover:border-line hover:text-text-strong hover:bg-surface-hover",
    danger:
      "border border-state-danger/40 text-state-danger hover:bg-state-danger/10 hover:border-state-danger/60",
  };
  return (
    <button
      type="button"
      title={title}
      onClick={onClick}
      disabled={disabled}
      className={clsx(
        base,
        sizes,
        variants[variant],
        disabled ? "cursor-not-allowed opacity-40" : "cursor-pointer",
        className
      )}
    >
      {children}
    </button>
  );
}

export function Panel({
  children,
  title,
  subtitle,
  actions,
  className,
  bodyClassName,
}: {
  children?: ReactNode;
  title?: ReactNode;
  subtitle?: ReactNode;
  actions?: ReactNode;
  className?: string;
  bodyClassName?: string;
}) {
  return (
    <section className={clsx("panel", className)}>
      {(title || actions) && (
        <header className="flex items-start justify-between gap-4 border-b border-line px-4 py-3">
          <div className="min-w-0">
            {title && (
              <h2 className="truncate text-[15px] font-medium text-text-strong">{title}</h2>
            )}
            {subtitle && (
              <p className="mt-0.5 truncate text-[12px] text-text-dim">{subtitle}</p>
            )}
          </div>
          {actions && <div className="flex shrink-0 items-center gap-1.5">{actions}</div>}
        </header>
      )}
      <div className={clsx("px-4 py-3", bodyClassName)}>{children}</div>
    </section>
  );
}

export function SearchInput({
  value,
  onChange,
  placeholder,
}: {
  value: string;
  onChange: (next: string) => void;
  placeholder: string;
}) {
  const t = useT();
  return (
    <div className="group relative flex items-center">
      <svg
        viewBox="0 0 24 24"
        fill="none"
        className="pointer-events-none absolute left-2.5 h-3.5 w-3.5 text-text-dim transition-colors group-focus-within:text-brand-300"
      >
        <circle cx="11" cy="11" r="7" stroke="currentColor" strokeWidth="1.6" />
        <path d="m16.5 16.5 3 3" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
      </svg>
      <input
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder}
        className="h-8 w-full rounded-md border border-line bg-ink-950/60 pl-8 pr-7 text-[12.5px] text-text-strong placeholder:text-text-dim outline-none transition-all focus:border-brand-500/50 focus:bg-ink-950 focus:ring-2 focus:ring-brand-500/15"
      />
      {value && (
        <button
          type="button"
          onClick={() => onChange("")}
          className="absolute right-2 cursor-pointer text-text-dim transition-colors hover:text-text-strong"
          aria-label={t("common.clear")}
        >
          <svg viewBox="0 0 24 24" fill="none" className="h-3.5 w-3.5">
            <path d="M6 6l12 12M18 6 6 18" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
          </svg>
        </button>
      )}
    </div>
  );
}

export function Toggle({
  checked,
  onChange,
  disabled,
}: {
  checked: boolean;
  onChange: (next: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={clsx(
        "relative h-4 w-7 rounded-full border transition-colors duration-200",
        checked
          ? "border-brand-500/60 bg-brand-500/70"
          : "border-line bg-surface-active",
        disabled ? "cursor-not-allowed opacity-40" : "cursor-pointer"
      )}
    >
      <span
        className={clsx(
          "absolute top-[1px] h-3 w-3 rounded-full bg-white transition-all duration-200",
          checked ? "left-[13px]" : "left-[2px]"
        )}
      />
    </button>
  );
}

export function Empty({
  title,
  hint,
  action,
}: {
  title: string;
  hint: string;
  action?: ReactNode;
}) {
  return (
    <div className="flex flex-col items-center justify-center gap-2 px-6 py-14 text-center">
      <div className="flex h-10 w-10 items-center justify-center rounded-xl border border-line bg-surface-2 text-text-dim">
        <svg viewBox="0 0 24 24" fill="none" className="h-5 w-5">
          <path
            d="M4 7.5A1.5 1.5 0 0 1 5.5 6h4l1.5 2h7.5A1.5 1.5 0 0 1 20 9.5v7A1.5 1.5 0 0 1 18.5 18h-13A1.5 1.5 0 0 1 4 16.5v-9Z"
            stroke="currentColor"
            strokeWidth="1.4"
          />
        </svg>
      </div>
      <p className="text-[13px] text-text-strong">{title}</p>
      <p className="max-w-sm text-[12px] leading-relaxed text-text-dim">{hint}</p>
      {action}
    </div>
  );
}
