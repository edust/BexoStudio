import type {
  WorkspaceImportAction,
  WorkspaceImportItemPreview,
  WorkspaceImportPreview,
  WorkspaceImportSelection,
} from "@/types/backend";

export type WorkspaceImportActionMap = Record<number, WorkspaceImportAction>;

export type WorkspaceImportActionSummary = {
  create: number;
  update: number;
  skip: number;
};

export type VirtualWorkspaceRange = {
  startIndex: number;
  endIndex: number;
  offsetTop: number;
  totalHeight: number;
};

export function buildInitialWorkspaceImportActions(
  preview: WorkspaceImportPreview,
  previous: WorkspaceImportActionMap = {},
): WorkspaceImportActionMap {
  const actions: WorkspaceImportActionMap = {};
  for (const item of preview.items) {
    const previousAction = previous[item.workspaceIndex];
    actions[item.workspaceIndex] = isWorkspaceImportActionAllowed(item, previousAction)
      ? previousAction
      : item.recommendedAction;
  }
  return actions;
}

export function isWorkspaceImportActionAllowed(
  item: WorkspaceImportItemPreview,
  action: WorkspaceImportAction | undefined,
) {
  if (action === "skip") {
    return true;
  }
  if (action === "create") {
    return item.canCreate;
  }
  if (action === "update") {
    return item.canUpdate;
  }
  return false;
}

export function buildWorkspaceImportSelections(
  preview: WorkspaceImportPreview,
  actions: WorkspaceImportActionMap,
): WorkspaceImportSelection[] {
  return preview.items.map((item) => ({
    workspaceIndex: item.workspaceIndex,
    action: isWorkspaceImportActionAllowed(item, actions[item.workspaceIndex])
      ? actions[item.workspaceIndex]
      : item.recommendedAction,
  }));
}

export function summarizeWorkspaceImportActions(
  selections: WorkspaceImportSelection[],
): WorkspaceImportActionSummary {
  return selections.reduce<WorkspaceImportActionSummary>(
    (summary, selection) => {
      summary[selection.action] += 1;
      return summary;
    },
    { create: 0, update: 0, skip: 0 },
  );
}

export function getVirtualWorkspaceRange(
  itemCount: number,
  scrollTop: number,
  viewportHeight: number,
  rowHeight: number,
  overscan = 4,
): VirtualWorkspaceRange {
  if (itemCount <= 0 || rowHeight <= 0 || viewportHeight <= 0) {
    return { startIndex: 0, endIndex: 0, offsetTop: 0, totalHeight: 0 };
  }
  const safeScrollTop = Math.max(0, scrollTop);
  const visibleStart = Math.floor(safeScrollTop / rowHeight);
  const visibleCount = Math.ceil(viewportHeight / rowHeight);
  const startIndex = Math.max(0, visibleStart - overscan);
  const endIndex = Math.min(itemCount, visibleStart + visibleCount + overscan);
  return {
    startIndex,
    endIndex,
    offsetTop: startIndex * rowHeight,
    totalHeight: itemCount * rowHeight,
  };
}
