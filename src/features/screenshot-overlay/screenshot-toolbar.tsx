import { Button, ConfigProvider, Tooltip, theme } from "antd";
import type { ReactNode } from "react";
import type { ToolbarIconAction } from "./overlay-model";

// The screenshot control surface is always light, independently of the main app theme.
export function ScreenshotToolbar({ tools, session, properties, objects, status }: {
  tools: ToolbarIconAction[];
  session: ToolbarIconAction[];
  properties: ToolbarIconAction[];
  objects: ToolbarIconAction[];
  status?: ReactNode;
}) {
  return (
    <ConfigProvider theme={{ inherit: false, algorithm: [theme.defaultAlgorithm, theme.compactAlgorithm],
      token: { colorPrimary: "#1677ff", colorText: "#243247", colorBgContainer: "#ffffff",
        colorBgElevated: "#ffffff", colorBorder: "#d5dce6", colorError: "#bd3535",
        controlHeightSM: 32, borderRadiusSM: 6, motion: false } }}>
      <div className="screenshot-toolbar-main" role="toolbar" aria-label="截图工具与完成操作">
        <div className="screenshot-toolbar-scroll" role="group" aria-label="标注工具">
          {tools.map((action) => <ToolbarAction key={action.key} action={action} />)}
        </div>
        <span className="screenshot-toolbar-divider" aria-hidden="true" />
        <div className="screenshot-toolbar-completion" role="group" aria-label="完成截图">
          {session.map((action) => <ToolbarAction key={action.key} action={action} />)}
        </div>
      </div>
      {properties.length || objects.length || status ? (
        <div className="screenshot-toolbar-properties" role="toolbar" aria-label="工具属性与对象操作">
          {status}
          <div className="screenshot-toolbar-scroll">
            {properties.map((action) => <ToolbarAction key={action.key} action={action} showLabel />)}
            {properties.length && objects.length ? <span className="screenshot-toolbar-divider" aria-hidden="true" /> : null}
            {objects.map((action) => <ToolbarAction key={action.key} action={action} />)}
          </div>
        </div>
      ) : null}
    </ConfigProvider>
  );
}

function ToolbarAction({ action, showLabel = false }: { action: ToolbarIconAction; showLabel?: boolean }) {
  const propertyLabel = showLabel && !action.key.endsWith("-inc") && action.key !== "text-rotate-right"
    ? action.label.startsWith("文字样式 · ") ? action.label.slice(7)
      : action.label.replace(/\s[-+]\d+%?\s·\s当前\s/, " ").replace(/文字左转 15° · 当前 /, "旋转 ")
    : null;
  return (
    <Tooltip placement="top" title={action.label} color="#ffffff"
      styles={{ container: { color: "#243247", fontSize: 12 } }}>
      <span className="screenshot-toolbar-action">
        <Button aria-label={action.label} aria-pressed={action.active}
          className={showLabel ? "screenshot-toolbar-property-button" : "screenshot-toolbar-icon-button"}
          danger={action.danger} disabled={action.disabled} icon={action.icon} loading={action.loading}
          size="small" type={action.active || action.key === "copy" ? "primary" : "text"}
          onClick={action.onClick}>
          {propertyLabel ?? (action.key === "copy" ? "复制" : action.key === "save" ? "保存" : null)}
        </Button>
      </span>
    </Tooltip>
  );
}
