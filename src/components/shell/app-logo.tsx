import { Avatar, Tooltip } from "antd";

export function AppLogo() {
  return (
    <Tooltip placement="right" title="Bexo Studio">
      <Avatar
        className="shrink-0 border border-[#d9e2ec] bg-white text-[#1f2937]"
        shape="square"
        size={30}
        style={{ borderRadius: 9, fontWeight: 700 }}
      >
        B
      </Avatar>
    </Tooltip>
  );
}
