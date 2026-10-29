import assert from "node:assert/strict";
import test from "node:test";
import { workspaceSidebarWidth, splitWorkspacePath } from "../../src/components/shell/workspace-sidebar-width.ts";
import { WORKSPACE_ROW_HEIGHT, visibleWorkspaceRows } from "../../src/features/workspaces/workspace-navigation-model.ts";

test("workspace width limits preserve the three panes and recover saved width on larger windows", () => {
  assert.equal(workspaceSidebarWidth(NaN, 1920), 320);
  assert.equal(workspaceSidebarWidth(440, 1920), 440);
  assert.equal(workspaceSidebarWidth(9999, 1280), 520);
  assert.equal(workspaceSidebarWidth(600, 1100), 428);
  assert.equal(workspaceSidebarWidth(520, 1920), 520);
  assert.equal(workspaceSidebarWidth(-10, 1920), 260);
});

test("path presentation preserves original root, final directory and separators", () => {
  for (const path of ["D:\\Desktop\\a\\StarExpoHub", "\\\\server\\share\\项目", "/home/user/project", "D:\\", "项目", ""]) {
    const {prefix, leaf} = splitWorkspacePath(path);
    assert.equal(prefix + leaf, path);
  }
  assert.deepEqual(splitWorkspacePath("D:/Desktop/project"), {prefix: "D:/Desktop/", leaf: "project"});
});

test("two-line virtual rows reach the last item without stale row-height offsets", () => {
  assert.equal(WORKSPACE_ROW_HEIGHT, 52);
  const rows = Array.from({length: 1000}, (_, i) => ({top: i*WORKSPACE_ROW_HEIGHT, height: WORKSPACE_ROW_HEIGHT}));
  const last = visibleWorkspaceRows(rows, 1000*WORKSPACE_ROW_HEIGHT, 1000*WORKSPACE_ROW_HEIGHT, 700);
  assert.equal(last.at(-1), rows.at(-1));
  assert.ok(last.length < 30);
});
