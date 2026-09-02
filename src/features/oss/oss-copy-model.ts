const MAX_OSS_OBJECT_KEY_BYTES = 1024;

export function buildOssCopyTargetKey(sourceKey: string) {
  const value = sourceKey.trim();
  const separator = value.lastIndexOf("/");
  const parent = separator >= 0 ? value.slice(0, separator + 1) : "";
  const name = separator >= 0 ? value.slice(separator + 1) : value;
  const extensionIndex = name.lastIndexOf(".");
  const hasExtension = extensionIndex > 0;
  const stem = hasExtension ? name.slice(0, extensionIndex) : name;
  const extension = hasExtension ? name.slice(extensionIndex) : "";
  return `${parent}${stem}-copy${extension}`;
}

export function validateOssCopyTargetKey(targetKey: string, sourceKey: string) {
  const value = targetKey.trim();
  if (!value) return "请输入目标 Object Key";
  if (value === sourceKey) return "目标 Object Key 不能与源对象相同";
  if (value.startsWith("/")) return "目标 Object Key 不能以 / 开头";
  if (value.endsWith("/")) return "目标 Object Key 不能以 / 结尾";
  if (new TextEncoder().encode(value).length > MAX_OSS_OBJECT_KEY_BYTES) {
    return `目标 Object Key 不能超过 ${MAX_OSS_OBJECT_KEY_BYTES} 字节`;
  }
  if ([...value].some((character) => character < " " || character === "\u007f")) {
    return "目标 Object Key 不能包含控制字符";
  }
  return null;
}
