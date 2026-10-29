import assert from "node:assert/strict";
import test from "node:test";
import { workspaceScope, moveWorkspaceWithinGroup, visibleWorkspaceRows } from "../../src/features/workspaces/workspace-navigation-model.ts";

const item = (id, isArchived = false) => ({ workspace: { id, isArchived } });
const ids = (items) => items.map((item) => item.workspace.id);

test("archive scope preserves order and never mixes archived and active records", () => {
  const items = [item("a"), item("old", true), item("b")];
  assert.deepEqual(ids(workspaceScope(items, false)), ["a", "b"]);
  assert.deepEqual(ids(workspaceScope(items, true)), ["old"]);
});

test("manual reorder preserves hidden archive and pinned slots without mutating source", () => {
  const items = [item("pin"), item("a"), item("old", true), item("b"), item("c")];
  assert.deepEqual(ids(moveWorkspaceWithinGroup(items, "a", "c", ["pin"])), ["pin", "b", "old", "c", "a"]);
  assert.deepEqual(ids(moveWorkspaceWithinGroup(items, "c", "a", ["pin"])), ["pin", "c", "old", "a", "b"]);
  assert.deepEqual(ids(items), ["pin", "a", "old", "b", "c"]);
  assert.equal(moveWorkspaceWithinGroup(items, "a", "pin", ["pin"]), items);
  assert.equal(moveWorkspaceWithinGroup(items, "a", "old", []), items);
  assert.equal(moveWorkspaceWithinGroup(items, "missing", "a", []), items);
});

test("virtual list bounds mounted rows for 10000 projects and clamps scroll after filtering", () => {
  const rows = Array.from({ length: 10000 }, (_, index) => ({ top: index * 64, height: 64, index }));
  const middle = visibleWorkspaceRows(rows, 640000, 320000, 600);
  assert.ok(middle.length < 20);
  assert.ok(middle.some((row) => row.index === 5000));
  assert.deepEqual(visibleWorkspaceRows(rows.slice(0, 2), 128, 320000, 600), rows.slice(0, 2));
  assert.deepEqual(visibleWorkspaceRows([], 0, 320000, 600), []);
});
