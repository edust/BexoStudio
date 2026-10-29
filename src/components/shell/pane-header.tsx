import type { ReactNode } from "react";

import { cn } from "@/lib/cn";

type PaneHeaderProps = {
  title: ReactNode;
  /** Short secondary info shown after the title, e.g. a count or a path. */
  meta?: ReactNode;
  actions?: ReactNode;
  className?: string;
};

/** Uniform 40px header used by every pane: title on the left, actions on the right. */
export function PaneHeader({ actions, className, meta, title }: PaneHeaderProps) {
  return (
    <header
      className={cn(
        "flex h-10 shrink-0 items-center gap-2 border-b border-line px-3",
        className,
      )}
    >
      <div className="flex min-w-0 flex-1 items-baseline gap-2">
        <h2 className="m-0 shrink-0 truncate text-[13px] font-semibold leading-5 text-fg">{title}</h2>
        {meta ? <div className="min-w-0 truncate text-[12px] leading-5 text-fg-3">{meta}</div> : null}
      </div>
      {actions ? <div className="flex shrink-0 items-center gap-1">{actions}</div> : null}
    </header>
  );
}
