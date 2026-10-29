import { ReloadOutlined, SearchOutlined } from "@ant-design/icons";
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { Alert, Button, Checkbox, Empty, Input, Select, Spin, Tooltip } from "antd";
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
  type UIEvent,
} from "react";

import { CodexHistoryMessageBlock } from "@/features/codex-history/codex-history-message-block";
import { CODEX_HISTORY_SESSION_ROW_HEIGHT, CodexHistorySessionItem } from "@/features/codex-history/codex-history-session-item";
import { filterCodexHistoryMessages } from "@/features/codex-history/codex-history-message-rules";
import { useCodexHistoryViewPreferences } from "@/features/codex-history/codex-history-view-preferences";
import { PaneHeader } from "@/components/shell/pane-header";
import { cn } from "@/lib/cn";
import {
  getCodexHistoryMessages,
  getErrorSummary,
  hasDesktopRuntime,
  listAllCodexHistorySessions,
  listWorkspaces,
} from "@/lib/command-client";
import type { CodexHistoryMessage, CodexHistorySession } from "@/types/backend";

const MESSAGE_PAGE_SIZE = 40;
const SESSION_PAGE_SIZE = 10;
const SESSION_ROW_HEIGHT = CODEX_HISTORY_SESSION_ROW_HEIGHT;
const SESSION_OVERSCAN = 8;
const SESSION_LOAD_THRESHOLD = 120;
const HISTORY_SEARCH_DEBOUNCE_MS = 250;

type ScrollRestoreState = {
  previousHeight: number;
  previousTop: number;
};

export default function HistoryPage() {
  const desktopRuntimeAvailable = hasDesktopRuntime();
  const [query, setQuery] = useState("");
  const [debouncedQuery, setDebouncedQuery] = useState("");
  const [workspaceFilter, setWorkspaceFilter] = useState("all");
  const [historyRefreshNonce, setHistoryRefreshNonce] = useState(0);
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
  const sessionListRef = useRef<HTMLDivElement | null>(null);
  const messagesScrollRef = useRef<HTMLDivElement | null>(null);
  const scrollRestoreRef = useRef<ScrollRestoreState | null>(null);
  const loadingOlderRef = useRef(false);
  const messageRequestIdRef = useRef(0);
  const codexHistoryViewPreferences =
    useCodexHistoryViewPreferences(desktopRuntimeAvailable);

  const sessionsQuery = useInfiniteQuery({
    queryKey: [
      "codex-history-global-sessions",
      workspaceFilter,
      debouncedQuery,
      historyRefreshNonce,
    ],
    queryFn: ({ pageParam }) =>
      listAllCodexHistorySessions({
        cursor: pageParam,
        limit: SESSION_PAGE_SIZE,
        query: debouncedQuery || null,
        workspaceId: workspaceFilter === "all" ? null : workspaceFilter,
      }),
    initialPageParam: null as string | null,
    getNextPageParam: (lastPage) =>
      lastPage.hasMore ? lastPage.nextCursor ?? undefined : undefined,
    enabled: desktopRuntimeAvailable,
    staleTime: 15_000,
  });
  const workspacesQuery = useQuery({
    queryKey: ["history-workspaces"],
    queryFn: listWorkspaces,
    enabled: desktopRuntimeAvailable,
    staleTime: 30_000,
  });

  const workspaces = workspacesQuery.data ?? [];
  const sessions = useMemo(
    () => sessionsQuery.data?.pages.flatMap((page) => page.sessions) ?? [],
    [sessionsQuery.data],
  );

  const selectedSession = useMemo(() => {
    if (!selectedSourcePath) {
      return null;
    }
    return sessions.find((session) => session.sourcePath === selectedSourcePath) ?? null;
  }, [selectedSourcePath, sessions]);

  const sessionVisibleRange = useMemo(() => {
    const viewportHeight = sessionViewportHeight || 520;
    const start = Math.max(
      0,
      Math.floor(sessionScrollTop / SESSION_ROW_HEIGHT) - SESSION_OVERSCAN,
    );
    const end = Math.min(
      sessions.length,
      Math.ceil((sessionScrollTop + viewportHeight) / SESSION_ROW_HEIGHT) + SESSION_OVERSCAN,
    );
    return { start, end };
  }, [sessions.length, sessionScrollTop, sessionViewportHeight]);

  const visibleSessions = useMemo(
    () => sessions.slice(sessionVisibleRange.start, sessionVisibleRange.end),
    [sessionVisibleRange.end, sessionVisibleRange.start, sessions],
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
    const timer = window.setTimeout(() => {
      setDebouncedQuery(query.trim());
    }, HISTORY_SEARCH_DEBOUNCE_MS);
    return () => window.clearTimeout(timer);
  }, [query]);

  useEffect(() => {
    setSessionScrollTop(0);
    if (sessionListRef.current) {
      sessionListRef.current.scrollTop = 0;
    }
  }, [debouncedQuery, historyRefreshNonce, workspaceFilter]);

  useEffect(() => {
    const stillVisible =
      selectedSourcePath &&
      sessions.some((session) => session.sourcePath === selectedSourcePath);
    if (stillVisible) {
      return;
    }
    setSelectedSourcePath(sessions[0]?.sourcePath ?? null);
  }, [selectedSourcePath, sessions]);

  useEffect(() => {
    if (!selectedSession || !desktopRuntimeAvailable) {
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
  }, [desktopRuntimeAvailable, messageReloadNonce, selectedSession]);

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
  }, [hasMoreMessages, messagesLoading, olderCursor, selectedSession]);

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

  const loadMoreSessions = useCallback(() => {
    if (!sessionsQuery.hasNextPage || sessionsQuery.isFetchingNextPage) {
      return;
    }
    void sessionsQuery.fetchNextPage();
  }, [sessionsQuery.fetchNextPage, sessionsQuery.hasNextPage, sessionsQuery.isFetchingNextPage]);

  const handleSessionListScroll = useCallback(
    (event: UIEvent<HTMLDivElement>) => {
      const element = event.currentTarget;
      setSessionScrollTop(element.scrollTop);
      const distanceToBottom =
        element.scrollHeight - element.scrollTop - element.clientHeight;
      if (distanceToBottom <= SESSION_LOAD_THRESHOLD) {
        loadMoreSessions();
      }
    },
    [loadMoreSessions],
  );

  const handleRefresh = useCallback(() => {
    setHistoryRefreshNonce((current) => current + 1);
    setSessionScrollTop(0);
    if (sessionListRef.current) {
      sessionListRef.current.scrollTop = 0;
    }
  }, []);

  const loadedSessionCount = sessions.length;
  const codexRootCount = sessionsQuery.data?.pages[0]?.codexRoots.length ?? 0;

  if (!desktopRuntimeAvailable) {
    return (
      <div className="p-6">
        <Alert message="当前页面需要在 Bexo Studio 桌面应用中打开" showIcon type="error" />
      </div>
    );
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[340px_minmax(0,1fr)] overflow-hidden bg-surface">
      <section aria-label="会话列表" className="flex min-h-0 flex-col border-r border-line">
        <PaneHeader
          actions={
            <Tooltip title="刷新">
              <Button
                aria-label="刷新会话列表"
                icon={<ReloadOutlined />}
                loading={sessionsQuery.isFetching && !sessionsQuery.isFetchingNextPage}
                onClick={handleRefresh}
                size="small"
                type="text"
              />
            </Tooltip>
          }
          meta={
            <Tooltip title={`来自 ${codexRootCount} 个 Codex 目录`}>
              <span>{sessionsQuery.hasNextPage ? `已加载 ${loadedSessionCount}` : loadedSessionCount}</span>
            </Tooltip>
          }
          title="会话历史"
        />
        <div className="shrink-0 space-y-2 border-b border-line px-2 py-2">
          <Select
            aria-label="按工作区筛选"
            className="w-full"
            onChange={(value) => {
              setWorkspaceFilter(value);
              setSessionScrollTop(0);
              if (sessionListRef.current) {
                sessionListRef.current.scrollTop = 0;
              }
            }}
            options={[
              { label: "全部工作区", value: "all" },
              ...workspaces.map((workspace) => ({
                label: workspace.name,
                value: workspace.id,
              })),
            ]}
            showSearch
            value={workspaceFilter}
          />
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
            placeholder="搜索标题、摘要、路径或 ID"
            prefix={<SearchOutlined className="text-fg-3" />}
            value={query}
          />
        </div>

        <div className="min-h-0 flex-1">
          {sessionsQuery.isLoading ? (
            <CenteredState>
              <Spin size="small" />
            </CenteredState>
          ) : sessionsQuery.isError && !sessions.length ? (
            <div className="p-3">
              <Alert message={getErrorSummary(sessionsQuery.error).message} showIcon type="error" />
            </div>
          ) : !sessions.length && !sessionsQuery.hasNextPage ? (
            <CenteredState>
              <Empty description="没有匹配的会话" image={Empty.PRESENTED_IMAGE_SIMPLE} />
            </CenteredState>
          ) : (
            <div className="h-full overflow-y-auto p-1" onScroll={handleSessionListScroll} ref={sessionListRef}>
              <div className="relative" style={{ height: sessions.length * SESSION_ROW_HEIGHT }}>
                {visibleSessions.map((session, index) => {
                  const absoluteIndex = sessionVisibleRange.start + index;
                  return (
                    <CodexHistorySessionItem
                      active={selectedSourcePath === session.sourcePath}
                      dateLabel={formatShortDate(session.lastActiveAt ?? session.createdAt)}
                      key={session.sourcePath}
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
              <div className="flex min-h-[40px] items-center justify-center py-1">
                {sessionsQuery.isError ? (
                  <Alert
                    action={
                      <Button onClick={loadMoreSessions} size="small">
                        重试
                      </Button>
                    }
                    className="!mb-0"
                    message={getErrorSummary(sessionsQuery.error).message}
                    showIcon
                    type="error"
                  />
                ) : sessionsQuery.isFetchingNextPage ? (
                  <Spin size="small" />
                ) : sessionsQuery.hasNextPage ? (
                  <Button onClick={loadMoreSessions} size="small" type="link">
                    继续加载 10 条
                  </Button>
                ) : (
                  <span className="text-[12px] text-fg-3">共 {loadedSessionCount} 条</span>
                )}
              </div>
            </div>
          )}
        </div>
      </section>

      <section aria-label="对话内容" className="flex min-h-0 min-w-0 flex-col">
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
              <span title={selectedSession.projectDir ?? selectedSession.sourcePath}>
                {formatDateTime(selectedSession.lastActiveAt ?? selectedSession.createdAt)}
                <span className="mx-2">·</span>
                {formatBytes(selectedSession.fileSizeBytes)}
                <span className="mx-2">·</span>
                {selectedSession.projectDir ?? selectedSession.sourcePath}
              </span>
            ) : null
          }
          title={selectedSession ? getSessionTitle(selectedSession) : "未选择会话"}
        />

        <div className="min-h-0 flex-1 overflow-y-auto px-4 py-3" onScroll={handleMessagesScroll} ref={messagesScrollRef}>
          {!selectedSession ? (
            <CenteredState>
              <Empty description="在左侧选择一个会话" image={Empty.PRESENTED_IMAGE_SIMPLE} />
            </CenteredState>
          ) : messagesLoading ? (
            <CenteredState>
              <Spin size="small" />
            </CenteredState>
          ) : (
            <div className="allow-text-selection flex w-full flex-col gap-3">
              {hasMoreMessages ? (
                <div className="flex justify-center py-1">
                  <Button loading={loadingOlder} onClick={() => void loadOlderMessages()} size="small">
                    加载更早消息
                  </Button>
                </div>
              ) : messages.length ? (
                <span className="self-center text-[12px] text-fg-3">已到会话开头</span>
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
                <span className="self-center text-[12px] text-fg-3">已隐藏 {hiddenMessageCount} 条技术项</span>
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
      </section>
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
