import type { CSSProperties } from "react";

import { cn } from "@/lib/cn";

/** Fixed-height session row; both history views virtualise with CODEX_HISTORY_SESSION_ROW_HEIGHT. */
export const CODEX_HISTORY_SESSION_ROW_HEIGHT = 52;

type CodexHistorySessionItemProps = {
  active: boolean;
  dateLabel: string;
  onSelect: () => void;
  secondary: string;
  style: CSSProperties;
  title: string;
  tooltip: string;
};

export function CodexHistorySessionItem({
  active,
  dateLabel,
  onSelect,
  secondary,
  style,
  title,
  tooltip,
}: CodexHistorySessionItemProps) {
  return (
    <button
      aria-current={active ? "true" : undefined}
      className={cn(
        "absolute left-0 right-0 flex w-full flex-col justify-center rounded-md px-2.5 text-left transition-colors",
        active ? "bg-selected" : "hover:bg-hover",
      )}
      onClick={onSelect}
      style={style}
      title={tooltip}
      type="button"
    >
      <span className="flex min-w-0 items-center gap-2">
        <span className={cn("min-w-0 flex-1 truncate text-[13px] leading-5 text-fg", active ? "font-semibold" : "font-medium")}>
          {title}
        </span>
        <span className="shrink-0 text-[12px] text-fg-3">{dateLabel}</span>
      </span>
      <span className="block truncate text-[12px] leading-5 text-fg-3">{secondary}</span>
    </button>
  );
}
