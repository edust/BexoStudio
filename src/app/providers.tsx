import { App as AntApp, ConfigProvider, Empty, theme, type ThemeConfig } from "antd";
import { QueryClientProvider } from "@tanstack/react-query";
import { type PropsWithChildren, useEffect, useMemo, useState } from "react";
import { Toaster } from "sonner";
import zhCN from "antd/locale/zh_CN";

import { AppErrorBoundary } from "@/app/error-boundary";
import { bootstrapDesktopRuntime } from "@/lib/tauri-runtime";
import { createAppQueryClient } from "@/queries/query-client";
import { useShellStore } from "@/stores/shell-store";

const lightAppTheme: ThemeConfig = {
  algorithm: [theme.defaultAlgorithm, theme.compactAlgorithm],
  token: {
    colorPrimary: "#1697c5",
    colorInfo: "#1697c5",
    colorSuccess: "#25a56a",
    colorWarning: "#d99215",
    colorError: "#cf5a4a",
    colorBgBase: "#f3f6fb",
    colorBgContainer: "#ffffff",
    colorBgElevated: "#ffffff",
    colorText: "#1f2937",
    colorTextSecondary: "#667085",
    colorTextTertiary: "#98a2b3",
    colorBorder: "#d9e2ec",
    colorBorderSecondary: "#e6edf5",
    borderRadius: 10,
    borderRadiusLG: 14,
    borderRadiusSM: 8,
    controlHeight: 34,
    controlHeightLG: 38,
    controlHeightSM: 28,
    fontFamily: `"MiSans","Segoe UI","PingFang SC","Noto Sans SC",sans-serif`,
    boxShadowSecondary: "0 18px 42px -28px rgba(15, 23, 42, 0.14)",
    motion: false,
  },
};

const darkAppTheme: ThemeConfig = {
  algorithm: [theme.darkAlgorithm, theme.compactAlgorithm],
  token: {
    colorPrimary: "#007acc",
    colorInfo: "#007acc",
    colorSuccess: "#2ea043",
    colorWarning: "#d29922",
    colorError: "#f14c4c",
    colorBgBase: "#1e1e1e",
    colorBgContainer: "#252526",
    colorBgElevated: "#2d2d30",
    colorText: "#cccccc",
    colorTextSecondary: "#a6a6a6",
    colorTextTertiary: "#8e8e8e",
    colorBorder: "#3c3c3c",
    colorBorderSecondary: "#2f2f2f",
    borderRadius: 10,
    borderRadiusLG: 14,
    borderRadiusSM: 8,
    controlHeight: 34,
    controlHeightLG: 38,
    controlHeightSM: 28,
    fontFamily: `"MiSans","Segoe UI","PingFang SC","Noto Sans SC",sans-serif`,
    boxShadowSecondary: "0 12px 28px -20px rgba(0, 0, 0, 0.55)",
    motion: false,
  },
};

export function AppProviders({ children }: PropsWithChildren) {
  const [queryClient] = useState(createAppQueryClient);
  const themeMode = useShellStore((state) => state.themeMode);
  const appTheme = useMemo<ThemeConfig>(
    () => (themeMode === "dark" ? darkAppTheme : lightAppTheme),
    [themeMode],
  );
  const toasterClassNames = useMemo(
    () =>
      themeMode === "dark"
        ? {
            toast:
              "border border-[#3c3c3c] bg-[#2d2d30] text-[#cccccc] shadow-[0_12px_28px_-20px_rgba(0,0,0,0.55)]",
            description: "text-[#a6a6a6]",
            actionButton: "bg-[#007acc] text-[#ffffff]",
          }
        : {
            toast:
              "border border-[#d9e2ec] bg-white text-[#1f2937] shadow-[0_18px_42px_-28px_rgba(15,23,42,0.14)]",
            description: "text-[#667085]",
            actionButton: "bg-[#1697c5] text-white",
          },
    [themeMode],
  );

  useEffect(() => {
    void bootstrapDesktopRuntime();
  }, []);

  useEffect(() => {
    if (typeof document === "undefined") {
      return;
    }

    const root = document.documentElement;
    root.dataset.theme = themeMode;
    root.style.colorScheme = themeMode;
  }, [themeMode]);

  useEffect(() => {
    function isEditableTarget(target: EventTarget | null) {
      if (!(target instanceof HTMLElement)) {
        return false;
      }

      if (target.closest(".allow-text-selection, .allow-context-menu")) {
        return true;
      }

      return Boolean(
        target.closest(
          'input, textarea, [contenteditable="true"], [contenteditable="plaintext-only"]',
        ),
      );
    }

    function handleClipboardEvent(event: ClipboardEvent) {
      const eventTarget = event.target ?? document.activeElement;
      if (isEditableTarget(eventTarget)) {
        return;
      }

      event.preventDefault();
    }

    function handleContextMenuEvent(event: MouseEvent) {
      const eventTarget = event.target ?? document.activeElement;
      if (isEditableTarget(eventTarget)) {
        return;
      }

      event.preventDefault();
    }

    document.addEventListener("copy", handleClipboardEvent, true);
    document.addEventListener("cut", handleClipboardEvent, true);
    document.addEventListener("contextmenu", handleContextMenuEvent, true);

    return () => {
      document.removeEventListener("copy", handleClipboardEvent, true);
      document.removeEventListener("cut", handleClipboardEvent, true);
      document.removeEventListener("contextmenu", handleContextMenuEvent, true);
    };
  }, []);

  return (
    <AppErrorBoundary>
      <ConfigProvider
        componentSize="small"
        locale={zhCN}
        renderEmpty={() => <Empty description="暂无内容" image={Empty.PRESENTED_IMAGE_SIMPLE} />}
        theme={appTheme}
        variant="outlined"
      >
        <AntApp>
          <QueryClientProvider client={queryClient}>
            {children}
            <Toaster
              closeButton
              expand
              position="top-right"
              richColors
              theme={themeMode}
              toastOptions={{
                classNames: toasterClassNames,
              }}
            />
          </QueryClientProvider>
        </AntApp>
      </ConfigProvider>
    </AppErrorBoundary>
  );
}
