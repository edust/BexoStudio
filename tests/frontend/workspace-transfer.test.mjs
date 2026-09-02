import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  buildInitialWorkspaceImportActions,
  buildWorkspaceImportSelections,
  getVirtualWorkspaceRange,
  summarizeWorkspaceImportActions,
} from "../../src/features/workspaces/workspace-transfer-model.ts";

const read = (path) => readFileSync(path, "utf8");

const preview = {
  sourcePath: "D:\\backup.bexo-workspaces.json",
  fileHash: "0".repeat(64),
  schemaVersion: 1,
  summary: {
    totalWorkspaceCount: 3,
    readyWorkspaceCount: 1,
    existingWorkspaceCount: 1,
    blockedWorkspaceCount: 1,
    missingProjectCount: 1,
    invalidProjectCount: 0,
  },
  warnings: [],
  items: [
    {
      workspaceIndex: 0,
      name: "new",
      suggestedName: "new",
      status: "ready",
      recommendedAction: "create",
      canCreate: true,
      canUpdate: false,
      projects: [],
      warnings: [],
    },
    {
      workspaceIndex: 1,
      name: "existing",
      suggestedName: "existing",
      status: "existing",
      recommendedAction: "skip",
      canCreate: false,
      canUpdate: true,
      projects: [],
      warnings: [],
    },
    {
      workspaceIndex: 2,
      name: "blocked",
      suggestedName: "blocked",
      status: "blocked",
      recommendedAction: "skip",
      canCreate: false,
      canUpdate: false,
      projects: [],
      warnings: [],
    },
  ],
};

test("workspace import actions keep valid user choices and repair invalid choices", () => {
  const actions = buildInitialWorkspaceImportActions(preview, {
    0: "skip",
    1: "update",
    2: "create",
  });
  assert.deepEqual(actions, { 0: "skip", 1: "update", 2: "skip" });
  const selections = buildWorkspaceImportSelections(preview, actions);
  assert.deepEqual(summarizeWorkspaceImportActions(selections), {
    create: 0,
    update: 1,
    skip: 2,
  });
});

test("workspace import list virtualization stays bounded for large files", () => {
  const range = getVirtualWorkspaceRange(500, 84 * 200, 352, 84, 4);
  assert.equal(range.totalHeight, 42_000);
  assert.equal(range.startIndex, 196);
  assert.ok(range.endIndex <= 210);
  assert.ok(range.endIndex < 500);
});

test("workspace transfer is wired through Rust commands, permissions, preview, and safe UX", () => {
  const sidebar = read("src/components/shell/section-sidebar.tsx");
  const dialog = read("src/features/workspaces/workspace-transfer-dialog.tsx");
  const client = read("src/lib/command-client.ts");
  const buildScript = read("src-tauri/build.rs");
  const handler = read("src-tauri/src/app/mod.rs");
  const permissions = read("src-tauri/permissions/window-commands.toml");
  const service = read("src-tauri/src/services/workspace_transfer_service.rs");

  assert.match(sidebar, /导入项目列表/);
  assert.match(sidebar, /导出项目列表/);
  assert.match(sidebar, /绝对目录路径、终端命令和参数/);
  assert.match(dialog, /重新定位/);
  assert.match(dialog, /更新会覆盖现有项目配置/);
  assert.match(dialog, /getVirtualWorkspaceRange/);
  for (const command of [
    "export_workspace_list",
    "preview_workspace_list_import",
    "apply_workspace_list_import",
  ]) {
    assert.match(client, new RegExp(`"${command}"`));
    assert.match(buildScript, new RegExp(`"${command}"`));
    assert.match(handler, new RegExp(command));
    assert.match(permissions, new RegExp(`allow-${command.replaceAll("_", "-")}`));
  }
  assert.match(service, /WORKSPACE_TRANSFER_FILE_CHANGED/);
  assert.match(service, /WORKSPACE_TRANSFER_MAX_FILE_BYTES/);
  assert.match(service, /MoveFileExW/);
});
