export const MAX_WORKSPACE_DESCRIPTION_CHARS = 200;

export type WorkspaceSearchableItem = {
  label: string;
  description: string;
  badge?: string;
  note?: string;
};

export function countUnicodeCharacters(value: string) {
  return Array.from(value).length;
}

export function validateWorkspaceDescription(value: string) {
  if (countUnicodeCharacters(value) > MAX_WORKSPACE_DESCRIPTION_CHARS) {
    return `备注不能超过 ${MAX_WORKSPACE_DESCRIPTION_CHARS} 个字符`;
  }

  const hasUnsupportedControlCharacter = Array.from(value).some((character) => {
    const codePoint = character.codePointAt(0) ?? 0;
    const isControlCharacter =
      codePoint <= 0x1f || (codePoint >= 0x7f && codePoint <= 0x9f);
    return isControlCharacter && ![0x09, 0x0a, 0x0d].includes(codePoint);
  });

  return hasUnsupportedControlCharacter ? "备注不能包含不支持的控制字符" : null;
}

export function filterWorkspaceItems<T extends WorkspaceSearchableItem>(items: T[], query: string) {
  const normalizedQuery = query.trim().toLocaleLowerCase();
  if (!normalizedQuery) {
    return items;
  }

  return items.filter((item) => {
    const searchableText = [item.label, item.description, item.badge ?? "", item.note ?? ""]
      .join(" ")
      .toLocaleLowerCase();
    return searchableText.includes(normalizedQuery);
  });
}
