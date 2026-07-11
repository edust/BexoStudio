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
import type { ThemeMode } from "@/stores/shell-store";
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
  themeMode: ThemeMode;
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
  themeMode,
}: PromptListItemProps) {
  const dragControls = useDragControls();
  const itemClassName = cn(
    "group relative flex h-[94px] items-start gap-2 overflow-hidden rounded-[12px] border px-3 py-2.5 transition-[border-color,background-color,opacity] duration-150",
    selected
      ? "border-[#8fd4ec] bg-[#f4fbfe]"
      : themeMode === "dark"
        ? "border-transparent hover:border-[#3c3c3c] hover:bg-[#2a2d2e]"
        : "border-transparent hover:border-[#d9e2ec] hover:bg-[#f8fafc]",
    dragging && "border-[#1697c5] bg-[#f0fbff] opacity-95",
  );
  const content = (
    <>
      <Tooltip
        title={reorderTooltip}
      >
        <button
          aria-label={`调整 ${prompt.title} 的顺序`}
          className={cn(
            "mt-0.5 flex h-6 w-6 shrink-0 touch-none items-center justify-center rounded-[6px] border border-[#d8e1eb] bg-white text-[#667085]",
            sortingDisabled
              ? "cursor-not-allowed opacity-45"
              : "cursor-grab active:cursor-grabbing",
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
        className="h-full min-w-0 flex-1 cursor-pointer overflow-hidden text-left disabled:cursor-not-allowed disabled:opacity-60"
        disabled={busy}
        onClick={() => onSelect(prompt)}
        type="button"
      >
        <span className="flex min-w-0 items-center gap-1">
          <span className="min-w-0 flex-1 truncate text-[13px] font-medium leading-5 text-[#1f2937]">
            {prompt.title}
          </span>
          {quickPasteBindings.slice(0, 2).map((binding) => (
            <Tooltip
              key={binding.slot}
              title={`快速粘贴槽位 ${binding.slot} · ${binding.shortcut}`}
            >
              <span
                aria-hidden="true"
                className="shrink-0 rounded-full border border-[#8fd4ec] bg-[#eef6fb] px-1.5 text-[9px] font-semibold leading-[17px] text-[#12718f]"
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
              <span className="shrink-0 text-[9px] text-[#667085]">
                +{quickPasteBindings.length - 2}
              </span>
            </Tooltip>
          ) : null}
        </span>
        <span className="mt-1 line-clamp-2 max-h-[40px] break-words text-[12px] leading-5 text-[#7b8794]">
          {buildPromptPreview(prompt.content) || "空内容"}
        </span>
      </button>

      <Tooltip title="复制 Prompt">
        <Button
          aria-label={`复制 ${prompt.title}`}
          className="!h-7 !w-7 !min-w-7 shrink-0 !rounded-[7px] !p-0 text-[#667085] opacity-70 transition-opacity duration-150 group-hover:opacity-100"
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
      whileDrag={{
        scale: 1.015,
        zIndex: 3,
        boxShadow: "0 22px 44px -28px rgba(22,151,197,0.45)",
      }}
    >
      {content}
    </Reorder.Item>
  );
}
