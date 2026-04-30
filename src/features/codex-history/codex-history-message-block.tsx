import { CaretDownOutlined, CaretRightOutlined } from "@ant-design/icons";
import { Button, Tag, Typography } from "antd";
import { useEffect, useMemo, useState, type CSSProperties } from "react";

import { cn } from "@/lib/cn";
import type { CodexHistoryMessage, CodexHistoryViewPreferences } from "@/types/backend";

import { shouldCollapseCodexHistoryMessage } from "./codex-history-message-rules";
import { normalizeCodexHistoryViewPreferences } from "./codex-history-view-preferences";

const PREVIEW_LENGTH = 160;

type CodexHistoryMessageBlockProps = {
  message: CodexHistoryMessage;
  viewPreferences?: CodexHistoryViewPreferences;
};

export function CodexHistoryMessageBlock({
  message,
  viewPreferences,
}: CodexHistoryMessageBlockProps) {
  const defaultCollapsed = shouldCollapseCodexHistoryMessage(message);
  const [collapsed, setCollapsed] = useState(defaultCollapsed);
  const style = getMessageStyle(message);
  const typographyPreferences = normalizeCodexHistoryViewPreferences(viewPreferences);
  const normalContentStyle = useMemo(
    () => buildContentStyle(typographyPreferences, true),
    [typographyPreferences.messageFontFamily, typographyPreferences.messageFontSize],
  );
  const codeContentStyle = useMemo(
    () => buildContentStyle(typographyPreferences, false),
    [typographyPreferences.messageFontFamily, typographyPreferences.messageFontSize],
  );
  const preview = useMemo(() => buildPreview(message.content), [message.content]);
  const codeLike =
    message.itemType === "function_call" ||
    message.itemType === "function_call_output" ||
    message.role === "tool";

  useEffect(() => {
    setCollapsed(defaultCollapsed);
  }, [defaultCollapsed, message.content, message.itemType, message.role, message.timestamp]);

  return (
    <article
      className={cn(
        "codex-history-message allow-text-selection rounded-[8px] border px-4 transition-colors",
        collapsed ? "py-2.5" : "py-3",
        style.container,
      )}
    >
      <div className={cn("flex flex-wrap items-center gap-2", collapsed ? "mb-1" : "mb-2")}>
        <Tag bordered={false} className={cn("m-0 rounded-[6px] px-2 py-0", style.tag)}>
          {formatRole(message.role)}
        </Tag>
        {message.itemType !== "message" ? (
          <Tag bordered={false} className="m-0 rounded-[6px] bg-[#f2f4f7] px-2 py-0 text-[#475467]">
            {formatItemType(message.itemType)}
          </Tag>
        ) : null}
        {message.truncated ? (
          <Tag bordered={false} className="m-0 rounded-[6px] bg-[#fff7ed] px-2 py-0 text-[#b54708]">
            已截断
          </Tag>
        ) : null}
        <Typography.Text className="text-[11px] text-[#98a2b3]">
          {formatDateTime(message.timestamp)}
        </Typography.Text>
        {defaultCollapsed ? (
          <Button
            className="ml-auto !h-6 !px-2 !text-[11px]"
            icon={collapsed ? <CaretRightOutlined /> : <CaretDownOutlined />}
            onClick={() => setCollapsed((current) => !current)}
            size="small"
            type="text"
          >
            {collapsed ? "展开" : "收起"}
          </Button>
        ) : null}
      </div>

      {collapsed ? (
        <Typography.Text
          className={cn("codex-history-message__preview block", style.text)}
          style={normalContentStyle}
        >
          {preview}
        </Typography.Text>
      ) : codeLike ? (
        <pre
          className={cn(
            "codex-history-message__body m-0 whitespace-pre-wrap break-words font-mono",
            style.text,
          )}
          style={codeContentStyle}
        >
          {message.content}
        </pre>
      ) : (
        <div
          className={cn(
            "codex-history-message__body m-0 whitespace-pre-wrap break-words font-sans",
            style.text,
          )}
          style={normalContentStyle}
        >
          {message.content}
        </div>
      )}
    </article>
  );
}

function buildContentStyle(
  preferences: CodexHistoryViewPreferences,
  useCustomFontFamily: boolean,
): CSSProperties {
  const fontSize = preferences.messageFontSize;
  const lineHeight = Math.max(18, Math.round(fontSize * 1.65));
  const fontFamily =
    useCustomFontFamily && preferences.messageFontFamily
      ? preferences.messageFontFamily
      : undefined;

  return {
    fontFamily,
    fontSize,
    lineHeight: `${lineHeight}px`,
  };
}

function buildPreview(content: string) {
  const normalized = content.replace(/\s+/g, " ").trim();
  if (!normalized) {
    return "无内容";
  }
  if (normalized.length <= PREVIEW_LENGTH) {
    return normalized;
  }
  return `${normalized.slice(0, PREVIEW_LENGTH)}...`;
}

function getMessageStyle(message: CodexHistoryMessage) {
  if (message.role === "user") {
    return {
      container: "codex-history-message--user",
      tag: "codex-history-message-tag--user",
      text: "codex-history-message-text--user",
    };
  }
  if (message.role === "assistant") {
    return {
      container: "codex-history-message--assistant",
      tag: "codex-history-message-tag--assistant",
      text: "codex-history-message-text--assistant",
    };
  }
  if (message.role === "tool" || message.itemType.includes("function")) {
    return {
      container: "codex-history-message--tool",
      tag: "codex-history-message-tag--tool",
      text: "codex-history-message-text--tool",
    };
  }
  return {
    container: "codex-history-message--default",
    tag: "codex-history-message-tag--default",
    text: "codex-history-message-text--default",
  };
}

function formatRole(role: string) {
  switch (role) {
    case "assistant":
      return "Codex";
    case "user":
      return "用户";
    case "tool":
      return "工具";
    case "system":
      return "系统";
    default:
      return role || "消息";
  }
}

function formatItemType(itemType: string) {
  switch (itemType) {
    case "function_call":
      return "函数调用";
    case "function_call_output":
      return "函数输出";
    case "reasoning":
      return "推理";
    case "web_search_call":
      return "联网搜索";
    default:
      return itemType;
  }
}

function formatDateTime(value?: string | null) {
  if (!value) {
    return "--";
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return "--";
  }
  return date.toLocaleString();
}
