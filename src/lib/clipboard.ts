export async function copyTextToClipboard(text: string, emptyMessage = "复制内容为空") {
  const clipboardText = resolveClipboardPayload(text, emptyMessage);

  let clipboardError: unknown;
  if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(clipboardText);
      return;
    } catch (error) {
      clipboardError = error;
    }
  }

  if (typeof document === "undefined") {
    if (clipboardError instanceof Error && clipboardError.message.trim()) {
      throw clipboardError;
    }
    throw new Error("当前环境不支持剪贴板复制");
  }

  const textarea = document.createElement("textarea");
  textarea.value = clipboardText;
  textarea.readOnly = true;
  textarea.className = "allow-text-selection allow-context-menu";
  textarea.style.position = "fixed";
  textarea.style.left = "-9999px";
  textarea.style.top = "0";
  textarea.style.opacity = "0";
  document.body.appendChild(textarea);
  textarea.focus();
  textarea.select();

  try {
    const copied = document.execCommand("copy");
    if (!copied) {
      throw new Error("剪贴板写入失败");
    }
  } finally {
    document.body.removeChild(textarea);
  }
}

export function resolveClipboardPayload(text: string, emptyMessage = "复制内容为空") {
  if (!text.trim()) {
    throw new Error(emptyMessage);
  }
  return text;
}

export function getClipboardErrorMessage(error: unknown, fallback = "复制失败") {
  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }

  return fallback;
}
