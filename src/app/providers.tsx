import { App as AntApp, ConfigProvider, Empty, theme, type ThemeConfig } from "antd";
import { QueryClientProvider } from "@tanstack/react-query";
import { type PropsWithChildren, useEffect, useState } from "react";
import { Toaster } from "sonner";
import zhCN from "antd/locale/zh_CN";

import { AppErrorBoundary } from "@/app/error-boundary";
import { bootstrapDesktopRuntime } from "@/lib/tauri-runtime";
import { createAppQueryClient } from "@/queries/query-client";

const appTheme: ThemeConfig = {
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

export function AppProviders({ children }: PropsWithChildren) {
  const [queryClient] = useState(createAppQueryClient);

  useEffect(() => {
    void bootstrapDesktopRuntime();
  }, []);

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
              theme="light"
              toastOptions={{
                classNames: {
                  toast:
                    "border border-[#d9e2ec] bg-white text-[#1f2937] shadow-[0_18px_42px_-28px_rgba(15,23,42,0.14)]",
                  description: "text-[#667085]",
                  actionButton: "bg-[#1697c5] text-white",
                },
              }}
            />
          </QueryClientProvider>
        </AntApp>
      </ConfigProvider>
    </AppErrorBoundary>
  );
}
