import { lazy, Suspense } from "react";

import { AppProviders } from "@/app/providers";
import { AppRouter } from "@/routes/app-router";

const ScreenshotOverlayPage = lazy(() => import("@/pages/screenshot-overlay-page"));
const CodexHistoryWindowPage = lazy(() => import("@/pages/codex-history-window-page"));

export function App() {
  const windowSearchParams =
    typeof window !== "undefined" ? new URLSearchParams(window.location.search) : null;
  const isScreenshotOverlay =
    windowSearchParams?.get("overlay") === "screenshot";
  const isCodexHistoryWindow =
    windowSearchParams?.get("window") === "codex-history";

  return (
    <AppProviders>
      <Suspense fallback={<WindowRouteLoading />}>
        {isScreenshotOverlay ? (
          <ScreenshotOverlayPage />
        ) : isCodexHistoryWindow ? (
          <CodexHistoryWindowPage />
        ) : (
          <AppRouter />
        )}
      </Suspense>
    </AppProviders>
  );
}

function WindowRouteLoading() {
  return (
    <div className="flex h-screen w-screen items-center justify-center bg-[var(--panel)] text-sm text-[var(--muted-foreground)]">
      正在加载窗口…
    </div>
  );
}
