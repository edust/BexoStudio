import { HolderOutlined } from "@ant-design/icons";
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { WORKSPACE_ROW_HEIGHT, visibleWorkspaceRows, type NavigableWorkspace } from "./workspace-navigation-model";

const GROUP_ROW_HEIGHT = 28;

type Props<T> = {
  items: T[];
  pinnedIds: string[];
  searching: boolean;
  resetKey: string;
  focusId: string | null;
  reorderEnabled: boolean;
  onMove: (source: string, target: string) => void;
  renderItem: (item: T) => ReactNode;
};

export function WorkspaceNavigationList<T extends NavigableWorkspace>({
  items, pinnedIds, searching, resetKey, focusId, reorderEnabled, onMove, renderItem,
}: Props<T>) {
  const viewport = useRef<HTMLDivElement>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [height, setHeight] = useState(500);
  const [collapsed, setCollapsed] = useState(false);
  const [dragging, setDragging] = useState<string | null>(null);
  const drag = useRef<{ id: string; y: number; startY: number; scrollTop: number } | null>(null);
  const [dragOffset, setDragOffset] = useState(0);
  const previousFocus = useRef<string | null>(null);
  useEffect(() => {
    if (focusId !== previousFocus.current && focusId && pinnedIds.includes(focusId)) setCollapsed(false);
    previousFocus.current = focusId;
  }, [focusId, pinnedIds]);
  const entries = useMemo(() => {
    const pins = new Set(pinnedIds);
    const groups = [items.filter((item) => pins.has(item.workspace.id)), items.filter((item) => !pins.has(item.workspace.id))];
    let top = 0;
    const rows: { key: string; top: number; height: number; item?: T; label?: string; count?: number }[] = [];
    groups.forEach((group, index) => {
      if (!group.length) return;
      rows.push({ key: `group-${index}`, top, height: GROUP_ROW_HEIGHT, label: index === 0 ? "常用" : "其他", count: group.length });
      top += GROUP_ROW_HEIGHT;
      if (index === 0 && collapsed && !searching) return;
      group.forEach((item) => {
        rows.push({ key: item.workspace.id, top, height: WORKSPACE_ROW_HEIGHT, item });
        top += WORKSPACE_ROW_HEIGHT;
      });
    });
    return { rows, total: top };
  }, [items, pinnedIds, collapsed, searching]);

  useEffect(() => {
    const element = viewport.current;
    if (!element) return;
    const update = () => setHeight(element.clientHeight);
    update();
    const observer = new ResizeObserver(update);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    viewport.current?.scrollTo({ top: 0 });
    setScrollTop(0);
    drag.current = null;
    setDragging(null);
  }, [resetKey]);

  useEffect(() => {
    const row = entries.rows.find((entry) => entry.key === focusId);
    const element = viewport.current;
    if (!row || !element) return;
    if (row.top < element.scrollTop) element.scrollTop = row.top;
    else if (row.top + row.height > element.scrollTop + element.clientHeight) {
      element.scrollTop = row.top + row.height - element.clientHeight;
    }
  }, [focusId, entries]);

  useEffect(() => {
    if (!dragging) return;
    let frame = 0;
    const scroll = () => {
      const element = viewport.current;
      if (!element || !drag.current) return;
      const rect = element.getBoundingClientRect();
      const y = drag.current.y;
      if (y < rect.top + 40) element.scrollTop -= 10;
      else if (y > rect.bottom - 40) element.scrollTop += 10;
      frame = requestAnimationFrame(scroll);
    };
    frame = requestAnimationFrame(scroll);
    return () => cancelAnimationFrame(frame);
  }, [dragging]);

  const sourceRow = entries.rows.find((row) => row.key === dragging);
  const targetRow = sourceRow && entries.rows.find((row) => row.item &&
    pinnedIds.includes(row.key) === pinnedIds.includes(sourceRow.key) &&
    sourceRow.top + dragOffset + sourceRow.height / 2 >= row.top &&
    sourceRow.top + dragOffset + sourceRow.height / 2 < row.top + row.height);
  const visible = visibleWorkspaceRows(entries.rows, entries.total, scrollTop, height);
  // Keep the captured row mounted while edge scrolling moves its original slot offscreen.
  if (sourceRow && !visible.includes(sourceRow)) visible.push(sourceRow);
  const updateDrag = () => {
    const current = drag.current;
    if (current && viewport.current) {
      setDragOffset(current.y - current.startY + viewport.current.scrollTop - current.scrollTop);
    }
  };
  return (
    <div ref={viewport} className="min-h-0 flex-1 overflow-y-auto px-1" aria-label="工作区列表"
      onScroll={(event) => { setScrollTop(event.currentTarget.scrollTop); updateDrag(); }}
      onPointerMove={(event) => { if (drag.current) { drag.current.y = event.clientY; updateDrag(); } }}
      onKeyDown={(event) => { if (event.key === "Escape") { drag.current = null; setDragging(null); } }}
      onPointerCancel={() => { drag.current = null; setDragging(null); }}
      onLostPointerCapture={() => { drag.current = null; setDragging(null); }}
      onPointerUp={(event) => {
        const source = drag.current;
        drag.current = null;
        setDragging(null);
        if (!source || !reorderEnabled) return;
        const top = (sourceRow?.top ?? 0) + (event.clientY - source.startY) + event.currentTarget.scrollTop - source.scrollTop + WORKSPACE_ROW_HEIGHT / 2;
        const target = entries.rows.find((row) => row.item && top >= row.top && top < row.top + row.height);
        if (target?.item) onMove(source.id, target.key);
      }}>
      <div style={{ height: entries.total, position: "relative" }}>
        {visible.map((row) => (
          <div key={row.key} data-workspace-row={row.item ? row.key : undefined}
            style={{ position: "absolute", top: row.top, height: row.height, left: 0, right: 0,
              zIndex: dragging === row.key ? 10 : 0,
              transform: `translateY(${dragging === row.key ? dragOffset :
                sourceRow && targetRow && row.item && row.key !== sourceRow.key &&
                pinnedIds.includes(row.key) === pinnedIds.includes(sourceRow.key) ?
                  (sourceRow.top < targetRow.top && row.top > sourceRow.top && row.top <= targetRow.top ? -WORKSPACE_ROW_HEIGHT :
                    sourceRow.top > targetRow.top && row.top >= targetRow.top && row.top < sourceRow.top ? WORKSPACE_ROW_HEIGHT : 0) : 0}px)`,
              transition: dragging === row.key ? "none" : "transform 180ms ease, top 180ms ease",
            }}>
            {row.item ? (
              <div className="group relative flex h-full items-center" style={{ borderRadius: 6, background: dragging === row.key ? "var(--ui-surface)" : undefined, boxShadow: dragging === row.key ? "0 8px 24px rgba(0,0,0,0.18)" : undefined }}>
                <button type="button" aria-label="拖动排序；更多菜单也可上移或下移" disabled={!reorderEnabled}
                  className="reveal-on-hover flex h-8 w-3.5 shrink-0 cursor-grab items-center justify-center text-[11px] text-fg-3 touch-none active:cursor-grabbing disabled:hidden"
                  onPointerDown={(event) => {
                    if (!reorderEnabled || event.button !== 0) return;
                    event.preventDefault();
                    event.stopPropagation();
                    viewport.current?.setPointerCapture(event.pointerId);
                    event.currentTarget.focus();
                    drag.current = { id: row.key, y: event.clientY, startY: event.clientY, scrollTop: viewport.current?.scrollTop ?? 0 };
                    setDragOffset(0);
                    setDragging(row.key);
                  }}><HolderOutlined /></button>
                <div className="min-w-0 flex-1">{renderItem(row.item)}</div>
              </div>
            ) : row.key === "group-0" ? (
              <button type="button" className="flex h-7 w-full items-center gap-1.5 px-2 text-[12px] text-fg-3 hover:text-fg-2"
                aria-expanded={!collapsed || searching} disabled={searching} onClick={() => setCollapsed(!collapsed)}>
                <span>{collapsed && !searching ? "▸" : "▾"}</span>{row.label}<span>{row.count}</span>
              </button>
            ) : <div className="flex h-7 items-center gap-1.5 px-2 text-[12px] text-fg-3">{row.label}<span>{row.count}</span></div>}
          </div>
        ))}
      </div>
    </div>
  );
}
