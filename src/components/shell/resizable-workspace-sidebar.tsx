import { useEffect, useRef, useState, type ReactNode } from "react";
import { toast } from "sonner";
import { DEFAULT_WORKSPACE_SIDEBAR_WIDTH, MIN_WORKSPACE_SIDEBAR_WIDTH, MAX_WORKSPACE_SIDEBAR_WIDTH, WORKSPACE_SIDEBAR_WIDTH_KEY, workspaceSidebarWidth } from "./workspace-sidebar-width";

export function ResizableWorkspaceSidebar({ children }: { children: ReactNode }) {
  const [initial] = useState(() => {
    try {
      const stored = window.localStorage.getItem(WORKSPACE_SIDEBAR_WIDTH_KEY);
      return { width: stored === null ? DEFAULT_WORKSPACE_SIDEBAR_WIDTH : Number(stored), error: false };
    } catch {
      return { width: DEFAULT_WORKSPACE_SIDEBAR_WIDTH, error: true };
    }
  });
  const [preferred, setPreferred] = useState(initial.width);
  const [viewportWidth, setViewportWidth] = useState(window.innerWidth);
  const drag = useRef<{ x: number; width: number; preferred: number } | null>(null);
  const latest = useRef(preferred);
  const width = workspaceSidebarWidth(preferred, viewportWidth);
  useEffect(() => {
    if (initial.error) toast.warning("无法读取项目栏宽度，已使用默认宽度");
    const resize = () => setViewportWidth(window.innerWidth);
    window.addEventListener("resize", resize);
    return () => window.removeEventListener("resize", resize);
  }, [initial.error]);

  function update(value: number) {
    latest.current = value;
    setPreferred(value);
  }
  function save(value: number) {
    update(value);
    try { window.localStorage.setItem(WORKSPACE_SIDEBAR_WIDTH_KEY, String(value)); }
    catch { toast.error("项目栏宽度已调整，但保存失败；重启后将恢复原宽度"); }
  }
  function cancelDrag() {
    if (drag.current) update(drag.current.preferred);
    drag.current = null;
  }
  return <div className="relative h-full min-h-0 shrink-0 border-r border-line" style={{ width }} data-workspace-sidebar>
    {children}
    <div role="separator" aria-label="调整项目栏宽度" aria-orientation="vertical" tabIndex={0}
      aria-valuemin={MIN_WORKSPACE_SIDEBAR_WIDTH} aria-valuemax={workspaceSidebarWidth(MAX_WORKSPACE_SIDEBAR_WIDTH, viewportWidth)} aria-valuenow={Math.round(width)}
      title="拖动调整项目栏宽度；方向键微调，双击恢复默认"
      className="absolute -right-1 top-0 z-20 h-full w-2 cursor-col-resize touch-none hover:bg-primary/25 focus-visible:bg-primary/25 focus-visible:outline-none"
      onDoubleClick={() => save(DEFAULT_WORKSPACE_SIDEBAR_WIDTH)}
      onPointerDown={(event) => {
        if (event.button !== 0) return;
        event.preventDefault();
        event.currentTarget.focus();
        drag.current = { x: event.clientX, width, preferred };
        event.currentTarget.setPointerCapture(event.pointerId);
      }}
      onPointerMove={(event) => {
        if (drag.current) update(workspaceSidebarWidth(drag.current.width + event.clientX - drag.current.x, window.innerWidth));
      }}
      onPointerUp={() => { if (drag.current) { drag.current = null; save(latest.current); } }}
      onPointerCancel={cancelDrag} onLostPointerCapture={cancelDrag}
      onKeyDown={(event) => {
        if (event.key === "Escape") { cancelDrag(); return; }
        if (drag.current) return;
        const value = event.key === "ArrowLeft" ? width - 16 : event.key === "ArrowRight" ? width + 16 : event.key === "Home" ? DEFAULT_WORKSPACE_SIDEBAR_WIDTH : event.key === "End" ? MAX_WORKSPACE_SIDEBAR_WIDTH : null;
        if (value !== null) { event.preventDefault(); save(workspaceSidebarWidth(value, viewportWidth)); }
      }} />
  </div>;
}
