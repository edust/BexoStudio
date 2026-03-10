import type { ComponentType } from "react";

export type AppRouteKey =
  | "home"
  | "settings"
  | "frozen";

export type PrimaryNavItem = {
  key: "home" | "settings";
  label: string;
  href: string;
  icon: ComponentType<{ className?: string }>;
};

export type SidebarItem = {
  key: string;
  label: string;
  description: string;
  badge?: string;
  href?: string;
};

export type SectionSidebarContent = {
  eyebrow: string;
  title?: string;
  description: string;
  searchPlaceholder?: string;
  dataSource?: "workspaces";
  items: SidebarItem[];
  footerTitle?: string;
  footerDescription?: string;
};
