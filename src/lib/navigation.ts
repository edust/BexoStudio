import { HomeOutlined, SettingOutlined } from "@ant-design/icons";

import type { AppRouteKey, PrimaryNavItem, SectionSidebarContent } from "@/types/navigation";

export const primaryNavigation: PrimaryNavItem[] = [
  { key: "home", label: "Workbench", href: "/", icon: HomeOutlined },
  { key: "settings", label: "Settings", href: "/settings", icon: SettingOutlined },
];

export const sidebarContentByRoute: Record<AppRouteKey, SectionSidebarContent> = {
  home: {
    eyebrow: "WORKBENCH",
    title: "",
    description: "",
    searchPlaceholder: "搜索工作区名称...",
    dataSource: "workspaces",
    items: [],
  },
  settings: {
    eyebrow: "SETTINGS",
    title: "",
    description: "",
    items: [
      { key: "general", label: "General", description: "通用设置", badge: "ui", href: "/settings/general" },
      { key: "hotkeys", label: "Hotkeys", description: "截图与输入热键", badge: "new", href: "/settings/hotkeys" },
    ],
  },
  frozen: {
    eyebrow: "MODULES PARKED",
    title: "冻结模块",
    description: "这些页面暂时退出主导航，只保留空白工作页和路由壳，用于先完成统一的桌面 UI 框架。",
    searchPlaceholder: "搜索冻结模块...",
    items: [
      { key: "workspaces", label: "Workspaces", description: "冻结为占位页，等待后续按新框架重建", href: "/workspaces" },
      { key: "snapshots", label: "Snapshots", description: "冻结为占位页，旧快照面板先清空", href: "/snapshots" },
      { key: "profiles", label: "Profiles", description: "冻结为占位页，旧 Profile 编辑器已退出主流程", href: "/profiles" },
      { key: "logs", label: "Logs", description: "冻结为占位页，诊断和列表在新框架后再回填", href: "/logs" },
    ],
    footerTitle: "Module Freeze",
    footerDescription: "先把框架做对，再决定哪些业务模块值得回归。",
  },
};

export function routeKeyFromPathname(pathname: string): AppRouteKey {
  if (pathname.startsWith("/settings")) return "settings";
  if (
    pathname.startsWith("/workspaces") ||
    pathname.startsWith("/snapshots") ||
    pathname.startsWith("/profiles") ||
    pathname.startsWith("/logs")
  ) {
    return "frozen";
  }
  return "home";
}

export function frozenModuleTitleFromPathname(pathname: string) {
  if (pathname.startsWith("/workspaces")) return "Workspaces";
  if (pathname.startsWith("/snapshots")) return "Snapshots";
  if (pathname.startsWith("/profiles")) return "Profiles";
  if (pathname.startsWith("/logs")) return "Logs";
  return "Module";
}
