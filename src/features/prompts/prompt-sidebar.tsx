import { PlusOutlined, ReloadOutlined, SearchOutlined } from "@ant-design/icons";
import { Alert, Button, Empty, Input, Spin, Tooltip, Typography } from "antd";
import { motion, Reorder } from "motion/react";
import {
  useCallback,
  useEffect,
  useDeferredValue,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
} from "react";

import { PromptListItem } from "@/features/prompts/prompt-list-item";
import type { PromptQuickPasteBinding } from "@/features/prompts/prompt-quick-paste";
import {
  filterPrompts,
  isSamePromptOrder,
  movePromptBeforeTarget,
  movePromptByOffset,
  orderPromptsByIds,
} from "@/features/prompts/prompt-model";
import { usePointerReorder } from "@/lib/use-pointer-reorder";
import { useShellStore } from "@/stores/shell-store";
import type { PromptRecord } from "@/types/backend";

const PROMPT_ROW_HEIGHT = 99;
const PROMPT_VIRTUALIZATION_THRESHOLD = 80;
const PROMPT_LIST_OVERSCAN = 5;

type PromptSidebarProps = {
  busy: boolean;
  desktopRuntimeAvailable: boolean;
  errorMessage?: string | null;
  loading: boolean;
  onCopy: (prompt: PromptRecord) => void | Promise<void>;
  onCreate: () => void;
  onRefresh: () => void | Promise<void>;
  onReorder: (prompts: PromptRecord[]) => void | Promise<void>;
  onSelect: (prompt: PromptRecord) => void;
  prompts: PromptRecord[];
  quickPasteBindingsByPromptId: ReadonlyMap<string, PromptQuickPasteBinding[]>;
  refreshing: boolean;
  selectedId: string | null;
};

export function PromptSidebar({
  busy,
  desktopRuntimeAvailable,
  errorMessage,
  loading,
  onCopy,
  onCreate,
  onRefresh,
  onReorder,
  onSelect,
  prompts,
  quickPasteBindingsByPromptId,
  refreshing,
  selectedId,
}: PromptSidebarProps) {
  const [query, setQuery] = useState("");
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportHeight, setViewportHeight] = useState(0);
  const [orderedPrompts, setOrderedPrompts] = useState(prompts);
  const [motionDraggingId, setMotionDraggingId] = useState<string | null>(null);
  const deferredQuery = useDeferredValue(query);
  const themeMode = useShellStore((state) => state.themeMode);
  const listRef = useRef<HTMLDivElement | null>(null);
  const latestPromptsRef = useRef(prompts);
  const latestOrderedPromptsRef = useRef(prompts);
  const motionDraggingIdRef = useRef<string | null>(null);
  const motionDragStartOrderRef = useRef<string[] | null>(null);
  const sortingDisabled =
    busy || !desktopRuntimeAvailable || Boolean(query.trim()) || prompts.length <= 1;
  const motionReorderEnabled =
    !sortingDisabled && prompts.length <= PROMPT_VIRTUALIZATION_THRESHOLD;
  const pointerReorderEnabled =
    !sortingDisabled && prompts.length > PROMPT_VIRTUALIZATION_THRESHOLD;
  const reorderTooltip = query.trim()
    ? "搜索过滤时暂不支持排序"
    : !desktopRuntimeAvailable
      ? "请在 Bexo Studio 桌面应用中排序"
      : busy
        ? "当前操作完成后可排序"
        : prompts.length <= 1
          ? "至少需要两条 Prompt 才能排序"
          : "拖拽排序；方向键上移或下移";
  const reorder = usePointerReorder({
    disabled: !pointerReorderEnabled,
    items: prompts,
    reorderItems: movePromptBeforeTarget,
    onReorder: ({ nextItems }) => onReorder(nextItems),
  });
  const displayPrompts = deferredQuery.trim()
    ? prompts
    : prompts.length <= PROMPT_VIRTUALIZATION_THRESHOLD
      ? orderedPrompts
      : reorder.items;
  const filteredPrompts = useMemo(
    () => filterPrompts(displayPrompts, deferredQuery),
    [deferredQuery, displayPrompts],
  );
  const virtualizationEnabled =
    !reorder.draggingId && filteredPrompts.length > PROMPT_VIRTUALIZATION_THRESHOLD;

  useEffect(() => {
    latestPromptsRef.current = prompts;
    if (motionDraggingIdRef.current) {
      return;
    }
    latestOrderedPromptsRef.current = prompts;
    setOrderedPrompts(prompts);
  }, [prompts]);

  const handlePreviewMotionReorder = useCallback((promptIds: string[]) => {
    const nextPrompts = orderPromptsByIds(latestOrderedPromptsRef.current, promptIds);
    if (!nextPrompts) {
      return;
    }
    latestOrderedPromptsRef.current = nextPrompts;
    setOrderedPrompts(nextPrompts);
  }, []);

  const handleMotionDragStart = useCallback((promptId: string) => {
    motionDragStartOrderRef.current = latestOrderedPromptsRef.current.map(
      (prompt) => prompt.id,
    );
    motionDraggingIdRef.current = promptId;
    setMotionDraggingId(promptId);
  }, []);

  const handleMotionDragEnd = useCallback(async () => {
    const previousOrder = motionDragStartOrderRef.current;
    const nextPrompts = latestOrderedPromptsRef.current;
    motionDragStartOrderRef.current = null;
    motionDraggingIdRef.current = null;
    setMotionDraggingId(null);

    if (!previousOrder || isSamePromptOrder(previousOrder, nextPrompts)) {
      return;
    }

    try {
      await onReorder(nextPrompts);
    } catch {
      const fallbackPrompts = latestPromptsRef.current;
      latestOrderedPromptsRef.current = fallbackPrompts;
      setOrderedPrompts(fallbackPrompts);
    }
  }, [onReorder]);
  const virtualRange = useMemo(() => {
    if (!virtualizationEnabled) {
      return { start: 0, end: filteredPrompts.length };
    }
    const start = Math.max(
      0,
      Math.floor(scrollTop / PROMPT_ROW_HEIGHT) - PROMPT_LIST_OVERSCAN,
    );
    const visibleCount = Math.ceil(viewportHeight / PROMPT_ROW_HEIGHT);
    return {
      start,
      end: Math.min(
        filteredPrompts.length,
        start + visibleCount + PROMPT_LIST_OVERSCAN * 2,
      ),
    };
  }, [filteredPrompts.length, scrollTop, viewportHeight, virtualizationEnabled]);

  useEffect(() => {
    const element = listRef.current;
    if (!element) {
      return;
    }
    const updateSize = () => setViewportHeight(element.clientHeight);
    updateSize();
    if (typeof ResizeObserver === "undefined") {
      window.addEventListener("resize", updateSize);
      return () => window.removeEventListener("resize", updateSize);
    }
    const observer = new ResizeObserver(updateSize);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  async function handleKeyboardReorder(
    event: KeyboardEvent<HTMLButtonElement>,
    promptId: string,
  ) {
    if (sortingDisabled || (event.key !== "ArrowUp" && event.key !== "ArrowDown")) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    const next = movePromptByOffset(prompts, promptId, event.key === "ArrowUp" ? -1 : 1);
    if (next) {
      try {
        await onReorder(next);
      } catch (error) {
        console.warn("[prompts] keyboard reorder rejected; query cache kept the previous order", {
          error,
          promptId,
        });
      }
    }
  }

  const renderedPrompts = virtualizationEnabled
    ? filteredPrompts.slice(virtualRange.start, virtualRange.end)
    : filteredPrompts;

  return (
    <aside className="flex h-full min-h-0 flex-col overflow-hidden border-r border-[color:var(--border)] bg-panel">
      <header className="flex h-[70px] shrink-0 items-center justify-between border-b border-[color:var(--border)] px-4">
        <div className="min-w-0">
          <Typography.Text className="block text-[13px] font-semibold text-foreground">
            常用 Prompts
          </Typography.Text>
          <Typography.Text className="block text-[11px] text-muted-foreground">
            {prompts.length.toLocaleString()} 条本地记录
          </Typography.Text>
        </div>
        <Tooltip title="新增 Prompt">
          <Button
            aria-label="新增 Prompt"
            disabled={busy}
            icon={<PlusOutlined />}
            onClick={onCreate}
            shape="circle"
            type="primary"
          />
        </Tooltip>
      </header>

      <div className="shrink-0 border-b border-[color:var(--border)] p-3">
        <Input
          allowClear
          aria-label="搜索 Prompts"
          disabled={loading}
          onChange={(event) => {
            setQuery(event.target.value);
            setScrollTop(0);
            if (listRef.current) {
              listRef.current.scrollTop = 0;
            }
          }}
          placeholder="搜索标题或内容"
          prefix={<SearchOutlined className="text-muted-foreground" />}
          value={query}
        />
        <div className="mt-2 flex items-center justify-between text-[11px] text-muted-foreground">
          <span>{filteredPrompts.length} / {prompts.length}</span>
          <div className="flex items-center gap-1">
            {query.trim() ? <span>清空搜索后可排序</span> : <span>拖拽或方向键排序</span>}
            <Tooltip title="刷新列表">
              <Button
                aria-label="刷新 Prompt 列表"
                disabled={busy || !desktopRuntimeAvailable}
                icon={<ReloadOutlined />}
                loading={refreshing}
                onClick={() => void onRefresh()}
                size="small"
                type="text"
              />
            </Tooltip>
          </div>
        </div>
      </div>

      {errorMessage ? (
        <Alert
          className="m-3 shrink-0"
          description={errorMessage}
          message="读取 Prompts 失败"
          showIcon
          type="error"
        />
      ) : null}
      {!desktopRuntimeAvailable ? (
        <Alert
          className="m-3 shrink-0"
          message="保存功能需要在 Bexo Studio 桌面应用中使用"
          showIcon
          type="info"
        />
      ) : null}

      <motion.div
        className="min-h-0 flex-1 overflow-y-auto px-2 py-2"
        layoutScroll
        onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
        ref={listRef}
        style={{ contain: "strict" }}
      >
        {loading ? (
          <div className="flex h-full items-center justify-center">
            <Spin size="small" />
          </div>
        ) : filteredPrompts.length ? (
          motionReorderEnabled ? (
            <Reorder.Group
              as="div"
              axis="y"
              className="flex flex-col gap-[5px]"
              onReorder={handlePreviewMotionReorder}
              values={orderedPrompts.map((prompt) => prompt.id)}
            >
              {orderedPrompts.map((prompt) => (
                <PromptListItem
                  busy={busy}
                  dragging={motionDraggingId === prompt.id}
                  key={prompt.id}
                  motionReorderEnabled
                  onCopy={onCopy}
                  onKeyboardReorder={handleKeyboardReorder}
                  onMotionDragEnd={handleMotionDragEnd}
                  onMotionDragStart={() => handleMotionDragStart(prompt.id)}
                  onPointerDown={reorder.handlePointerDown}
                  onSelect={onSelect}
                  pointerReorderEnabled={false}
                  prompt={prompt}
                  quickPasteBindings={quickPasteBindingsByPromptId.get(prompt.id) ?? []}
                  reorderTooltip={reorderTooltip}
                  selected={selectedId === prompt.id}
                  sortingDisabled={sortingDisabled}
                  themeMode={themeMode}
                />
              ))}
            </Reorder.Group>
          ) : virtualizationEnabled ? (
            <div
              className="relative"
              style={{ height: filteredPrompts.length * PROMPT_ROW_HEIGHT }}
            >
              {renderedPrompts.map((prompt, visibleIndex) => {
                const index = virtualRange.start + visibleIndex;
                return (
                  <div
                    className="absolute inset-x-0"
                    key={prompt.id}
                    style={{
                      height: PROMPT_ROW_HEIGHT,
                      transform: `translateY(${index * PROMPT_ROW_HEIGHT}px)`,
                    }}
                  >
                    <PromptListItem
                      busy={busy}
                      dragging={false}
                      motionReorderEnabled={false}
                      onCopy={onCopy}
                      onKeyboardReorder={handleKeyboardReorder}
                      onMotionDragEnd={handleMotionDragEnd}
                      onMotionDragStart={() => handleMotionDragStart(prompt.id)}
                      onPointerDown={reorder.handlePointerDown}
                      onSelect={onSelect}
                      pointerReorderEnabled={pointerReorderEnabled}
                      prompt={prompt}
                      quickPasteBindings={quickPasteBindingsByPromptId.get(prompt.id) ?? []}
                      reorderTooltip={reorderTooltip}
                      selected={selectedId === prompt.id}
                      sortingDisabled={sortingDisabled}
                      themeMode={themeMode}
                    />
                  </div>
                );
              })}
            </div>
          ) : (
            <div className="flex flex-col gap-[5px]">
              {renderedPrompts.map((prompt) => (
                <PromptListItem
                  busy={busy}
                  dragging={reorder.draggingId === prompt.id}
                  key={prompt.id}
                  motionReorderEnabled={false}
                  onCopy={onCopy}
                  onKeyboardReorder={handleKeyboardReorder}
                  onMotionDragEnd={handleMotionDragEnd}
                  onMotionDragStart={() => handleMotionDragStart(prompt.id)}
                  onPointerDown={reorder.handlePointerDown}
                  onSelect={onSelect}
                  pointerReorderEnabled={pointerReorderEnabled}
                  prompt={prompt}
                  quickPasteBindings={quickPasteBindingsByPromptId.get(prompt.id) ?? []}
                  reorderTooltip={reorderTooltip}
                  selected={selectedId === prompt.id}
                  sortingDisabled={sortingDisabled}
                  themeMode={themeMode}
                />
              ))}
            </div>
          )
        ) : (
          <div className="flex h-full min-h-[220px] items-center justify-center px-3">
            <Empty
              description={query.trim() ? "没有匹配的 Prompt" : "还没有保存 Prompt"}
              image={Empty.PRESENTED_IMAGE_SIMPLE}
            >
              {!query.trim() ? (
                <Button icon={<PlusOutlined />} onClick={onCreate} type="primary">
                  创建第一个 Prompt
                </Button>
              ) : null}
            </Empty>
          </div>
        )}
      </motion.div>
    </aside>
  );
}
