import { MoonOutlined, SunOutlined } from "@ant-design/icons";
import { Divider, Tooltip } from "antd";
import { Link, useLocation } from "react-router-dom";

import { AppLogo } from "@/components/shell/app-logo";
import { cn } from "@/lib/cn";
import { primaryNavigation } from "@/lib/navigation";
import { useShellStore } from "@/stores/shell-store";

export function PrimaryRail() {
  const location = useLocation();
  const themeMode = useShellStore((state) => state.themeMode);
  const toggleThemeMode = useShellStore((state) => state.toggleThemeMode);
  const isDark = themeMode === "dark";
  const homeItem = primaryNavigation.find((item) => item.key === "home");
  const settingsItem = primaryNavigation.find((item) => item.key === "settings");

  return (
    <aside
      className={cn(
        "bexo-shell-surface flex h-full min-h-0 w-full flex-col items-center px-1 py-2",
        isDark
          ? "rounded-[14px] border-[#2a2a2a] bg-[#181818] shadow-none"
          : "rounded-[14px]",
      )}
    >
      <div className="flex w-full justify-center">
        <AppLogo />
      </div>

      <Divider className={cn("my-2", isDark ? "border-[#2a2a2a]" : "border-[#e6edf5]")} />

      <div className="flex w-full flex-1 flex-col items-center gap-0.5">
        {homeItem ? (
          <PrimaryRailButton
            item={homeItem}
            active={location.pathname === "/"}
            themeMode={themeMode}
          />
        ) : null}
      </div>

      <div className="mt-auto flex w-full flex-col items-center gap-0.5">
        <ThemeToggleButton mode={themeMode} onToggle={toggleThemeMode} />
        {settingsItem ? (
          <PrimaryRailButton
            item={settingsItem}
            active={location.pathname.startsWith("/settings")}
            themeMode={themeMode}
          />
        ) : null}
      </div>
    </aside>
  );
}

function PrimaryRailButton({
  item,
  active,
  themeMode,
}: {
  item: (typeof primaryNavigation)[number];
  active: boolean;
  themeMode: "light" | "dark";
}) {
  const Icon = item.icon;
  const isDark = themeMode === "dark";
  const iconClassName = cn(
    "text-[20px] transition-colors",
    active
      ? isDark
        ? "!text-[#ffffff]"
        : "!text-[#475467]"
      : isDark
        ? "!text-[#d4d4d4] group-hover:!text-[#ffffff]"
        : "!text-[#7b8794] group-hover:!text-[#1f2937]",
  );

  return (
    <Tooltip placement="right" title={item.label}>
      <div className="relative flex w-full justify-center">
        <Link
          aria-label={item.label}
          className={cn(
            "group flex h-[40px] w-[40px] items-center justify-center rounded-[8px] border text-[20px] transition-colors",
            active
              ? isDark
                ? "border-[#4a4f55] bg-[#34373a]"
                : "border-[#d9e2ec] bg-[#edf1f5]"
              : isDark
                ? "border-transparent bg-[#2a2d2e] hover:bg-[#34373a]"
                : "border-transparent bg-transparent hover:bg-[#f7fafc]",
          )}
          to={item.href}
        >
          <Icon className={iconClassName} />
        </Link>
      </div>
    </Tooltip>
  );
}

function ThemeToggleButton({
  mode,
  onToggle,
}: {
  mode: "light" | "dark";
  onToggle: () => void;
}) {
  const isDark = mode === "dark";
  const Icon = isDark ? SunOutlined : MoonOutlined;
  const iconClassName = cn(
    "text-[20px] transition-colors",
    isDark ? "!text-[#d4d4d4] group-hover:!text-[#ffffff]" : "!text-[#7b8794] group-hover:!text-[#1f2937]",
  );

  return (
    <Tooltip placement="right" title={isDark ? "切换到亮色主题" : "切换到暗色主题"}>
      <div className="relative flex w-full justify-center">
        <button
          aria-label={isDark ? "切换到亮色主题" : "切换到暗色主题"}
          className={cn(
            "group flex h-[40px] w-[40px] items-center justify-center rounded-[8px] border border-transparent text-[20px] transition-colors",
            isDark
              ? "bg-[#2a2d2e] text-[#d4d4d4] hover:bg-[#34373a] hover:text-[#ffffff]"
              : "bg-transparent text-[#7b8794] hover:bg-[#f7fafc] hover:text-[#1f2937]",
          )}
          onClick={onToggle}
          type="button"
        >
          <Icon className={iconClassName} />
        </button>
      </div>
    </Tooltip>
  );
}
