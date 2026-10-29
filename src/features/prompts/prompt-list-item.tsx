import { CopyOutlined, HolderOutlined } from "@ant-design/icons";
import { Button, Tooltip } from "antd";
import { motion, Reorder, useDragControls } from "motion/react";
import type {
  KeyboardEvent,
  PointerEvent as ReactPointerEvent,
} from "react";

import { buildPromptPreview } from "@/features/prompts/prompt-model";
import type { PromptQuickPasteBinding } from "@/features/prompts/prompt-quick-paste";
import { cn } from "@/lib/cn";
import { reorderLayoutTransition } from "@/lib/reorder-motion";
import type { PromptRecord } from "@/types/backend";

type PromptListItemProps = {
  busy: boolean;
  dragging: boolean;
  motionReorderEnabled: boolean;
  onCopy: (prompt: PromptRecord) => void | Promise<void>;
  onMotionDragEnd: () => void | Promise<void>;
  onMotionDragStart: () => void;
  onKeyboardReorder: (
    event: KeyboardEvent<HTMLButtonElement>,
    promptId: string,
  ) => void | Promise<void>;
  onPointerDown: (event: ReactPointerEvent<HTMLElement>, itemId: string) => void;
  onSelect: (prompt: PromptRecord) => void;
  pointerReorderEnabled: boolean;
  prompt: PromptRecord;
  quickPasteBindings: PromptQuickPasteBinding[];
  reorderTooltip: string;
  selected: boolean;
  sortingDisabled: boolean;
};

export function PromptListItem({
  busy,
  dragging,
  motionReorderEnabled,
  onCopy,
  onMotionDragEnd,
  onMotionDragStart,
  onKeyboardReorder,
  onPointerDown,
  onSelect,
  pointerReorderEnabled,
  prompt,
  quickPasteBindings,
  reorderTooltip,
  selected,
  sortingDisabled,
}: PromptListItemProps) {
  const dragControls = useDragControls();
  // Fixed 56px row: the virtual list relies on this height (PROMPT_ROW_HEIGHT).
  const itemClassName = cn(
    "group relative flex h-[56px] items-center gap-1.5 overflow-hidden rounded-md pl-0.5 pr-1 transition-colors",
    selected || dragging ? "bg-selected" : "hover:bg-hover",
    dragging && "opacity-95",
  );
  const content = (
    <>
      <Tooltip mouseEnterDelay={0.4} title={reorderTooltip}>
        <button
          aria-label={`调整 ${prompt.title} 的顺序`}
          className={cn(
            "reveal-on-hover flex h-8 w-3.5 shrink-0 touch-none items-center justify-center text-[11px] text-fg-3",
            sortingDisabled ? "cursor-not-allowed" : "cursor-grab active:cursor-grabbing",
          )}
          disabled={sortingDisabled}
          onClick={(event) => event.stopPropagation()}
          onKeyDown={(event) => void onKeyboardReorder(event, prompt.id)}
          onPointerDown={(event) => {
            event.stopPropagation();
            if (sortingDisabled) {
              return;
            }
            if (motionReorderEnabled) {
              dragControls.start(event);
              return;
            }
            onPointerDown(event, prompt.id);
          }}
          type="button"
        >
          <HolderOutlined />
        </button>
      </Tooltip>

      <button
        aria-label={`打开 ${prompt.title}`}
        className="h-full min-w-0 flex-1 cursor-pointer overflow-hidden py-2 text-left disabled:cursor-not-allowed disabled:opacity-60"
        disabled={busy}
        onClick={() => onSelect(prompt)}
        type="button"
      >
        <span className="flex min-w-0 items-center gap-1">
          <span className={cn("min-w-0 flex-1 truncate text-[13px] leading-5 text-fg", selected ? "font-semibold" : "font-medium")}>
            {prompt.title}
          </span>
          {quickPasteBindings.slice(0, 2).map((binding) => (
            <Tooltip key={binding.slot} title={`快速粘贴槽位 ${binding.slot} · ${binding.shortcut}`}>
              <span
                aria-hidden="true"
                className="shrink-0 rounded bg-primary-soft px-1 text-[11px] leading-[18px] text-primary"
              >
                快捷{binding.slot}
              </span>
              <span className="sr-only">
                快速粘贴槽位 {binding.slot}，{binding.shortcut}
              </span>
            </Tooltip>
          ))}
          {quickPasteBindings.length > 2 ? (
            <Tooltip
              title={quickPasteBindings
                .slice(2)
                .map((binding) => `槽位 ${binding.slot} · ${binding.shortcut}`)
                .join("\n")}
            >
              <span className="shrink-0 text-[11px] text-fg-3">+{quickPasteBindings.length - 2}</span>
            </Tooltip>
          ) : null}
        </span>
        <span className="block truncate text-[12px] leading-5 text-fg-3">
          {buildPromptPreview(prompt.content) || "空内容"}
        </span>
      </button>

      <Tooltip title="复制 Prompt">
        <Button
          aria-label={`复制 ${prompt.title}`}
          className="reveal-on-hover shrink-0"
          disabled={busy}
          icon={<CopyOutlined />}
          onClick={(event) => {
            event.stopPropagation();
            void onCopy(prompt);
          }}
          size="small"
          type="text"
        />
      </Tooltip>
    </>
  );

  if (!motionReorderEnabled) {
    return (
      <motion.div
        animate={{ scale: dragging && pointerReorderEnabled ? 1.01 : 1 }}
        className={itemClassName}
        data-reorder-item-id={prompt.id}
        layout={pointerReorderEnabled ? "position" : false}
        transition={reorderLayoutTransition}
      >
        {content}
      </motion.div>
    );
  }

  return (
    <Reorder.Item
      as="div"
      className={itemClassName}
      data-reorder-item-id={prompt.id}
      dragControls={dragControls}
      dragListener={false}
      layout="position"
      onDragEnd={() => void onMotionDragEnd()}
      onDragStart={onMotionDragStart}
      transition={reorderLayoutTransition}
      value={prompt.id}
      whileDrag={{ zIndex: 3, boxShadow: "0 8px 24px -12px rgba(15,23,42,0.35)" }}
    >
      {content}
    </Reorder.Item>
  );
}
