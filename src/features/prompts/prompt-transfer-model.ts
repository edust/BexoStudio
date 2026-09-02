import type {
  PromptImportAction,
  PromptImportItemPreview,
  PromptImportPreview,
  PromptImportSelection,
} from "@/types/backend";

export type PromptImportActionMap = Record<number, PromptImportAction>;

export type PromptImportActionSummary = {
  create: number;
  update: number;
  skip: number;
};

export type VirtualPromptImportRange = {
  startIndex: number;
  endIndex: number;
  offsetTop: number;
  totalHeight: number;
};

export function buildInitialPromptImportActions(
  preview: PromptImportPreview,
  previous: PromptImportActionMap = {},
): PromptImportActionMap {
  const actions: PromptImportActionMap = {};
  for (const item of preview.items) {
    const previousAction = previous[item.promptIndex];
    actions[item.promptIndex] = isPromptImportActionAllowed(item, previousAction)
      ? previousAction
      : item.recommendedAction;
  }
  return actions;
}

export function isPromptImportActionAllowed(
  item: PromptImportItemPreview,
  action: PromptImportAction | undefined,
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

export function buildPromptImportSelections(
  preview: PromptImportPreview,
  actions: PromptImportActionMap,
): PromptImportSelection[] {
  return preview.items.map((item) => ({
    promptIndex: item.promptIndex,
    action: isPromptImportActionAllowed(item, actions[item.promptIndex])
      ? actions[item.promptIndex]
      : item.recommendedAction,
  }));
}

export function summarizePromptImportActions(
  selections: PromptImportSelection[],
): PromptImportActionSummary {
  return selections.reduce<PromptImportActionSummary>(
    (summary, selection) => {
      summary[selection.action] += 1;
      return summary;
    },
    { create: 0, update: 0, skip: 0 },
  );
}

export function getVirtualPromptImportRange(
  itemCount: number,
  scrollTop: number,
  viewportHeight: number,
  rowHeight: number,
  overscan = 5,
): VirtualPromptImportRange {
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

export function buildPromptExportFilename(now = new Date()) {
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `bexo-prompts-${year}-${month}-${day}.bexo-prompts.json`;
}
