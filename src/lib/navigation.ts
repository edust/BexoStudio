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
  { key: "home", label: "Workbench", href: "/", icon: HomeOutlined },
  { key: "oss", label: "OSS 文件", href: "/oss", icon: CloudServerOutlined },
  { key: "history", label: "Session / History", href: "/history", icon: HistoryOutlined },
  { key: "codexAuth", label: "Codex Auth", href: "/codex-auth", icon: KeyOutlined },
  { key: "prompts", label: "Prompts", href: "/prompts", icon: PromptLetterIcon },
  { key: "settings", label: "Settings", href: "/settings", icon: SettingOutlined },
];

export const sidebarContentByRoute: Record<AppRouteKey, SectionSidebarContent> = {
  home: {
    eyebrow: "WORKBENCH",
    title: "",
    description: "",
    searchPlaceholder: "搜索名称、路径或备注...",
    dataSource: "workspaces",
    items: [],
  },
  oss: {
    eyebrow: "OSS FILES",
    title: "",
    description: "管理多个 RAM AccessKey 下的 Bucket 文件与传输任务。",
    searchPlaceholder: "搜索 OSS 账号...",
    items: [],
  },
  history: {
    eyebrow: "SESSION / HISTORY",
    title: "",
    description: "全局只读查看 Codex 会话历史，可按工作区路径筛选。",
    searchPlaceholder: "搜索历史视图...",
    items: [
      {
        key: "codex-history",
        label: "Codex History",
        description: "读取本机 Codex sessions",
        badge: "read",
        href: "/history",
      },
    ],
  },
  codexAuth: {
    eyebrow: "CODEX AUTH",
    title: "",
    description: "只管理 Codex auth.json 与 config.toml 授权配置。",
    searchPlaceholder: "搜索 Codex 授权...",
    items: [
      {
        key: "codex-auth",
        label: "Codex Auth",
        description: "管理本机 Codex 授权配置",
        badge: "local",
        href: "/codex-auth",
      },
    ],
  },
  prompts: {
    eyebrow: "PROMPTS",
    title: "",
    description: "保存、排序并一键复制常用 Prompt。",
    searchPlaceholder: "搜索 Prompts...",
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
