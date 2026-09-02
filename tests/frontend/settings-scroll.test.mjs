import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

test("Settings content keeps Hotkeys cards at natural height for outer scrolling", () => {
  const settingsPage = readFileSync("src/pages/settings-page.tsx", "utf8");
  const promptSettings = readFileSync(
    "src/features/settings/prompt-quick-paste-settings.tsx",
    "utf8",
  );

  assert.match(settingsPage, /flex min-h-full flex-col bg-white/);
  assert.match(settingsPage, /flex flex-1 flex-col px-4 pb-6 pt-4/);
  assert.match(settingsPage, /<div className="flex flex-col gap-0">/);
  assert.match(settingsPage, /className="shrink-0 rounded-\[0\] border/);
  assert.match(promptSettings, /className="mt-3 shrink-0 overflow-hidden/);
  assert.doesNotMatch(settingsPage, /flex min-h-0 flex-1 flex-col gap-0/);
});
