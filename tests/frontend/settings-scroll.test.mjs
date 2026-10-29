import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

test("Settings content keeps grouped rows at natural height for outer scrolling", () => {
  const settingsPage = readFileSync("src/pages/settings-page.tsx", "utf8");
  const promptSettings = readFileSync(
    "src/features/settings/prompt-quick-paste-settings.tsx",
    "utf8",
  );
  const shell = readFileSync("src/layouts/app-shell.tsx", "utf8");

  // The shell main element scrolls Settings; the page itself must not clamp its height.
  assert.match(settingsPage, /flex min-h-full flex-col bg-surface/);
  assert.match(settingsPage, /<SettingsGroup title="启动">/);
  assert.match(promptSettings, /className="shrink-0 space-y-1\.5"/);
  assert.doesNotMatch(settingsPage, /flex min-h-0 flex-1 flex-col gap-0/);
  assert.doesNotMatch(shell, /ROUTES_OWNING_SCROLL = new Set\(\[[^\]]*"settings"/);
});
