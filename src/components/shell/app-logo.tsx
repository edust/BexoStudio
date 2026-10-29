import { Tooltip } from "antd";

export function AppLogo() {
  return (
    <Tooltip mouseEnterDelay={0.3} placement="right" title="Bexo Studio">
      <span
        aria-label="Bexo Studio"
        className="flex h-8 w-8 items-center justify-center rounded-md bg-primary text-[14px] font-bold text-white"
        role="img"
      >
        B
      </span>
    </Tooltip>
  );
}
