import {
  CloudServerOutlined,
  HistoryOutlined,
  HomeOutlined,
  KeyOutlined,
  SettingOutlined,
} from "@ant-design/icons";

import { PromptLetterIcon } from "@/components/shell/prompt-letter-icon";
import type { AppRouteKey, PrimaryNavItem, SectionSidebarContent } from "@/types/navigation";

export const primaryNavigation: PrimaryNavItem[] = [
  { key: "home", label: "工作台", href: "/", icon: HomeOutlined },
  { key: "oss", label: "OSS 文件", href: "/oss", icon: CloudServerOutlined },
  { key: "history", label: "会话历史", href: "/history", icon: HistoryOutlined },
  { key: "codexAuth", label: "Codex 授权", href: "/codex-auth", icon: KeyOutlined },
  { key: "prompts", label: "Prompts", href: "/prompts", icon: PromptLetterIcon },
  { key: "settings", label: "设置", href: "/settings", icon: SettingOutlined },
];

// Only routes that render the shared SectionSidebar need content here.
export const sidebarContentByRoute: Partial<Record<AppRouteKey, SectionSidebarContent>> = {
  home: {
    title: "工作区",
    searchPlaceholder: "搜索名称、路径或备注（Ctrl+K）",
    dataSource: "workspaces",
    items: [],
  },
  frozen: {
    title: "暂未开放",
    items: [
      { key: "workspaces", label: "Workspaces", href: "/workspaces" },
      { key: "snapshots", label: "Snapshots", href: "/snapshots" },
      { key: "profiles", label: "Profiles", href: "/profiles" },
      { key: "logs", label: "Logs", href: "/logs" },
    ],
  },
};

// Settings uses in-page tabs instead of a section sidebar.
export const settingsSections = [
  { key: "general", label: "通用", href: "/settings/general" },
  { key: "hotkeys", label: "热键", href: "/settings/hotkeys" },
] as const;

export function routeKeyFromPathname(pathname: string): AppRouteKey {
  if (pathname.startsWith("/oss")) return "oss";
  if (pathname.startsWith("/history")) return "history";
  if (pathname.startsWith("/codex-auth")) return "codexAuth";
  if (pathname.startsWith("/prompts")) return "prompts";
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
