import { Divider, Tooltip } from "antd";
import { Link, useLocation } from "react-router-dom";

import { AppLogo } from "@/components/shell/app-logo";
import { cn } from "@/lib/cn";
import { primaryNavigation } from "@/lib/navigation";

export function PrimaryRail() {
  const location = useLocation();
  const homeItem = primaryNavigation.find((item) => item.key === "home");
  const settingsItem = primaryNavigation.find((item) => item.key === "settings");

  return (
    <aside className="bexo-shell-surface flex h-full min-h-0 w-full flex-col items-center rounded-[14px] px-1 py-2">
      <div className="flex w-full justify-center">
        <AppLogo />
      </div>

      <Divider className="my-2 border-[#e6edf5]" />

      <div className="flex w-full flex-1 flex-col items-center gap-0.5">
        {homeItem ? <PrimaryRailButton item={homeItem} active={location.pathname === "/"} /> : null}
      </div>

      <div className="mt-auto flex w-full flex-col items-center gap-0.5">
        {settingsItem ? (
          <PrimaryRailButton
            item={settingsItem}
            active={location.pathname.startsWith("/settings")}
          />
        ) : null}
      </div>
    </aside>
  );
}

function PrimaryRailButton({
  item,
  active,
}: {
  item: (typeof primaryNavigation)[number];
  active: boolean;
}) {
  const Icon = item.icon;

  return (
    <Tooltip placement="right" title={item.label}>
      <Link
        aria-label={item.label}
        className={cn(
          "flex h-[34px] w-[34px] items-center justify-center rounded-[9px] border text-[14px] transition-colors",
          active
            ? "border-[#8fd0e5] bg-[#e6f6fb] text-[#0f7ea5]"
            : "border-transparent bg-transparent text-[#7b8794] hover:border-[#d9e2ec] hover:bg-[#f7fafc] hover:text-[#1f2937]",
        )}
        to={item.href}
      >
        <Icon />
      </Link>
    </Tooltip>
  );
}
