import { MoonOutlined, SunOutlined } from "@ant-design/icons";
import { Tooltip } from "antd";
import type { ComponentType, ReactNode } from "react";
import { Link, useLocation } from "react-router-dom";

import { AppLogo } from "@/components/shell/app-logo";
import { cn } from "@/lib/cn";
import { primaryNavigation } from "@/lib/navigation";
import { useShellStore } from "@/stores/shell-store";

const railButtonClassName =
  "flex h-9 w-9 items-center justify-center rounded-md text-[18px] transition-colors focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-primary";

export function PrimaryRail() {
  const location = useLocation();
  const themeMode = useShellStore((state) => state.themeMode);
  const toggleThemeMode = useShellStore((state) => state.toggleThemeMode);
  const isDark = themeMode === "dark";
  const mainItems = primaryNavigation.filter((item) => item.key !== "settings");
  const settingsItem = primaryNavigation.find((item) => item.key === "settings");

  const isActive = (href: string) =>
    href === "/" ? location.pathname === "/" : location.pathname === href || location.pathname.startsWith(`${href}/`);

  return (
    <nav
      aria-label="主导航"
      className="flex h-full w-12 shrink-0 flex-col items-center gap-1 border-r border-line bg-canvas py-2"
    >
      <div className="mb-1">
        <AppLogo />
      </div>

      {mainItems.map((item) => (
        <RailLink active={isActive(item.href)} href={item.href} icon={item.icon} key={item.key} label={item.label} />
      ))}

      <div className="mt-auto flex flex-col items-center gap-1">
        <RailTooltip label={isDark ? "切换到亮色主题" : "切换到暗色主题"}>
          <button
            aria-label={isDark ? "切换到亮色主题" : "切换到暗色主题"}
            className={cn(railButtonClassName, "text-fg-3 hover:bg-hover hover:text-fg")}
            onClick={toggleThemeMode}
            type="button"
          >
            {isDark ? <SunOutlined /> : <MoonOutlined />}
          </button>
        </RailTooltip>
        {settingsItem ? (
          <RailLink
            active={isActive(settingsItem.href)}
            href={settingsItem.href}
            icon={settingsItem.icon}
            label={settingsItem.label}
          />
        ) : null}
      </div>
    </nav>
  );
}

function RailTooltip({ children, label }: { children: ReactNode; label: string }) {
  return (
    <Tooltip mouseEnterDelay={0.3} placement="right" title={label}>
      {children}
    </Tooltip>
  );
}

function RailLink({
  active,
  href,
  icon: Icon,
  label,
}: {
  active: boolean;
  href: string;
  icon: ComponentType<{ className?: string }>;
  label: string;
}) {
  return (
    <RailTooltip label={label}>
      <Link
        aria-current={active ? "page" : undefined}
        aria-label={label}
        className={cn(
          railButtonClassName,
          active ? "bg-selected !text-primary" : "!text-fg-3 hover:bg-hover hover:!text-fg",
        )}
        to={href}
      >
        <Icon />
      </Link>
    </RailTooltip>
  );
}
