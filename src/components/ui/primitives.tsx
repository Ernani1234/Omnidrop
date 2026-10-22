import { Checkbox as RCheckbox, Collapsible as RCollapsible, Dialog as RDialog, Progress as RProgress, RadioGroup, Switch as RSwitch, Tooltip as RTooltip } from "radix-ui";
import { Check, ChevronDown, X } from "lucide-react";
import type { ButtonHTMLAttributes, ComponentProps, ReactNode } from "react";
import { useId } from "react";

export function cn(...classes: Array<string | false | null | undefined>) {
  return classes.filter(Boolean).join(" ");
}

type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";
type ButtonSize = "sm" | "md" | "lg";

const VARIANTS: Record<ButtonVariant, string> = {
  primary: "bg-accent text-on-accent hover:bg-accent-hover shadow-sm disabled:bg-surface-3 disabled:text-subtle disabled:shadow-none",
  secondary: "bg-surface text-text border border-border-strong hover:bg-surface-2 disabled:text-subtle",
  ghost: "text-muted hover:text-text hover:bg-surface-2 disabled:text-subtle",
  danger: "bg-surface text-danger border border-border-strong hover:bg-danger-soft",
};

const SIZES: Record<ButtonSize, string> = {
  sm: "h-8 px-3 text-[13px] gap-1.5 rounded-lg",
  md: "h-10 px-4 text-sm gap-2 rounded-xl",
  lg: "h-12 px-5 text-[15px] gap-2.5 rounded-xl",
};

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  icon?: ReactNode;
  ref?: React.Ref<HTMLButtonElement>;
}

export function Button({ variant = "secondary", size = "md", icon, className, children, type = "button", ...props }: ButtonProps) {
  return (
    <button
      type={type}
      className={cn(
        "inline-flex shrink-0 items-center justify-center font-semibold whitespace-nowrap transition-colors duration-150 disabled:cursor-not-allowed",
        VARIANTS[variant],
        SIZES[size],
        className,
      )}
      {...props}
    >
      {icon}
      {children}
    </button>
  );
}

export function Tooltip({ label, children, side = "top" }: { label: string; children: ReactNode; side?: "top" | "bottom" | "left" | "right" }) {
  return (
    <RTooltip.Root delayDuration={350}>
      <RTooltip.Trigger asChild>{children}</RTooltip.Trigger>
      <RTooltip.Portal>
        <RTooltip.Content
          side={side}
          sideOffset={6}
          className="z-50 max-w-xs rounded-lg bg-text px-2.5 py-1.5 text-xs font-medium text-bg shadow-card"
        >
          {label}
        </RTooltip.Content>
      </RTooltip.Portal>
    </RTooltip.Root>
  );
}

export function IconButton({ label, children, className, tooltip = true, ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { label: string; tooltip?: boolean }) {
  const button = (
    <button
      type="button"
      aria-label={label}
      className={cn(
        "inline-flex size-9 shrink-0 items-center justify-center rounded-lg text-muted transition-colors hover:bg-surface-2 hover:text-text disabled:opacity-40",
        className,
      )}
      {...props}
    >
      {children}
    </button>
  );
  return tooltip ? <Tooltip label={label}>{button}</Tooltip> : button;
}

export interface Choice<T extends string> {
  value: T;
  label: string;
  description?: string;
  disabled?: boolean;
  badge?: string;
}

/** Radio group rendered as selectable chips. Arrow keys move the choice. */
export function ChoiceChips<T extends string>({
  label,
  value,
  onChange,
  choices,
  columns,
}: {
  label: string;
  value: T;
  onChange: (value: T) => void;
  choices: Choice<T>[];
  columns?: number;
}) {
  return (
    <RadioGroup.Root
      aria-label={label}
      value={value}
      onValueChange={(v) => onChange(v as T)}
      className="grid gap-2"
      style={{ gridTemplateColumns: `repeat(${columns ?? Math.min(choices.length, 3)}, minmax(0, 1fr))` }}
    >
      {choices.map((choice) => (
        <RadioGroup.Item
          key={choice.value}
          value={choice.value}
          disabled={choice.disabled}
          title={choice.disabled ? choice.description : undefined}
          className={cn(
            "relative flex h-11 items-center justify-center rounded-xl border px-2 text-sm font-bold tracking-tight transition-colors",
            "border-border bg-surface text-text hover:border-border-strong",
            "data-[state=checked]:border-accent data-[state=checked]:bg-accent-soft data-[state=checked]:text-accent-ink",
            "disabled:cursor-not-allowed disabled:opacity-45 disabled:hover:border-border",
          )}
        >
          {choice.label}
          {choice.badge && (
            <span className="absolute -top-2 right-1.5 rounded-full bg-surface-3 px-1.5 text-[10px] font-semibold text-muted">
              {choice.badge}
            </span>
          )}
        </RadioGroup.Item>
      ))}
    </RadioGroup.Root>
  );
}

/** Segmented control for short exclusive choices (presets, modes). */
export function Segmented<T extends string>({
  label,
  value,
  onChange,
  choices,
}: {
  label: string;
  value: T;
  onChange: (value: T) => void;
  choices: Choice<T>[];
}) {
  return (
    <RadioGroup.Root
      aria-label={label}
      value={value}
      onValueChange={(v) => onChange(v as T)}
      className="flex rounded-xl bg-surface-2 p-1"
    >
      {choices.map((choice) => (
        <RadioGroup.Item
          key={choice.value}
          value={choice.value}
          disabled={choice.disabled}
          className={cn(
            "flex-1 rounded-lg px-1.5 py-2 text-[13px] leading-tight font-semibold whitespace-nowrap text-muted transition-colors",
            "hover:text-text data-[state=checked]:bg-surface data-[state=checked]:text-text data-[state=checked]:shadow-card",
            "disabled:opacity-40",
          )}
        >
          {choice.label}
        </RadioGroup.Item>
      ))}
    </RadioGroup.Root>
  );
}

export function RadioList<T extends string>({
  label,
  value,
  onChange,
  choices,
}: {
  label: string;
  value: T;
  onChange: (value: T) => void;
  choices: Choice<T>[];
}) {
  return (
    <RadioGroup.Root aria-label={label} value={value} onValueChange={(v) => onChange(v as T)} className="grid gap-1.5">
      {choices.map((choice) => {
        const id = `${label}-${choice.value}`;
        return (
          <div key={choice.value} className="flex items-center gap-2.5">
            <RadioGroup.Item
              id={id}
              value={choice.value}
              disabled={choice.disabled}
              className="flex size-[18px] shrink-0 items-center justify-center rounded-full border-2 border-border-strong data-[state=checked]:border-accent"
            >
              <RadioGroup.Indicator className="size-2 rounded-full bg-accent" />
            </RadioGroup.Item>
            <label htmlFor={id} className="text-sm text-text">
              {choice.label}
            </label>
          </div>
        );
      })}
    </RadioGroup.Root>
  );
}

export function Switch({ label, hint, checked, onChange }: { label: string; hint?: string; checked: boolean; onChange: (checked: boolean) => void }) {
  const id = useId();
  return (
    <div className="flex items-start justify-between gap-3">
      <label htmlFor={id} className="text-sm">
        <span className="font-medium text-text">{label}</span>
        {hint && <span className="block text-xs text-muted">{hint}</span>}
      </label>
      <RSwitch.Root
        id={id}
        checked={checked}
        onCheckedChange={onChange}
        className="relative h-6 w-10 shrink-0 rounded-full bg-surface-3 transition-colors data-[state=checked]:bg-accent"
      >
        <RSwitch.Thumb className="block size-5 translate-x-0.5 rounded-full bg-white shadow transition-transform data-[state=checked]:translate-x-[18px]" />
      </RSwitch.Root>
    </div>
  );
}

export function Checkbox({ label, checked, onChange, className }: { label: string; checked: boolean | "indeterminate"; onChange: (checked: boolean) => void; className?: string }) {
  return (
    <RCheckbox.Root
      aria-label={label}
      checked={checked}
      onCheckedChange={(v) => onChange(v === true)}
      className={cn(
        "flex size-[18px] shrink-0 items-center justify-center rounded-md border-2 border-border-strong bg-surface transition-colors",
        "data-[state=checked]:border-accent data-[state=checked]:bg-accent data-[state=indeterminate]:border-accent data-[state=indeterminate]:bg-accent",
        className,
      )}
    >
      <RCheckbox.Indicator className="text-on-accent">
        {checked === "indeterminate" ? <span className="block h-0.5 w-2 rounded bg-current" /> : <Check className="size-3" strokeWidth={3.5} />}
      </RCheckbox.Indicator>
    </RCheckbox.Root>
  );
}

export function ProgressBar({ value, label, className, indeterminate }: { value: number; label: string; className?: string; indeterminate?: boolean }) {
  const pct = Math.round(Math.min(1, Math.max(0, value)) * 100);
  return (
    <RProgress.Root
      value={indeterminate ? null : pct}
      max={100}
      aria-label={label}
      className={cn("relative h-2 overflow-hidden rounded-full bg-surface-3", className)}
    >
      <RProgress.Indicator
        className={cn("h-full rounded-full bg-accent transition-[width] duration-300 ease-out", indeterminate && "progress-indeterminate w-full")}
        style={indeterminate ? undefined : { width: `${pct}%` }}
      />
    </RProgress.Root>
  );
}

export function Disclosure({ title, children, defaultOpen = false }: { title: string; children: ReactNode; defaultOpen?: boolean }) {
  return (
    <RCollapsible.Root defaultOpen={defaultOpen} className="group/disclosure">
      <RCollapsible.Trigger className="flex w-full items-center justify-between rounded-lg py-1.5 text-sm font-semibold text-muted hover:text-text">
        {title}
        <ChevronDown className="size-4 transition-transform group-data-[state=open]/disclosure:rotate-180" aria-hidden />
      </RCollapsible.Trigger>
      <RCollapsible.Content className="pt-3">{children}</RCollapsible.Content>
    </RCollapsible.Root>
  );
}

export function Dialog({
  open,
  onOpenChange,
  title,
  description,
  children,
  closeLabel,
  wide,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description?: string;
  children: ReactNode;
  closeLabel: string;
  wide?: boolean;
}) {
  return (
    <RDialog.Root open={open} onOpenChange={onOpenChange}>
      <RDialog.Portal>
        <RDialog.Overlay className="fixed inset-0 z-40 bg-overlay" />
        <RDialog.Content
          className={cn(
            "fixed top-1/2 left-1/2 z-50 flex max-h-[85vh] w-[calc(100vw-48px)] -translate-x-1/2 -translate-y-1/2 flex-col rounded-2xl border border-border bg-surface shadow-card",
            wide ? "max-w-2xl" : "max-w-lg",
          )}
        >
          <div className="flex items-start justify-between gap-4 border-b border-border px-6 pt-5 pb-4">
            <div>
              <RDialog.Title className="font-display text-xl font-bold tracking-tight">{title}</RDialog.Title>
              {description ? (
                <RDialog.Description className="mt-0.5 text-sm text-muted">{description}</RDialog.Description>
              ) : (
                <RDialog.Description className="sr-only">{title}</RDialog.Description>
              )}
            </div>
            <RDialog.Close asChild>
              <IconButton label={closeLabel} tooltip={false}>
                <X className="size-5" />
              </IconButton>
            </RDialog.Close>
          </div>
          <div className="min-h-0 overflow-y-auto px-6 py-5">{children}</div>
        </RDialog.Content>
      </RDialog.Portal>
    </RDialog.Root>
  );
}

export function Field({ label, hint, children, htmlFor }: { label: string; hint?: string; children: ReactNode; htmlFor?: string }) {
  return (
    <div className="grid gap-1.5">
      <label htmlFor={htmlFor} className="text-[13px] font-semibold text-text">
        {label}
      </label>
      {children}
      {hint && <p className="text-xs text-muted">{hint}</p>}
    </div>
  );
}

export function TextInput({ className, ...props }: ComponentProps<"input">) {
  return (
    <input
      className={cn(
        "h-10 w-full rounded-xl border border-border-strong bg-surface px-3 text-sm text-text placeholder:text-subtle",
        "focus-visible:border-accent",
        className,
      )}
      {...props}
    />
  );
}

export function NativeSelect({ className, children, ...props }: ComponentProps<"select">) {
  return (
    <div className="relative">
      <select
        className={cn(
          "h-10 w-full appearance-none rounded-xl border border-border-strong bg-surface pr-9 pl-3 text-sm text-text",
          className,
        )}
        {...props}
      >
        {children}
      </select>
      <ChevronDown className="pointer-events-none absolute top-1/2 right-3 size-4 -translate-y-1/2 text-muted" aria-hidden />
    </div>
  );
}

export function SectionLabel({ children, id }: { children: ReactNode; id?: string }) {
  return (
    <h3 id={id} className="mb-2 text-[12px] font-bold tracking-wider text-subtle uppercase">
      {children}
    </h3>
  );
}
