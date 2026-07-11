import type { PromptRecord, UpsertPromptPayload } from "@/types/backend";

export const MAX_PROMPT_TITLE_CHARS = 120;
export const MAX_PROMPT_CONTENT_CHARS = 100_000;

export type PromptDraft = {
  id: string;
  title: string;
  content: string;
  isNew: boolean;
};

export type PromptDraftErrors = {
  title?: string;
  content?: string;
};

export type PromptDraftValidation =
  | { ok: true; input: UpsertPromptPayload; errors: PromptDraftErrors }
  | { ok: false; errors: PromptDraftErrors };

export function createPromptDraftId() {
  const cryptoApi = globalThis.crypto;
  if (typeof cryptoApi?.randomUUID === "function") {
    return cryptoApi.randomUUID();
  }
  if (typeof cryptoApi?.getRandomValues !== "function") {
    throw new Error("当前环境无法生成 Prompt 标识");
  }

  const bytes = new Uint8Array(16);
  cryptoApi.getRandomValues(bytes);
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = Array.from(bytes, (value) => value.toString(16).padStart(2, "0"));
  return `${hex.slice(0, 4).join("")}-${hex.slice(4, 6).join("")}-${hex
    .slice(6, 8)
    .join("")}-${hex.slice(8, 10).join("")}-${hex.slice(10).join("")}`;
}

export function buildEmptyPromptDraft(): PromptDraft {
  return {
    id: createPromptDraftId(),
    title: "",
    content: "",
    isNew: true,
  };
}

export function promptRecordToDraft(prompt: PromptRecord): PromptDraft {
  return {
    id: prompt.id,
    title: prompt.title,
    content: prompt.content,
    isNew: false,
  };
}

export function validatePromptDraft(draft: PromptDraft): PromptDraftValidation {
  const title = draft.title.trim();
  const errors: PromptDraftErrors = {};
  if (!title) {
    errors.title = "请输入 Prompt 标题";
  } else if (countPromptCharacters(title) > MAX_PROMPT_TITLE_CHARS) {
    errors.title = `标题不能超过 ${MAX_PROMPT_TITLE_CHARS} 个字符`;
  } else if (containsControlCharacter(title)) {
    errors.title = "标题不能包含换行或控制字符";
  }

  if (!draft.content.trim()) {
    errors.content = "请输入 Prompt 内容";
  } else if (countPromptCharacters(draft.content) > MAX_PROMPT_CONTENT_CHARS) {
    errors.content = `Prompt 内容不能超过 ${MAX_PROMPT_CONTENT_CHARS.toLocaleString()} 个字符`;
  } else if (draft.content.includes("\0")) {
    errors.content = "Prompt 内容不能包含 NUL 字符";
  }

  if (errors.title || errors.content) {
    return { ok: false, errors };
  }

  return {
    ok: true,
    errors,
    input: {
      id: draft.id,
      title,
      content: draft.content,
    },
  };
}

export function isPromptDraftDirty(draft: PromptDraft | null, source: PromptRecord | null) {
  if (!draft) {
    return false;
  }
  if (draft.isNew) {
    return Boolean(draft.title.trim() || draft.content.trim());
  }
  if (!source || source.id !== draft.id) {
    return true;
  }
  return draft.title !== source.title || draft.content !== source.content;
}

export function filterPrompts(prompts: PromptRecord[], query: string) {
  const normalized = query.trim().toLocaleLowerCase();
  if (!normalized) {
    return prompts;
  }
  return prompts.filter((prompt) =>
    `${prompt.title}\n${prompt.content}`.toLocaleLowerCase().includes(normalized),
  );
}

export function movePromptBeforeTarget(
  prompts: PromptRecord[],
  draggingId: string,
  targetId: string,
) {
  if (draggingId === targetId) {
    return null;
  }
  const fromIndex = prompts.findIndex((prompt) => prompt.id === draggingId);
  const targetIndex = prompts.findIndex((prompt) => prompt.id === targetId);
  if (fromIndex < 0 || targetIndex < 0) {
    return null;
  }

  const next = [...prompts];
  const [dragging] = next.splice(fromIndex, 1);
  const targetIndexAfterRemoval = next.findIndex((prompt) => prompt.id === targetId);
  if (targetIndexAfterRemoval < 0) {
    return null;
  }
  const insertionIndex =
    fromIndex < targetIndex ? targetIndexAfterRemoval + 1 : targetIndexAfterRemoval;
  next.splice(insertionIndex, 0, dragging);
  return next;
}

export function movePromptByOffset(
  prompts: PromptRecord[],
  promptId: string,
  offset: -1 | 1,
) {
  const index = prompts.findIndex((prompt) => prompt.id === promptId);
  const targetIndex = index + offset;
  if (index < 0 || targetIndex < 0 || targetIndex >= prompts.length) {
    return null;
  }
  const next = [...prompts];
  [next[index], next[targetIndex]] = [next[targetIndex], next[index]];
  return next;
}

export function orderPromptsByIds(prompts: PromptRecord[], promptIds: string[]) {
  if (prompts.length !== promptIds.length) {
    return null;
  }

  const promptById = new Map(prompts.map((prompt) => [prompt.id, prompt] as const));
  if (promptById.size !== prompts.length) {
    return null;
  }

  const seen = new Set<string>();
  const ordered: PromptRecord[] = [];
  for (const promptId of promptIds) {
    if (seen.has(promptId)) {
      return null;
    }
    const prompt = promptById.get(promptId);
    if (!prompt) {
      return null;
    }
    seen.add(promptId);
    ordered.push(prompt);
  }
  return ordered;
}

export function isSamePromptOrder(previousIds: string[], prompts: PromptRecord[]) {
  return (
    previousIds.length === prompts.length &&
    previousIds.every((promptId, index) => promptId === prompts[index]?.id)
  );
}

export function buildPromptPreview(content: string, maxLength = 88) {
  const normalized = content.replace(/\s+/g, " ").trim();
  if (normalized.length <= maxLength) {
    return normalized;
  }
  return `${normalized.slice(0, Math.max(1, maxLength - 1))}…`;
}

export function countPromptCharacters(value: string) {
  return Array.from(value).length;
}

function containsControlCharacter(value: string) {
  return Array.from(value).some((character) => {
    const codePoint = character.codePointAt(0) ?? 0;
    return codePoint <= 0x1f || (codePoint >= 0x7f && codePoint <= 0x9f);
  });
}
