import { App as AntApp, ConfigProvider, Empty, theme, type ThemeConfig } from "antd";
import { QueryClientProvider } from "@tanstack/react-query";
import { type PropsWithChildren, useEffect, useMemo, useState } from "react";
import { Toaster, toast } from "sonner";
import zhCN from "antd/locale/zh_CN";

import { AppErrorBoundary } from "@/app/error-boundary";
import {
  getErrorSummary,
  getHotkeyHealth,
  hasDesktopRuntime,
  listenToHotkeyTriggerEvents,
  listenToPromptQuickPasteResultEvents,
} from "@/lib/command-client";
import { bootstrapDesktopRuntime } from "@/lib/tauri-runtime";
import { createAppQueryClient } from "@/queries/query-client";
import { uiFontFamily, uiPalette } from "@/lib/theme-tokens";
import { useShellStore } from "@/stores/shell-store";

const RUNTIME_STYLE_NONCE_ELEMENT_ID = "bexo-runtime-style-nonce";
const runtimeStyleNonce = resolveRuntimeStyleNonce();

function buildAppTheme(mode: "light" | "dark"): ThemeConfig {
  const palette = uiPalette[mode];
  return {
    algorithm: mode === "dark" ? theme.darkAlgorithm : theme.defaultAlgorithm,
    token: {
      colorPrimary: palette.primary,
      colorInfo: palette.primary,
      colorLink: palette.primary,
      colorSuccess: palette.success,
      colorWarning: palette.warning,
      colorError: palette.danger,
      colorBgBase: palette.canvas,
      colorBgLayout: palette.canvas,
      colorBgContainer: palette.surface,
      colorBgElevated: palette.surface,
      colorText: palette.fg,
      colorTextSecondary: palette.fg2,
      colorTextTertiary: palette.fg3,
      colorBorder: palette.lineStrong,
      colorBorderSecondary: palette.line,
      fontSize: 13,
      fontSizeSM: 12,
      borderRadius: 6,
      borderRadiusLG: 8,
      borderRadiusSM: 4,
      controlHeight: 30,
      controlHeightLG: 34,
      controlHeightSM: 24,
      fontFamily: uiFontFamily,
      boxShadowSecondary:
        mode === "dark"
          ? "0 8px 24px -12px rgba(0, 0, 0, 0.6)"
          : "0 8px 24px -12px rgba(15, 23, 42, 0.18)",
      motion: false,
    },
  };
}

const lightAppTheme = buildAppTheme("light");
const darkAppTheme = buildAppTheme("dark");

let hotkeyHealthWarningShown = false;

export function AppProviders({ children }: PropsWithChildren) {
  const [queryClient] = useState(createAppQueryClient);
  const mainWindowRoute = isMainWindowRoute();
  const themeMode = useShellStore((state) => state.themeMode);
  const appTheme = useMemo<ThemeConfig>(
    () => (themeMode === "dark" ? darkAppTheme : lightAppTheme),
    [themeMode],
  );
  const toasterClassNames = useMemo(
    () => ({
      toast: "border border-line bg-surface text-fg shadow-[0_8px_24px_-12px_rgba(15,23,42,0.25)]",
      description: "text-fg-2",
      actionButton: "bg-primary text-white",
    }),
    [],
  );

  useEffect(() => {
    if (!mainWindowRoute) {
      return;
    }
    void bootstrapDesktopRuntime();
  }, [mainWindowRoute]);

  useEffect(() => {
    if (!mainWindowRoute || !hasDesktopRuntime()) {
      return;
    }

    let disposed = false;
    void getHotkeyHealth()
      .then((health) => {
        if (disposed || health.status !== "degraded" || hotkeyHealthWarningShown) {
          return;
        }
        hotkeyHealthWarningShown = true;
        toast.error("全局热键初始化失败", {
          description:
            health.lastError?.message ?? "请在设置页检查快捷键冲突并重试注册。",
          duration: 10_000,
        });
      })
      .catch((error) => {
        if (!disposed) {
          console.error("Failed to query hotkey health", getErrorSummary(error));
        }
      });

    return () => {
      disposed = true;
    };
  }, [mainWindowRoute]);

  useEffect(() => {
    if (!mainWindowRoute || !hasDesktopRuntime()) {
      return;
    }

    let disposed = false;
    let unlisten: (() => void) | null = null;

    void (async () => {
      try {
        const attached = await listenToHotkeyTriggerEvents((event) => {
          if (
            event.action === "screenshot_capture" ||
            event.action.startsWith("prompt_quick_paste_")
          ) {
            return;
          }
          const label = resolveHotkeyActionLabel(event.action);
          toast.info(`${label}已触发`, {
            description: event.shortcut,
          });
        });

        if (disposed) {
          attached();
          return;
        }

        unlisten = attached;
      } catch (error) {
        console.error("Failed to listen hotkey trigger events", error);
      }
    })();

    return () => {
      disposed = true;
      if (unlisten) {
        unlisten();
      }
    };
  }, [mainWindowRoute]);

  useEffect(() => {
    if (!mainWindowRoute || !hasDesktopRuntime()) {
      return;
    }

    let disposed = false;
    let unlisten: (() => void) | null = null;

    void (async () => {
      try {
        const attached = await listenToPromptQuickPasteResultEvents((event) => {
          if (event.status === "sent") {
            return;
          }

          const description = `${event.shortcut} · 槽位 ${event.slot}`;
          const errorMessage = event.error?.message ?? event.message;
          toast.error("Prompt 快速粘贴失败", {
            description: `${errorMessage}（${description}）`,
            duration: 10_000,
          });
        });

        if (disposed) {
          attached();
          return;
        }
        unlisten = attached;
      } catch (error) {
        console.error("Failed to listen prompt quick paste result events", error);
      }
    })();

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [mainWindowRoute]);

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
        button={{ autoInsertSpace: false }}
        componentSize="small"
        csp={runtimeStyleNonce ? { nonce: runtimeStyleNonce } : undefined}
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

function resolveRuntimeStyleNonce() {
  if (typeof document === "undefined") {
    return undefined;
  }

  const nonceAnchor = document.getElementById(RUNTIME_STYLE_NONCE_ELEMENT_ID);
  const nonce = nonceAnchor instanceof HTMLStyleElement ? (nonceAnchor.nonce ?? "").trim() : "";
  if (!nonce && import.meta.env.PROD && hasDesktopRuntime()) {
    console.error(
      "Tauri production runtime style nonce is unavailable; dynamic component styles may be blocked",
    );
  }
  return nonce || undefined;
}

function resolveHotkeyActionLabel(action: string) {
  switch (action) {
    case "screenshot_capture":
      return "截图热键";
    case "voice_input_toggle":
      return "语音输入切换热键";
    case "voice_input_hold":
      return "语音输入按住热键";
    default:
      return "热键";
  }
}

function isMainWindowRoute() {
  if (typeof window === "undefined") {
    return true;
  }
  const searchParams = new URLSearchParams(window.location.search);
  return !searchParams.has("overlay") && !searchParams.has("window");
}
