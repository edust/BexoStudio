import { ReloadOutlined, SearchOutlined } from "@ant-design/icons";
import { useQuery } from "@tanstack/react-query";
import { Alert, Button, Checkbox, Empty, Input, Spin, Tooltip, Typography } from "antd";
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { toast } from "sonner";

import { PaneHeader } from "@/components/shell/pane-header";
import { CodexHistoryMessageBlock } from "@/features/codex-history/codex-history-message-block";
import { CODEX_HISTORY_SESSION_ROW_HEIGHT, CodexHistorySessionItem } from "@/features/codex-history/codex-history-session-item";
import { filterCodexHistoryMessages } from "@/features/codex-history/codex-history-message-rules";
import { useCodexHistoryViewPreferences } from "@/features/codex-history/codex-history-view-preferences";
import { cn } from "@/lib/cn";
import {
  getCodexHistoryMessages,
  getErrorSummary,
  hasDesktopRuntime,
  listCodexHistorySessions,
} from "@/lib/command-client";
import type { CodexHistoryMessage, CodexHistorySession } from "@/types/backend";

const MESSAGE_PAGE_SIZE = 40;
const SESSION_ROW_HEIGHT = CODEX_HISTORY_SESSION_ROW_HEIGHT;
const SESSION_OVERSCAN = 6;

type ScrollRestoreState = {
  previousHeight: number;
  previousTop: number;
};

export default function CodexHistoryWindowPage() {
  const desktopRuntimeAvailable = hasDesktopRuntime();
  const workspaceId = useMemo(() => {
    if (typeof window === "undefined") {
      return null;
    }
    const value = new URLSearchParams(window.location.search).get("workspaceId");
    return value?.trim() ? value.trim() : null;
  }, []);
  const [query, setQuery] = useState("");
  const [selectedSourcePath, setSelectedSourcePath] = useState<string | null>(null);
  const [messages, setMessages] = useState<CodexHistoryMessage[]>([]);
  const [olderCursor, setOlderCursor] = useState<string | null>(null);
  const [hasMoreMessages, setHasMoreMessages] = useState(false);
  const [messagesLoading, setMessagesLoading] = useState(false);
  const [loadingOlder, setLoadingOlder] = useState(false);
  const [messageError, setMessageError] = useState<string | null>(null);
  const [messageReloadNonce, setMessageReloadNonce] = useState(0);
  const [showTechnicalMessages, setShowTechnicalMessages] = useState(false);
  const [initialScrollPending, setInitialScrollPending] = useState(false);
  const [sessionScrollTop, setSessionScrollTop] = useState(0);
  const [sessionViewportHeight, setSessionViewportHeight] = useState(0);
  const messagesScrollRef = useRef<HTMLDivElement | null>(null);
  const sessionListRef = useRef<HTMLDivElement | null>(null);
  const scrollRestoreRef = useRef<ScrollRestoreState | null>(null);
  const loadingOlderRef = useRef(false);
  const messageRequestIdRef = useRef(0);
  const codexHistoryViewPreferences =
    useCodexHistoryViewPreferences(desktopRuntimeAvailable);

  const sessionsQuery = useQuery({
    queryKey: ["codex-history-sessions", workspaceId],
    queryFn: () => listCodexHistorySessions(workspaceId ?? ""),
    enabled: desktopRuntimeAvailable && Boolean(workspaceId),
    staleTime: 15_000,
  });

  const filteredSessions = useMemo(() => {
    const sessions = sessionsQuery.data?.sessions ?? [];
    const normalized = query.trim().toLowerCase();
    if (!normalized) {
      return sessions;
    }

    return sessions.filter((session) => {
      const searchable = [
        session.title,
        session.summary,
        session.projectDir,
        session.sessionId,
        session.sourcePath,
      ]
        .filter(Boolean)
        .join(" ")
        .toLowerCase();
      return searchable.includes(normalized);
    });
  }, [query, sessionsQuery.data?.sessions]);

  const selectedSession = useMemo(() => {
    if (!selectedSourcePath) {
      return null;
    }
    return filteredSessions.find((session) => session.sourcePath === selectedSourcePath) ?? null;
  }, [filteredSessions, selectedSourcePath]);

  const sessionVisibleRange = useMemo(() => {
    const viewportHeight = sessionViewportHeight || 520;
    const start = Math.max(
      0,
      Math.floor(sessionScrollTop / SESSION_ROW_HEIGHT) - SESSION_OVERSCAN,
    );
    const end = Math.min(
      filteredSessions.length,
      Math.ceil((sessionScrollTop + viewportHeight) / SESSION_ROW_HEIGHT) + SESSION_OVERSCAN,
    );
    return { start, end };
  }, [filteredSessions.length, sessionScrollTop, sessionViewportHeight]);

  const visibleSessions = useMemo(
    () => filteredSessions.slice(sessionVisibleRange.start, sessionVisibleRange.end),
    [filteredSessions, sessionVisibleRange.end, sessionVisibleRange.start],
  );

  const visibleMessages = useMemo(
    () => filterCodexHistoryMessages(messages, showTechnicalMessages),
    [messages, showTechnicalMessages],
  );
  const hiddenMessageCount = messages.length - visibleMessages.length;

  useEffect(() => {
    const element = sessionListRef.current;
    if (!element) {
      return;
    }

    const updateViewportHeight = () => {
      setSessionViewportHeight(element.clientHeight);
    };
    updateViewportHeight();

    const observer = new ResizeObserver(updateViewportHeight);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const stillVisible =
      selectedSourcePath &&
      filteredSessions.some((session) => session.sourcePath === selectedSourcePath);
    if (stillVisible) {
      return;
    }

    setSelectedSourcePath(filteredSessions[0]?.sourcePath ?? null);
  }, [filteredSessions, selectedSourcePath]);

  useEffect(() => {
    if (!selectedSession || !workspaceId || !desktopRuntimeAvailable) {
      setMessages([]);
      setOlderCursor(null);
      setHasMoreMessages(false);
      setMessageError(null);
      setMessagesLoading(false);
      setInitialScrollPending(false);
      return;
    }

    const requestId = messageRequestIdRef.current + 1;
    messageRequestIdRef.current = requestId;
    setMessages([]);
    setOlderCursor(null);
    setHasMoreMessages(false);
    setMessageError(null);
    setMessagesLoading(true);
    setInitialScrollPending(false);

    getCodexHistoryMessages({
      workspaceId,
      sourcePath: selectedSession.sourcePath,
      limit: MESSAGE_PAGE_SIZE,
    })
      .then((page) => {
        if (messageRequestIdRef.current !== requestId) {
          return;
        }
        setMessages(page.messages);
        setOlderCursor(page.olderCursor ?? null);
        setHasMoreMessages(page.hasMore);
        setInitialScrollPending(true);
      })
      .catch((error) => {
        if (messageRequestIdRef.current !== requestId) {
          return;
        }
        setMessageError(getErrorSummary(error).message);
      })
      .finally(() => {
        if (messageRequestIdRef.current === requestId) {
          setMessagesLoading(false);
        }
      });
  }, [desktopRuntimeAvailable, messageReloadNonce, selectedSession, workspaceId]);

  useEffect(() => {
    if (!initialScrollPending) {
      return;
    }
    const element = messagesScrollRef.current;
    if (!element) {
      return;
    }

    window.requestAnimationFrame(() => {
      element.scrollTop = element.scrollHeight;
      setInitialScrollPending(false);
    });
  }, [initialScrollPending, messages.length]);

  useEffect(() => {
    const restoreState = scrollRestoreRef.current;
    if (!restoreState) {
      return;
    }
    const element = messagesScrollRef.current;
    if (!element) {
      scrollRestoreRef.current = null;
      return;
    }

    window.requestAnimationFrame(() => {
      element.scrollTop =
        element.scrollHeight - restoreState.previousHeight + restoreState.previousTop;
      scrollRestoreRef.current = null;
    });
  }, [messages.length]);

  const loadOlderMessages = useCallback(async () => {
    if (
      !workspaceId ||
      !selectedSession ||
      !olderCursor ||
      !hasMoreMessages ||
      messagesLoading ||
      loadingOlderRef.current
    ) {
      return;
    }

    const scrollElement = messagesScrollRef.current;
    scrollRestoreRef.current = scrollElement
      ? {
          previousHeight: scrollElement.scrollHeight,
          previousTop: scrollElement.scrollTop,
        }
      : null;

    loadingOlderRef.current = true;
    setLoadingOlder(true);
    setMessageError(null);

    try {
      const page = await getCodexHistoryMessages({
        workspaceId,
        sourcePath: selectedSession.sourcePath,
        cursor: olderCursor,
        limit: MESSAGE_PAGE_SIZE,
      });
      setMessages((current) => [...page.messages, ...current]);
      setOlderCursor(page.olderCursor ?? null);
      setHasMoreMessages(page.hasMore);
    } catch (error) {
      scrollRestoreRef.current = null;
      setMessageError(getErrorSummary(error).message);
    } finally {
      loadingOlderRef.current = false;
      setLoadingOlder(false);
    }
  }, [hasMoreMessages, messagesLoading, olderCursor, selectedSession, workspaceId]);

  const handleMessagesScroll = useCallback(() => {
    const element = messagesScrollRef.current;
    if (!element || element.scrollTop > 96) {
      return;
    }
    void loadOlderMessages();
  }, [loadOlderMessages]);

  const handleRetryMessages = useCallback(() => {
    if (messages.length && hasMoreMessages && olderCursor) {
      void loadOlderMessages();
      return;
    }
    setMessageReloadNonce((current) => current + 1);
  }, [hasMoreMessages, loadOlderMessages, messages.length, olderCursor]);

  const handleRefreshSessions = useCallback(async () => {
    try {
      await sessionsQuery.refetch();
    } catch (error) {
      toast.error(getErrorSummary(error).message);
    }
  }, [sessionsQuery]);

  const rootLabel = sessionsQuery.data?.workspacePath ?? "未解析工作区路径";
  const sessionsTotal = sessionsQuery.data?.sessions.length ?? 0;

  if (!desktopRuntimeAvailable) {
    return (
      <WindowStateShell>
        <Alert message="当前页面需要在 Bexo Studio 桌面应用中打开" showIcon type="error" />
      </WindowStateShell>
    );
  }

  if (!workspaceId) {
    return (
      <WindowStateShell>
        <Alert message="缺少工作区参数，无法读取 Codex 历史" showIcon type="error" />
      </WindowStateShell>
    );
  }

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-surface text-fg">
      <aside className="flex w-[320px] shrink-0 flex-col border-r border-line bg-surface">
        <PaneHeader
          actions={
            <Tooltip title="刷新">
              <Button
                aria-label="刷新会话列表"
                icon={<ReloadOutlined />}
                loading={sessionsQuery.isFetching}
                onClick={() => void handleRefreshSessions()}
                size="small"
                type="text"
              />
            </Tooltip>
          }
          meta={query.trim() ? `${filteredSessions.length}/${sessionsTotal}` : sessionsTotal}
          title="Codex 历史"
        />
        <div className="shrink-0 space-y-1 border-b border-line px-2 py-2">
          <div className="truncate px-1 text-[12px] text-fg-3" title={rootLabel}>{rootLabel}</div>
          <Input
            allowClear
            aria-label="搜索会话"
            onChange={(event) => {
              setQuery(event.target.value);
              setSessionScrollTop(0);
              if (sessionListRef.current) {
                sessionListRef.current.scrollTop = 0;
              }
            }}
            placeholder="搜索会话"
            prefix={<SearchOutlined className="text-fg-3" />}
            value={query}
          />
        </div>
        <div className="min-h-0 flex-1">
          {sessionsQuery.isLoading ? (
            <CenteredState>
              <Spin />
            </CenteredState>
          ) : sessionsQuery.isError ? (
            <div className="p-4">
              <Alert
                message={getErrorSummary(sessionsQuery.error).message}
                showIcon
                type="error"
              />
            </div>
          ) : !filteredSessions.length ? (
            <CenteredState>
              <Empty description="没有匹配的 Codex 会话" image={Empty.PRESENTED_IMAGE_SIMPLE} />
            </CenteredState>
          ) : (
            <div
              className="h-full overflow-y-auto p-1"
              onScroll={(event) => setSessionScrollTop(event.currentTarget.scrollTop)}
              ref={sessionListRef}
            >
              <div
                className="relative"
                style={{ height: filteredSessions.length * SESSION_ROW_HEIGHT }}
              >
                {visibleSessions.map((session, index) => {
                  const absoluteIndex = sessionVisibleRange.start + index;
                  return (
                    <CodexHistorySessionItem
                      key={session.sourcePath}
                      active={selectedSourcePath === session.sourcePath}
                      dateLabel={formatShortDate(session.lastActiveAt ?? session.createdAt)}
                      onSelect={() => setSelectedSourcePath(session.sourcePath)}
                      secondary={session.summary || (session.projectDir ?? session.sourcePath)}
                      title={getSessionTitle(session)}
                      tooltip={`${getSessionTitle(session)}\n${session.projectDir ?? session.sourcePath}`}
                      style={{
                        height: SESSION_ROW_HEIGHT,
                        top: absoluteIndex * SESSION_ROW_HEIGHT,
                      }}
                    />
                  );
                })}
              </div>
            </div>
          )}
        </div>
      </aside>

      <main className="flex min-h-0 min-w-0 flex-1 flex-col">
        <PaneHeader
          actions={
            <Checkbox
              checked={showTechnicalMessages}
              className="text-[12px]"
              onChange={(event) => setShowTechnicalMessages(event.target.checked)}
            >
              显示技术项
            </Checkbox>
          }
          meta={
            selectedSession ? (
              <span title={selectedSession.sourcePath}>
                {formatDateTime(selectedSession.lastActiveAt ?? selectedSession.createdAt)}
                <span className="mx-2">·</span>
                {formatBytes(selectedSession.fileSizeBytes)}
              </span>
            ) : null
          }
          title={selectedSession ? getSessionTitle(selectedSession) : "未选择会话"}
        />
        <div
          className="min-h-0 flex-1 overflow-y-auto px-4 py-3"
          onScroll={handleMessagesScroll}
          ref={messagesScrollRef}
        >
          {!selectedSession ? (
            <CenteredState>
              <Empty description="未选择 Codex 会话" image={Empty.PRESENTED_IMAGE_SIMPLE} />
            </CenteredState>
          ) : messagesLoading ? (
            <CenteredState>
              <Spin />
            </CenteredState>
          ) : (
            <div className="allow-text-selection flex w-full flex-col gap-3">
              {hasMoreMessages ? (
                <div className="flex justify-center py-1">
                  <Button
                    loading={loadingOlder}
                    onClick={() => void loadOlderMessages()}
                    size="small"
                  >
                    加载更早消息
                  </Button>
                </div>
              ) : messages.length ? (
                <Typography.Text className="self-center text-[11px] text-fg-3">
                  已到会话开头
                </Typography.Text>
              ) : null}

              {messageError ? (
                <Alert
                  action={
                    <Button onClick={handleRetryMessages} size="small">
                      重试
                    </Button>
                  }
                  message={messageError}
                  showIcon
                  type="error"
                />
              ) : null}

              {!messages.length && !messageError ? (
                <CenteredState compact>
                  <Empty description="这个会话没有可显示的对话" image={Empty.PRESENTED_IMAGE_SIMPLE} />
                </CenteredState>
              ) : null}
              {visibleMessages.length === 0 && hiddenMessageCount > 0 && !messageError ? (
                <Typography.Text className="self-center text-[11px] text-fg-3">
                  已隐藏 {hiddenMessageCount} 条技术项
                </Typography.Text>
              ) : null}

              {visibleMessages.map((message, index) => (
                <CodexHistoryMessageBlock
                  key={`${message.timestamp ?? "no-ts"}-${message.role}-${message.itemType}-${index}`}
                  message={message}
                  viewPreferences={codexHistoryViewPreferences}
                />
              ))}
            </div>
          )}
        </div>
      </main>
    </div>
  );
}

function WindowStateShell({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-screen w-screen items-center justify-center bg-surface-2 p-6">
      <div className="w-full max-w-[520px]">{children}</div>
    </div>
  );
}

function CenteredState({
  children,
  compact = false,
}: {
  children: ReactNode;
  compact?: boolean;
}) {
  return (
    <div
      className={cn(
        "flex min-h-full items-center justify-center text-center",
        compact ? "py-8" : "p-6",
      )}
    >
      {children}
    </div>
  );
}


function getSessionTitle(session: CodexHistorySession) {
  return session.title?.trim() || session.sessionId || "未命名 session";
}

function formatShortDate(value?: string | null) {
  if (!value) {
    return "--";
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return "--";
  }
  return date.toLocaleDateString(undefined, {
    month: "2-digit",
    day: "2-digit",
  });
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

function formatBytes(value: number) {
  if (!Number.isFinite(value) || value <= 0) {
    return "0 B";
  }
  const units = ["B", "KB", "MB", "GB"];
  let size = value;
  let unitIndex = 0;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }
  const digits = unitIndex === 0 ? 0 : 1;
  return `${size.toFixed(digits)} ${units[unitIndex]}`;
}
