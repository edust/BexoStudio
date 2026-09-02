export const MAX_OSS_FOLDER_NAME_CHARS = 254;

export function normalizeOssFolderName(value: string) {
  return value.trim();
}

export function validateOssFolderName(value: string): string | null {
  const normalized = normalizeOssFolderName(value);
  if (!normalized) {
    return "请输入文件夹名称";
  }
  if ([...normalized].length > MAX_OSS_FOLDER_NAME_CHARS) {
    return `文件夹名称不能超过 ${MAX_OSS_FOLDER_NAME_CHARS} 个字符`;
  }
  if (normalized.includes("/") || normalized.includes("\\")) {
    return "文件夹名称不能包含斜杠或反斜杠";
  }
  if (normalized === "." || normalized === "..") {
    return "文件夹名称不能为 . 或 ..";
  }
  if ([...normalized].some(isControlCharacter)) {
    return "文件夹名称不能包含控制字符";
  }
  return null;
}

export function buildOssFolderKey(parentPrefix: string, name: string) {
  const normalizedPrefix = parentPrefix.trim();
  const prefix = normalizedPrefix && !normalizedPrefix.endsWith("/")
    ? `${normalizedPrefix}/`
    : normalizedPrefix;
  return `${prefix}${normalizeOssFolderName(name)}/`;
}

function isControlCharacter(value: string) {
  const codePoint = value.codePointAt(0) ?? 0;
  return (codePoint >= 0 && codePoint <= 0x1f) || (codePoint >= 0x7f && codePoint <= 0x9f);
}
