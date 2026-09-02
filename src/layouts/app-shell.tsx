import { Layout } from "antd";
import { Outlet, useLocation } from "react-router-dom";

import { PrimaryRail } from "@/components/shell/primary-rail";
import { SectionSidebar } from "@/components/shell/section-sidebar";
import { OssAccountSidebar } from "@/features/oss/oss-account-sidebar";
import { routeKeyFromPathname, sidebarContentByRoute } from "@/lib/navigation";

const { Content } = Layout;

export function AppShell() {
  const location = useLocation();
  const routeKey = routeKeyFromPathname(location.pathname);
  const sidebarContent = sidebarContentByRoute[routeKey];
  const showSectionSidebar =
    routeKey !== "history" && routeKey !== "codexAuth" && routeKey !== "prompts";
  const routeOwnsScroll =
    routeKey === "history" || routeKey === "codexAuth" || routeKey === "prompts" || routeKey === "oss";

  return (
    <div className="h-screen overflow-hidden bg-background p-2">
      <div
        className={
          showSectionSidebar
            ? "grid h-full min-h-0 grid-cols-[52px_320px_minmax(0,1fr)] gap-2"
            : "grid h-full min-h-0 grid-cols-[52px_minmax(0,1fr)] gap-2"
        }
      >
        <PrimaryRail />
        {showSectionSidebar ? (
          routeKey === "oss" ? <OssAccountSidebar /> : <SectionSidebar content={sidebarContent} />
        ) : null}
        <Layout className="bexo-shell-surface h-full min-h-0 overflow-hidden rounded-[16px]">
          <Content
            className={
              routeOwnsScroll
                ? "h-full min-h-0 overflow-hidden px-0 py-0"
                : "h-full min-h-0 overflow-y-auto px-0 py-0"
            }
          >
            <Outlet />
          </Content>
        </Layout>
      </div>
    </div>
  );
}
