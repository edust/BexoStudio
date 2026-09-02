import {
  applyPromptListImport,
  deletePrompt,
  exportPromptList,
  listPrompts,
  previewPromptListImport,
  reorderPrompts,
  upsertPrompt,
} from "@/lib/command-client";

export const promptsQueryKey = ["prompts"] as const;

export {
  applyPromptListImport,
  deletePrompt,
  exportPromptList,
  listPrompts,
  previewPromptListImport,
  reorderPrompts,
  upsertPrompt,
};
