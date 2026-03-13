import { AppProviders } from "@/app/providers";
import ScreenshotOverlayPage from "@/pages/screenshot-overlay-page";
import { AppRouter } from "@/routes/app-router";

export function App() {
  const isScreenshotOverlay =
    typeof window !== "undefined" &&
    new URLSearchParams(window.location.search).get("overlay") === "screenshot";

  return (
    <AppProviders>
      {isScreenshotOverlay ? <ScreenshotOverlayPage /> : <AppRouter />}
    </AppProviders>
  );
}
