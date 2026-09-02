import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const expectedArguments =
  "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --disable-gpu --disable-gpu-compositing --disable-software-rasterizer";

test("all Bexo Studio WebView2 creation paths disable GPU consistently", () => {
  const config = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));
  const configuredWindows = new Map(
    config.app.windows.map((windowConfig) => [windowConfig.label, windowConfig]),
  );

  for (const label of ["main", "screenshot_overlay"]) {
    assert.equal(
      configuredWindows.get(label)?.additionalBrowserArgs,
      expectedArguments,
      `${label} must use the production WebView2 browser arguments`,
    );
  }

  const servicesModule = readFileSync("src-tauri/src/services/mod.rs", "utf8");
  assert.ok(
    servicesModule.includes(expectedArguments),
    "the shared Rust WebView2 argument constant must match Tauri config",
  );

  for (const sourcePath of [
    "src-tauri/src/services/codex_history_service.rs",
    "src-tauri/src/services/screenshot_service.rs",
  ]) {
    const source = readFileSync(sourcePath, "utf8");
    assert.match(
      source,
      /additional_browser_args\(super::WINDOWS_WEBVIEW2_BROWSER_ARGS\)/,
      `${sourcePath} must apply the shared WebView2 arguments`,
    );
  }
});
