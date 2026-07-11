import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  buildPromptQuickPasteBindingsByPromptId,
  defaultPromptQuickPasteShortcut,
  normalizePromptQuickPasteSlots,
} from "../../src/features/prompts/prompt-quick-paste.ts";

const promptId = "11111111-1111-4111-8111-111111111111";

test("Prompt quick paste defaults expose exactly five requested shortcuts", () => {
  const slots = normalizePromptQuickPasteSlots(undefined);
  assert.equal(slots.length, 5);
  assert.deepEqual(
    slots.map((slot) => slot.shortcut),
    [1, 2, 3, 4, 5].map(defaultPromptQuickPasteShortcut),
  );
  assert.equal(slots.every((slot) => !slot.enabled && slot.promptId === null), true);
});

test("Prompt quick paste normalization keeps stable slots and disables missing bindings", () => {
  const slots = normalizePromptQuickPasteSlots([
    {
      slot: 3,
      enabled: true,
      promptId,
      shortcut: " Ctrl+Shift+F3 ",
    },
    {
      slot: 1,
      enabled: true,
      promptId: " ",
      shortcut: "",
    },
  ]);
  assert.equal(slots[0].enabled, false);
  assert.equal(slots[0].shortcut, "Ctrl+Alt+Shift+1");
  assert.deepEqual(slots[2], {
    slot: 3,
    enabled: true,
    promptId,
    shortcut: "Ctrl+Shift+F3",
  });
});

test("Prompt quick paste badges resolve by stable Prompt id, not list order", () => {
  const bindings = buildPromptQuickPasteBindingsByPromptId([
    {
      slot: 1,
      enabled: true,
      promptId,
      shortcut: "Ctrl+Alt+Shift+1",
    },
    {
      slot: 2,
      enabled: false,
      promptId,
      shortcut: "Ctrl+Alt+Shift+2",
    },
  ]);
  assert.deepEqual(bindings.get(promptId), [
    { slot: 1, shortcut: "Ctrl+Alt+Shift+1" },
  ]);
});

test("Settings and Prompt list expose the complete quick paste UI", () => {
  const settingsPage = readFileSync("src/pages/settings-page.tsx", "utf8");
  const settingsPanel = readFileSync(
    "src/features/settings/prompt-quick-paste-settings.tsx",
    "utf8",
  );
  const promptItem = readFileSync("src/features/prompts/prompt-list-item.tsx", "utf8");
  const navigation = readFileSync("src/lib/navigation.ts", "utf8");
  const pasteService = readFileSync(
    "src-tauri/src/services/prompt_quick_paste_service.rs",
    "utf8",
  );
  const pasteAdapter = readFileSync("src-tauri/src/adapters/prompt_paste.rs", "utf8");

  assert.match(settingsPage, /<PromptQuickPasteSettings/);
  assert.match(settingsPanel, /Prompt 快速粘贴/);
  assert.match(settingsPanel, /normalizePromptQuickPasteSlots/);
  assert.match(settingsPanel, /选择 Prompt 后自动启用/);
  assert.match(settingsPanel, /录制热键/);
  assert.match(promptItem, /快速粘贴槽位/);
  assert.match(navigation, /key: "hotkeys"[\s\S]*?href: "\/settings\/hotkeys"/);
  assert.match(pasteService, /Arc<dyn PromptPasteAdapter>/);
  assert.match(pasteService, /hotkey:\/\/prompt-quick-paste-result|PROMPT_QUICK_PASTE_RESULT_EVENT_NAME/);
  assert.match(pasteAdapter, /wait_for_modifier_release\(\)\?;[\s\S]*?write_clipboard_with_retry/);
  assert.match(pasteAdapter, /release_paste_keys/);
  assert.match(pasteAdapter, /SendInput/);
});

test("successful Prompt quick paste stays silent while failures remain visible", () => {
  const providers = readFileSync("src/app/providers.tsx", "utf8");
  const pasteService = readFileSync(
    "src-tauri/src/services/prompt_quick_paste_service.rs",
    "utf8",
  );

  assert.match(providers, /event\.status === "sent"\) \{\s*return;/);
  assert.doesNotMatch(providers, /toast\.success\(event\.message/);
  assert.match(providers, /toast\.error\("Prompt 快速粘贴失败"/);
  assert.match(pasteService, /fn send_failure_notification/);
  assert.match(
    pasteService,
    /event\.status != PromptQuickPasteResultStatus::Failed[\s\S]*?return;/,
  );
  assert.match(pasteService, /\.title\("Prompt 快速粘贴失败"\)/);
  assert.doesNotMatch(pasteService, /\.title\("Prompt 快速粘贴"\)/);
});
