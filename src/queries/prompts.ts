import {
  deletePrompt,
  listPrompts,
  reorderPrompts,
  upsertPrompt,
} from "@/lib/command-client";

export const promptsQueryKey = ["prompts"] as const;

export { deletePrompt, listPrompts, reorderPrompts, upsertPrompt };

