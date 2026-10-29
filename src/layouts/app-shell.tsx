import { Outlet, useLocation } from "react-router-dom";

import { ResizableWorkspaceSidebar } from "@/components/shell/resizable-workspace-sidebar";
import { PrimaryRail } from "@/components/shell/primary-rail";
import { SectionSidebar } from "@/components/shell/section-sidebar";
import { OssAccountSidebar } from "@/features/oss/oss-account-sidebar";
import { routeKeyFromPathname, sidebarContentByRoute } from "@/lib/navigation";

// Routes that render their own list pane inside the content area.
const ROUTES_WITHOUT_SECTION_SIDEBAR = new Set(["history", "codexAuth", "prompts", "settings"]);
// Routes whose page manages scrolling internally.
const ROUTES_OWNING_SCROLL = new Set(["history", "codexAuth", "prompts", "oss", "home"]);

export function AppShell() {
  const location = useLocation();
  const routeKey = routeKeyFromPathname(location.pathname);
  const sidebarContent = sidebarContentByRoute[routeKey];
  const showSectionSidebar = !ROUTES_WITHOUT_SECTION_SIDEBAR.has(routeKey);

  let sidebar = null;
  if (showSectionSidebar) {
    if (routeKey === "oss") {
      sidebar = (
        <div className="h-full min-h-0 w-[280px] shrink-0 border-r border-line">
          <OssAccountSidebar />
        </div>
      );
    } else if (sidebarContent && routeKey === "home") {
      sidebar = (
        <ResizableWorkspaceSidebar>
          <SectionSidebar content={sidebarContent} />
        </ResizableWorkspaceSidebar>
      );
    } else if (sidebarContent) {
      sidebar = (
        <div className="h-full min-h-0 w-[240px] shrink-0 border-r border-line">
          <SectionSidebar content={sidebarContent} />
        </div>
      );
    }
  }

  return (
    <div className="flex h-screen overflow-hidden bg-canvas text-fg">
      <PrimaryRail />
      {sidebar}
      <main
        className={
          ROUTES_OWNING_SCROLL.has(routeKey)
            ? "h-full min-h-0 min-w-0 flex-1 overflow-hidden bg-surface"
            : "h-full min-h-0 min-w-0 flex-1 overflow-y-auto bg-surface"
        }
      >
        <Outlet />
      </main>
    </div>
  );
}
