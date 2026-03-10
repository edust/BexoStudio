import { Layout } from "antd";
import { Outlet, useLocation } from "react-router-dom";

import { PrimaryRail } from "@/components/shell/primary-rail";
import { SectionSidebar } from "@/components/shell/section-sidebar";
import { routeKeyFromPathname, sidebarContentByRoute } from "@/lib/navigation";

const { Content } = Layout;

export function AppShell() {
  const location = useLocation();
  const routeKey = routeKeyFromPathname(location.pathname);
  const sidebarContent = sidebarContentByRoute[routeKey];

  return (
    <div className="h-screen overflow-hidden bg-background p-2">
      <div className="grid h-full min-h-0 grid-cols-[52px_280px_minmax(0,1fr)] gap-2">
        <PrimaryRail />
        <SectionSidebar content={sidebarContent} />
        <Layout className="bexo-shell-surface min-h-0 overflow-hidden rounded-[16px]">
          <Content className="min-h-0 overflow-y-auto px-0 py-0">
            <Outlet />
          </Content>
        </Layout>
      </div>
    </div>
  );
}
