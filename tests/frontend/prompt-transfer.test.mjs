import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  buildInitialPromptImportActions,
  buildPromptExportFilename,
  buildPromptImportSelections,
  getVirtualPromptImportRange,
  summarizePromptImportActions,
} from "../../src/features/prompts/prompt-transfer-model.ts";

const read = (path) => readFileSync(path, "utf8");

const preview = {
  sourcePath: "D:\\backup.bexo-prompts.json",
  fileHash: "0".repeat(64),
  schemaVersion: 1,
  summary: {
    totalPromptCount: 4,
    readyPromptCount: 1,
    existingPromptCount: 1,
    conflictPromptCount: 1,
    duplicatePromptCount: 1,
  },
  items: [
    {
      promptIndex: 0,
      id: "00000000-0000-4000-8000-000000000001",
      title: "ready",
      contentPreview: "new",
      status: "ready",
      recommendedAction: "create",
      canCreate: true,
      canUpdate: false,
      message: "ready",
    },
    {
      promptIndex: 1,
      id: "00000000-0000-4000-8000-000000000002",
      title: "existing",
      contentPreview: "same",
      status: "existing",
      recommendedAction: "skip",
      canCreate: false,
      canUpdate: false,
      message: "existing",
    },
    {
      promptIndex: 2,
      id: "00000000-0000-4000-8000-000000000003",
      title: "conflict",
      contentPreview: "changed",
      status: "conflict",
      recommendedAction: "skip",
      canCreate: false,
      canUpdate: true,
      message: "conflict",
    },
    {
      promptIndex: 3,
      id: "00000000-0000-4000-8000-000000000004",
      title: "duplicate",
      contentPreview: "same",
      status: "duplicate",
      recommendedAction: "skip",
      canCreate: false,
      canUpdate: false,
      message: "duplicate",
    },
  ],
};

test("prompt import actions retain only choices allowed by the latest preview", () => {
  const actions = buildInitialPromptImportActions(preview, {
    0: "skip",
    1: "update",
    2: "update",
    3: "create",
  });
  assert.deepEqual(actions, {
    0: "skip",
    1: "skip",
    2: "update",
    3: "skip",
  });
  const selections = buildPromptImportSelections(preview, actions);
  assert.deepEqual(summarizePromptImportActions(selections), {
    create: 0,
    update: 1,
    skip: 3,
  });
});

test("prompt import virtualization remains bounded for the 1000 item limit", () => {
  const range = getVirtualPromptImportRange(1_000, 104 * 500, 416, 104, 5);
  assert.equal(range.totalHeight, 104_000);
  assert.equal(range.startIndex, 495);
  assert.ok(range.endIndex <= 509);
  assert.ok(range.endIndex < 1_000);
});

test("prompt export filename uses a deterministic local calendar date", () => {
  assert.equal(
    buildPromptExportFilename(new Date(2026, 8, 2, 23, 59, 59)),
    "bexo-prompts-2026-09-02.bexo-prompts.json",
  );
});

test("prompt transfer is wired through Rust, permissions, safe UX, and draft protection", () => {
  const page = read("src/pages/prompts-page.tsx");
  const sidebar = read("src/features/prompts/prompt-sidebar.tsx");
  const dialog = read("src/features/prompts/prompt-transfer-dialog.tsx");
  const client = read("src/lib/command-client.ts");
  const buildScript = read("src-tauri/build.rs");
  const handler = read("src-tauri/src/app/mod.rs");
  const permissions = read("src-tauri/permissions/window-commands.toml");
  const service = read("src-tauri/src/services/prompt_transfer_service.rs");
  const repository = read("src-tauri/src/persistence/prompt_transfer_repo.rs");

  assert.match(sidebar, /导入 Prompts/);
  assert.match(sidebar, /导出全部 Prompts/);
  assert.match(page, /放弃未保存的修改/);
  assert.match(page, /完整正文，可能含有敏感信息/);
  assert.match(dialog, /内容冲突/);
  assert.match(dialog, /数据库没有写入部分结果/);
  assert.match(dialog, /getVirtualPromptImportRange/);
  for (const command of [
    "export_prompt_list",
    "preview_prompt_list_import",
    "apply_prompt_list_import",
  ]) {
    assert.match(client, new RegExp(`"${command}"`));
    assert.match(buildScript, new RegExp(`"${command}"`));
    assert.match(handler, new RegExp(command));
    assert.match(permissions, new RegExp(`allow-${command.replaceAll("_", "-")}`));
  }
  assert.match(service, /PROMPT_TRANSFER_FILE_CHANGED/);
  assert.match(service, /PROMPT_TRANSFER_MAX_FILE_BYTES/);
  assert.match(service, /MoveFileExW/);
  assert.match(repository, /apply_prompt_import/);
  assert.match(repository, /PROMPT_TRANSFER_SELECTION_INVALID/);
});
