import type { ReactNode } from "react";

import { cn } from "@/lib/cn";

/** A titled block of settings rows rendered as one bordered list. */
export function SettingsGroup({ children, title }: { children: ReactNode; title: string }) {
  return (
    <section aria-label={title} className="space-y-1.5">
      <h3 className="m-0 px-1 text-[12px] font-medium text-fg-3">{title}</h3>
      <div className="divide-y divide-line overflow-hidden rounded-md border border-line bg-surface">{children}</div>
    </section>
  );
}

type SettingsRowProps = {
  label: ReactNode;
  /** One short line; longer explanations belong in a tooltip. */
  description?: ReactNode;
  children?: ReactNode;
  /** Stretch the control column (paths, text inputs). */
  wide?: boolean;
  className?: string;
};

export function SettingsRow({ children, className, description, label, wide = false }: SettingsRowProps) {
  return (
    <div
      className={cn(
        "grid min-h-12 items-center gap-x-4 gap-y-1 px-3 py-2",
        wide ? "grid-cols-[200px_minmax(0,1fr)]" : "grid-cols-[minmax(0,1fr)_auto]",
        className,
      )}
    >
      <div className="min-w-0">
        <div className="text-[13px] text-fg">{label}</div>
        {description ? <div className="mt-0.5 text-[12px] leading-5 text-fg-3">{description}</div> : null}
      </div>
      {children ? <div className={cn("flex min-w-0 items-center gap-2", wide ? "" : "justify-end")}>{children}</div> : null}
    </div>
  );
}

/** Read-only path display that keeps the full value in the tooltip. */
export function PathValue({ value }: { value: string }) {
  return (
    <div
      className="flex h-[30px] min-w-0 flex-1 items-center overflow-hidden rounded-md border border-line bg-surface-2 px-2.5 font-mono text-[12px] text-fg-2"
      title={value}
    >
      <span className="truncate">{value}</span>
    </div>
  );
}
