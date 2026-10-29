export type NavigableWorkspace = {
  workspace: { id: string; isArchived: boolean };
};

export function workspaceScope<T extends NavigableWorkspace>(items: T[], archived: boolean) {
  return items.filter((item) => item.workspace.isArchived === archived);
}

// Only reorder the matching scope/group slots; hidden and archived rows keep their positions.
export function moveWorkspaceWithinGroup<T extends NavigableWorkspace>(
  items: T[], sourceId: string, targetId: string, pinnedIds: string[],
) {
  const source = items.find((item) => item.workspace.id === sourceId);
  const target = items.find((item) => item.workspace.id === targetId);
  const pinned = new Set(pinnedIds);
  if (!source || !target || source === target ||
    source.workspace.isArchived !== target.workspace.isArchived ||
    pinned.has(sourceId) !== pinned.has(targetId)) return items;
  const group = items.filter((item) =>
    item.workspace.isArchived === source.workspace.isArchived &&
    pinned.has(item.workspace.id) === pinned.has(sourceId));
  const from = group.indexOf(source);
  const to = group.indexOf(target);
  group.splice(from, 1);
  group.splice(to, 0, source);
  let index = 0;
  return items.map((item) =>
    item.workspace.isArchived === source.workspace.isArchived &&
    pinned.has(item.workspace.id) === pinned.has(sourceId) ? group[index++] : item);
}

export const WORKSPACE_ROW_HEIGHT = 52;

export function visibleWorkspaceRows<T extends { top: number; height: number }>(rows: T[], total: number, scrollTop: number, height: number) {
  const maxScroll = Math.max(0, total - height);
  const top = Math.min(maxScroll, Math.max(0, scrollTop));
  return rows.filter((row) => row.top + row.height > top - 256 && row.top < top + height + 256);
}
