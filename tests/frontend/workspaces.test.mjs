import assert from "node:assert/strict";
import test from "node:test";

import {
  countUnicodeCharacters,
  filterWorkspaceItems,
  MAX_WORKSPACE_DESCRIPTION_CHARS,
  validateWorkspaceDescription,
} from "../../src/features/workspaces/workspace-model.ts";

const workspaces = [
  {
    label: "StarExpoHub",
    description: "D:\\Desktop\\a\\StarExpoHub",
    note: "活动报名系统，后端 API 和管理端",
  },
  {
    label: "go-x",
    description: "D:\\Desktop\\go\\go-x",
    note: "行情数据实验项目",
  },
];

test("workspace search covers name, path, and note without changing order", () => {
  assert.deepEqual(filterWorkspaceItems(workspaces, "StarExpoHub"), [workspaces[0]]);
  assert.deepEqual(filterWorkspaceItems(workspaces, "go\\go-x"), [workspaces[1]]);
  assert.deepEqual(filterWorkspaceItems(workspaces, "行情"), [workspaces[1]]);
  assert.deepEqual(filterWorkspaceItems(workspaces, ""), workspaces);
});

test("workspace description length counts Unicode characters", () => {
  assert.equal(countUnicodeCharacters("项目🚀"), 3);
  assert.equal(countUnicodeCharacters("记".repeat(MAX_WORKSPACE_DESCRIPTION_CHARS)), 200);
  assert.equal(countUnicodeCharacters("记".repeat(MAX_WORKSPACE_DESCRIPTION_CHARS + 1)), 201);
});

test("workspace description validation rejects unsupported controls", () => {
  assert.equal(validateWorkspaceDescription("第一行\n第二行\t"), null);
  assert.equal(validateWorkspaceDescription("不可见\u0007"), "备注不能包含不支持的控制字符");
  assert.equal(
    validateWorkspaceDescription("记".repeat(MAX_WORKSPACE_DESCRIPTION_CHARS + 1)),
    "备注不能超过 200 个字符",
  );
});
