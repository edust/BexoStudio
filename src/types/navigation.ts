import type { ComponentType } from "react";

export type AppRouteKey =
  | "home"
  | "oss"
  | "history"
  | "codexAuth"
  | "prompts"
  | "settings"
  | "frozen";

export type PrimaryNavItem = {
  key: "home" | "oss" | "history" | "codexAuth" | "prompts" | "settings";
  label: string;
  href: string;
  icon: ComponentType<{ className?: string }>;
};

export type SidebarItem = {
  key: string;
  label: string;
  description?: string;
  href?: string;
};

export type SectionSidebarContent = {
  title: string;
  searchPlaceholder?: string;
  dataSource?: "workspaces";
  items: SidebarItem[];
};
