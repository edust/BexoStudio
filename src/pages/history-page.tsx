import { ReloadOutlined, SearchOutlined } from "@ant-design/icons";
import { useQuery } from "@tanstack/react-query";
import { Alert, Button, Checkbox, Empty, Input, Select, Spin, Tag, Tooltip, Typography } from "antd";
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";
import { toast } from "sonner";

import { CodexHistoryMessageBlock } from "@/features/codex-history/codex-history-message-block";
import { filterCodexHistoryMessages } from "@/features/codex-history/codex-history-message-rules";
import { useCodexHistoryViewPreferences } from "@/features/codex-history/codex-history-view-preferences";
import { cn } from "@/lib/cn";
import {
  getCodexHistoryMessages,
  getErrorSummary,
  hasDesktopRuntime,
  listAllCodexHistorySessions,
  listWorkspaces,
} from "@/lib/command-client";
import type { CodexHistoryMessage, CodexHistorySession, WorkspaceRecord } from "@/types/backend";

const MESSAGE_PAGE_SIZE = 40;
const SESSION_ROW_HEIGHT = 92;
const SESSION_OVERSCAN = 8;

type ScrollRestoreState = {
  previousHeight: number;
  previousTop: number;
};

export default function HistoryPage() {
  const desktopRuntimeAvailable = hasDesktopRuntime();
  const [query, setQuery] = useState("");
  const [workspaceFilter, setWorkspaceFilter] = useState("all");
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

  const sessionsQuery = useQuery({
    queryKey: ["codex-history-global-sessions"],
    queryFn: listAllCodexHistorySessions,
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
  const selectedWorkspace = useMemo(
    () => workspaces.find((workspace) => workspace.id === workspaceFilter) ?? null,
    [workspaceFilter, workspaces],
  );
  const selectedWorkspacePath = selectedWorkspace ? resolveWorkspacePath(selectedWorkspace) : "";

  const filteredSessions = useMemo(() => {
    const sessions = sessionsQuery.data?.sessions ?? [];
    const normalizedQuery = query.trim().toLowerCase();

    return sessions.filter((session) => {
      if (selectedWorkspacePath && !pathBelongsToRoot(session.projectDir, selectedWorkspacePath)) {
        return false;
      }

      if (!normalizedQuery) {
        return true;
      }

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
      return searchable.includes(normalizedQuery);
    });
  }, [query, selectedWorkspacePath, sessionsQuery.data?.sessions]);

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

  const handleRefresh = useCallback(async () => {
    try {
      await sessionsQuery.refetch();
    } catch (error) {
      toast.error(getErrorSummary(error).message);
    }
  }, [sessionsQuery]);

  const totalSessions = sessionsQuery.data?.sessions.length ?? 0;
  const codexRootCount = sessionsQuery.data?.codexRoots.length ?? 0;

  if (!desktopRuntimeAvailable) {
    return (
      <div className="p-6">
        <Alert message="当前页面需要在 Bexo Studio 桌面应用中打开" showIcon type="error" />
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col bg-[#f5f7fa]">
      <header className="border-b border-[#d9e2ec] bg-white px-5 py-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div className="min-w-0">
            <Typography.Title className="!mb-1 !text-[18px] !leading-7" level={3}>
              Session / History
            </Typography.Title>
            <Typography.Paragraph className="!mb-0 !text-[12px] !leading-5 !text-[#667085]">
              只读查看本机 Codex 会话，支持按工作区路径筛选。
            </Typography.Paragraph>
          </div>
          <div className="flex items-center gap-2">
            <Checkbox
              checked={showTechnicalMessages}
              className="mr-1 text-[12px] text-[#667085]"
              onChange={(event) => setShowTechnicalMessages(event.target.checked)}
            >
              显示技术项
            </Checkbox>
            <Tag bordered={false} className="m-0 rounded-[6px] bg-[#eef6fb] text-[#12718f]">
              {filteredSessions.length} / {totalSessions} sessions
            </Tag>
            <Tag bordered={false} className="m-0 rounded-[6px] bg-[#f2f4f7] text-[#475467]">
              {codexRootCount} roots
            </Tag>
            <Tooltip title="刷新">
              <Button
                className="!h-8 !w-8 !min-w-8 !p-0"
                icon={<ReloadOutlined />}
                loading={sessionsQuery.isFetching}
                onClick={() => void handleRefresh()}
              />
            </Tooltip>
          </div>
        </div>

        <div className="mt-4 grid gap-3 lg:grid-cols-[minmax(220px,320px)_minmax(260px,1fr)]">
          <Select
            className="w-full"
            onChange={(value) => {
              setWorkspaceFilter(value);
              setSessionScrollTop(0);
              if (sessionListRef.current) {
                sessionListRef.current.scrollTop = 0;
              }
            }}
            options={[
              { label: "全部路径", value: "all" },
              ...workspaces.map((workspace) => ({
                label: workspace.name,
                value: workspace.id,
              })),
            ]}
            placeholder="按工作区筛选"
            showSearch
            value={workspaceFilter}
          />
          <Input
            allowClear
            onChange={(event) => {
              setQuery(event.target.value);
              setSessionScrollTop(0);
              if (sessionListRef.current) {
                sessionListRef.current.scrollTop = 0;
              }
            }}
            placeholder="搜索标题、摘要、路径或 session id"
            prefix={<SearchOutlined className="text-[#98a2b3]" />}
            value={query}
          />
        </div>
      </header>

      <div className="grid min-h-0 flex-1 grid-cols-[360px_minmax(0,1fr)] overflow-hidden">
        <section className="min-h-0 border-r border-[#d9e2ec] bg-white">
          {sessionsQuery.isLoading ? (
            <CenteredState>
              <Spin />
            </CenteredState>
          ) : sessionsQuery.isError ? (
            <div className="p-4">
              <Alert message={getErrorSummary(sessionsQuery.error).message} showIcon type="error" />
            </div>
          ) : !filteredSessions.length ? (
            <CenteredState>
              <Empty description="没有匹配的 Codex session" image={Empty.PRESENTED_IMAGE_SIMPLE} />
            </CenteredState>
          ) : (
            <div
              className="h-full overflow-y-auto px-2 py-2"
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
                    <SessionListItem
                      active={selectedSourcePath === session.sourcePath}
                      key={session.sourcePath}
                      onSelect={() => setSelectedSourcePath(session.sourcePath)}
                      session={session}
                      style={{
                        height: SESSION_ROW_HEIGHT - 6,
                        top: absoluteIndex * SESSION_ROW_HEIGHT,
                      }}
                    />
                  );
                })}
              </div>
            </div>
          )}
        </section>

        <section className="flex min-h-0 min-w-0 flex-col">
          <div className="flex min-h-[72px] items-center justify-between gap-3 border-b border-[#d9e2ec] bg-white px-5 py-3">
            <div className="min-w-0">
              <Typography.Title className="!mb-1 !text-[16px] !leading-6" level={4}>
                {selectedSession ? getSessionTitle(selectedSession) : "未选择 session"}
              </Typography.Title>
              <Typography.Paragraph
                className="!mb-0 !text-[12px] !leading-5 !text-[#667085]"
                ellipsis={{ rows: 1, tooltip: selectedSession?.projectDir ?? selectedSession?.sourcePath }}
              >
                {selectedSession?.projectDir ?? selectedSession?.sourcePath ?? "从左侧选择一个 Codex session"}
              </Typography.Paragraph>
            </div>
            {selectedSession ? (
              <div className="flex shrink-0 items-center gap-2">
                <Tag bordered={false} className="m-0 rounded-[6px] bg-[#ecfdf3] text-[#027a48]">
                  {formatDateTime(selectedSession.lastActiveAt ?? selectedSession.createdAt)}
                </Tag>
                <Tag bordered={false} className="m-0 rounded-[6px] bg-[#f2f4f7] text-[#475467]">
                  {formatBytes(selectedSession.fileSizeBytes)}
                </Tag>
              </div>
            ) : null}
          </div>

          <div
            className="min-h-0 flex-1 overflow-y-auto px-5 py-5"
            onScroll={handleMessagesScroll}
            ref={messagesScrollRef}
          >
            {!selectedSession ? (
              <CenteredState>
                <Empty description="选择 session 后查看最近对话" image={Empty.PRESENTED_IMAGE_SIMPLE} />
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
                  <Typography.Text className="self-center text-[11px] text-[#98a2b3]">
                    已到达此 session 起点
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
                    <Empty
                      description="这个 session 没有可显示的对话"
                      image={Empty.PRESENTED_IMAGE_SIMPLE}
                    />
                  </CenteredState>
                ) : null}
                {visibleMessages.length === 0 && hiddenMessageCount > 0 && !messageError ? (
                  <Typography.Text className="self-center text-[11px] text-[#98a2b3]">
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
        </section>
      </div>
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

function SessionListItem({
  active,
  onSelect,
  session,
  style,
}: {
  active: boolean;
  onSelect: () => void;
  session: CodexHistorySession;
  style: CSSProperties;
}) {
  return (
    <button
      className={cn(
        "absolute left-0 right-0 flex w-full flex-col items-stretch rounded-[8px] border px-3 py-2 text-left transition-colors",
        active
          ? "border-[#8fd4ec] bg-[#f4fbfe]"
          : "border-transparent bg-white hover:border-[#d9e2ec] hover:bg-[#f8fafc]",
      )}
      onClick={onSelect}
      style={style}
      type="button"
    >
      <div className="flex items-center justify-between gap-2">
        <Typography.Text
          className="min-w-0 text-[13px] font-medium text-[#1f2937]"
          ellipsis={{ tooltip: getSessionTitle(session) }}
        >
          {getSessionTitle(session)}
        </Typography.Text>
        <Typography.Text className="shrink-0 text-[10px] text-[#98a2b3]">
          {formatShortDate(session.lastActiveAt ?? session.createdAt)}
        </Typography.Text>
      </div>
      <Typography.Paragraph
        className="!mb-0 !mt-1 !text-[11px] !leading-4 !text-[#667085]"
        ellipsis={{ rows: 1, tooltip: session.summary ?? session.sourcePath }}
      >
        {session.summary ?? session.sessionId}
      </Typography.Paragraph>
      <div className="mt-1 flex items-center justify-between gap-2">
        <Typography.Text
          className="min-w-0 !text-[10px] !leading-4 !text-[#98a2b3]"
          ellipsis={{ tooltip: session.projectDir ?? session.sourcePath }}
        >
          {session.projectDir ?? session.sourcePath}
        </Typography.Text>
        <Typography.Text className="shrink-0 !text-[10px] !leading-4 !text-[#98a2b3]">
          {formatBytes(session.fileSizeBytes)}
        </Typography.Text>
      </div>
    </button>
  );
}

function resolveWorkspacePath(workspace: WorkspaceRecord) {
  return workspace.projects[0]?.path?.trim() ?? "";
}

function pathBelongsToRoot(path: string | null | undefined, root: string) {
  const normalizedPath = normalizePath(path ?? "");
  const normalizedRoot = normalizePath(root);
  if (!normalizedPath || !normalizedRoot) {
    return false;
  }
  return normalizedPath === normalizedRoot || normalizedPath.startsWith(`${normalizedRoot}\\`);
}

function normalizePath(path: string) {
  let value = path.trim().replaceAll("/", "\\");
  if (value.startsWith("\\\\?\\UNC\\")) {
    value = `\\\\${value.slice("\\\\?\\UNC\\".length)}`;
  } else if (value.startsWith("\\\\?\\")) {
    value = value.slice("\\\\?\\".length);
  }
  while (value.length > 3 && value.endsWith("\\")) {
    value = value.slice(0, -1);
  }
  return value.toLowerCase();
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
