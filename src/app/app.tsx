import { AppProviders } from "@/app/providers";
import CodexHistoryWindowPage from "@/pages/codex-history-window-page";
import ScreenshotOverlayPage from "@/pages/screenshot-overlay-page";
import { AppRouter } from "@/routes/app-router";

export function App() {
  const windowSearchParams =
    typeof window !== "undefined" ? new URLSearchParams(window.location.search) : null;
  const isScreenshotOverlay =
    windowSearchParams?.get("overlay") === "screenshot";
  const isCodexHistoryWindow =
    windowSearchParams?.get("window") === "codex-history";

  return (
    <AppProviders>
      {isScreenshotOverlay ? (
        <ScreenshotOverlayPage />
      ) : isCodexHistoryWindow ? (
        <CodexHistoryWindowPage />
      ) : (
        <AppRouter />
      )}
    </AppProviders>
  );
}
