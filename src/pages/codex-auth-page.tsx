import {
  CheckCircleOutlined,
  DeleteOutlined,
  ImportOutlined,
  PlusOutlined,
  ReloadOutlined,
  SaveOutlined,
  SearchOutlined,
  SwapOutlined,
} from "@ant-design/icons";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Alert, Button, Empty, Input, Popconfirm, Spin, Tooltip } from "antd";
import { useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";

import { PaneHeader } from "@/components/shell/pane-header";
import { cn } from "@/lib/cn";
import { defaultAppPreferences } from "@/lib/app-preferences";
import { getCodexHomeDirectory, getErrorSummary, hasDesktopRuntime } from "@/lib/command-client";
import {
  codexAuthProfileDetailQueryKey,
  codexAuthProfilesQueryKey,
  deleteCodexAuthProfile,
  getCodexAuthProfileDetail,
  importCurrentCodexAuthProfile,
  listCodexAuthProfiles,
  queryCodexAuthQuota,
  refreshAllCodexAuthQuotas,
  switchCodexAuthProfile,
  upsertCodexAuthProfile,
} from "@/queries/codex-auth";
import { appPreferencesQueryKey, getAppPreferences } from "@/queries/preferences";
import type {
  CodexAuthProfileDetail,
  CodexAuthProfileSummary,
  CodexAuthQuotaRefreshBatchResult,
  UpsertCodexAuthProfilePayload,
} from "@/types/backend";

const EMPTY_AUTH_JSON = "{\n  \"auth_mode\": \"chatgpt\",\n  \"tokens\": {}\n}";
const CODEX_AUTH_QUOTA_REFRESH_INTERVAL_MIN = 10;
const CODEX_AUTH_QUOTA_REFRESH_INTERVAL_MAX = 3600;

type DraftState = {
  id?: string;
  name: string;
  description: string;
  codexHome: string;
  authJson: string;
  configToml: string;
};

type AutoRefreshStatus = {
  phase: "idle" | "refreshing" | "countdown" | "error";
  countdownSeconds: number;
  lastResult?: CodexAuthQuotaRefreshBatchResult | null;
  error?: string | null;
};

export default function CodexAuthPage() {
  const desktopRuntimeAvailable = hasDesktopRuntime();
  const queryClient = useQueryClient();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [draft, setDraft] = useState<DraftState>(() => buildEmptyDraft(""));
  const [inlineError, setInlineError] = useState<string | null>(null);
  const [queryText, setQueryText] = useState("");
  const [autoRefreshStatus, setAutoRefreshStatus] = useState<AutoRefreshStatus>({
    phase: "idle",
    countdownSeconds: 0,
    lastResult: null,
    error: null,
  });
  const refreshLoopGenerationRef = useRef(0);
  const refreshBatchInFlightRef = useRef(false);

  const profilesQuery = useQuery({
    queryKey: codexAuthProfilesQueryKey,
    queryFn: listCodexAuthProfiles,
    enabled: desktopRuntimeAvailable,
    staleTime: 10_000,
  });

  const codexHomeQuery = useQuery({
    queryKey: ["codexAuthHomeDirectory"],
    queryFn: getCodexHomeDirectory,
    enabled: desktopRuntimeAvailable,
    staleTime: 30_000,
  });
  const preferencesQuery = useQuery({
    queryKey: appPreferencesQueryKey,
    queryFn: getAppPreferences,
    enabled: desktopRuntimeAvailable,
    staleTime: 30_000,
  });
  const profileDetailQuery = useQuery({
    queryKey: codexAuthProfileDetailQueryKey(selectedId ?? "none"),
    queryFn: () => getCodexAuthProfileDetail(selectedId!),
    enabled: desktopRuntimeAvailable && Boolean(selectedId),
    staleTime: 10_000,
  });

  const profiles = profilesQuery.data ?? [];
  const selectedProfile = useMemo(
    () => profiles.find((profile) => profile.id === selectedId) ?? null,
    [profiles, selectedId],
  );
  const sortedProfiles = useMemo(() => sortCodexAuthProfiles(profiles), [profiles]);
  const filteredProfiles = useMemo(() => {
    const normalized = queryText.trim().toLowerCase();
    if (!normalized) {
      return sortedProfiles;
    }
    return sortedProfiles.filter((profile) =>
      [profile.name, profile.description, profile.codexHome]
        .filter(Boolean)
        .join(" ")
        .toLowerCase()
        .includes(normalized),
    );
  }, [queryText, sortedProfiles]);
  const autoRefreshIntervalSeconds = normalizeCodexAuthQuotaRefreshInterval(
    preferencesQuery.data?.codexAuth?.quotaRefreshIntervalSeconds,
  );

  useEffect(() => {
    if (selectedId && profiles.some((profile) => profile.id === selectedId)) {
      return;
    }
    setSelectedId(sortedProfiles[0]?.id ?? null);
  }, [profiles, selectedId, sortedProfiles]);

  useEffect(() => {
    if (selectedId) {
      if (profileDetailQuery.data?.id === selectedId) {
        setDraft(profileToDraft(profileDetailQuery.data));
        setInlineError(null);
      } else if (profileDetailQuery.isError) {
        setInlineError(getErrorSummary(profileDetailQuery.error).message);
      } else {
        const summary = profiles.find((profile) => profile.id === selectedId);
        if (summary) {
          setDraft({
            id: summary.id,
            name: summary.name,
            description: summary.description ?? "",
            codexHome: summary.codexHome,
            authJson: "",
            configToml: "",
          });
        }
      }
      return;
    }
    const homePath = codexHomeQuery.data?.path ?? "";
    setDraft(buildEmptyDraft(homePath));
  }, [
    codexHomeQuery.data?.path,
    profileDetailQuery.data,
    profileDetailQuery.error,
    profileDetailQuery.isError,
    profiles,
    selectedId,
  ]);

  useEffect(() => {
    if (!desktopRuntimeAvailable) {
      setAutoRefreshStatus({
        phase: "idle",
        countdownSeconds: 0,
        lastResult: null,
        error: null,
      });
      return;
    }

    const generation = refreshLoopGenerationRef.current + 1;
    refreshLoopGenerationRef.current = generation;
    let cancelled = false;
    let timerId: ReturnType<typeof window.setTimeout> | null = null;

    const clearTimer = () => {
      if (timerId !== null) {
        window.clearTimeout(timerId);
        timerId = null;
      }
    };

    const scheduleCountdown = (
      result: CodexAuthQuotaRefreshBatchResult | null,
      error: string | null,
    ) => {
      let remainingSeconds = autoRefreshIntervalSeconds;
      setAutoRefreshStatus({
        phase: error ? "error" : "countdown",
        countdownSeconds: remainingSeconds,
        lastResult: result,
        error,
      });

      const tick = () => {
        if (cancelled || refreshLoopGenerationRef.current !== generation) {
          return;
        }

        remainingSeconds -= 1;
        if (remainingSeconds <= 0) {
          void runBatch();
          return;
        }

        setAutoRefreshStatus({
          phase: error ? "error" : "countdown",
          countdownSeconds: remainingSeconds,
          lastResult: result,
          error,
        });
        timerId = window.setTimeout(tick, 1000);
      };

      timerId = window.setTimeout(tick, 1000);
    };

    const runBatch = async () => {
      if (cancelled || refreshLoopGenerationRef.current !== generation) {
        return;
      }
      if (refreshBatchInFlightRef.current) {
        setAutoRefreshStatus((current) => ({
          ...current,
          phase: "refreshing",
          countdownSeconds: 0,
          error: null,
        }));
        timerId = window.setTimeout(() => void runBatch(), 1000);
        return;
      }

      clearTimer();
      refreshBatchInFlightRef.current = true;
      setAutoRefreshStatus((current) => ({
        ...current,
        phase: "refreshing",
        countdownSeconds: 0,
        error: null,
      }));

      try {
        const result = await refreshAllCodexAuthQuotas();
        if (cancelled || refreshLoopGenerationRef.current !== generation) {
          return;
        }
        queryClient.setQueryData(codexAuthProfilesQueryKey, result.profiles);
        scheduleCountdown(result, null);
      } catch (error) {
        if (cancelled || refreshLoopGenerationRef.current !== generation) {
          return;
        }
        scheduleCountdown(null, getErrorSummary(error).message);
      } finally {
        refreshBatchInFlightRef.current = false;
      }
    };

    void runBatch();

    return () => {
      cancelled = true;
      clearTimer();
    };
  }, [autoRefreshIntervalSeconds, desktopRuntimeAvailable, queryClient]);

  const importMutation = useMutation({
    mutationFn: importCurrentCodexAuthProfile,
    onSuccess: async (profile) => {
      await queryClient.invalidateQueries({ queryKey: codexAuthProfilesQueryKey });
      setSelectedId(profile.id);
      toast.success("已导入当前 Codex 授权");
    },
    onError: (error) => setInlineError(getErrorSummary(error).message),
  });

  const saveMutation = useMutation({
    mutationFn: (input: UpsertCodexAuthProfilePayload) => upsertCodexAuthProfile(input),
    onSuccess: async (profile) => {
      await queryClient.invalidateQueries({ queryKey: codexAuthProfilesQueryKey });
      setSelectedId(profile.id);
      toast.success("Codex 授权已保存");
    },
    onError: (error) => setInlineError(getErrorSummary(error).message),
  });

  const switchMutation = useMutation({
    mutationFn: switchCodexAuthProfile,
    onSuccess: async (result) => {
      await queryClient.invalidateQueries({ queryKey: codexAuthProfilesQueryKey });
      setSelectedId(result.profile.id);
      toast.success("已切换当前 Codex 授权", {
        description: result.authPath,
      });
    },
    onError: (error) => setInlineError(getErrorSummary(error).message),
  });

  const quotaMutation = useMutation({
    mutationFn: queryCodexAuthQuota,
    onSuccess: async (profile) => {
      await queryClient.invalidateQueries({ queryKey: codexAuthProfilesQueryKey });
      setSelectedId(profile.id);
      if (profile.lastQuota?.success) {
        toast.success("Codex 额度已刷新");
      } else {
        toast.warning("额度查询未成功", {
          description: profile.lastQuota?.credentialMessage ?? profile.lastQuota?.error ?? "当前授权不支持额度查询",
        });
      }
    },
    onError: (error) => setInlineError(getErrorSummary(error).message),
  });

  const deleteMutation = useMutation({
    mutationFn: deleteCodexAuthProfile,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: codexAuthProfilesQueryKey });
      setSelectedId(null);
      toast.success("已删除 Codex 授权记录");
    },
    onError: (error) => setInlineError(getErrorSummary(error).message),
  });

  function handleNewProfile() {
    setSelectedId(null);
    setDraft(buildEmptyDraft(codexHomeQuery.data?.path ?? ""));
    setInlineError(null);
  }

  function handleSave() {
    const name = draft.name.trim();
    const codexHome = draft.codexHome.trim();
    if (!name || !codexHome) {
      setInlineError("名称和 Codex 配置目录不能为空");
      return;
    }
    setInlineError(null);
    saveMutation.mutate({
      id: draft.id,
      name,
      description: draft.description.trim(),
      codexHome,
      authJson: draft.authJson,
      configToml: draft.configToml,
    });
  }

  const selectedQuota = selectedProfile?.lastQuota ?? null;
  const autoRefreshing = autoRefreshStatus.phase === "refreshing";
  const busy =
    autoRefreshing ||
    importMutation.isPending ||
    saveMutation.isPending ||
    switchMutation.isPending ||
    quotaMutation.isPending ||
    deleteMutation.isPending ||
    profileDetailQuery.isFetching;

  return (
    <div className="grid h-full max-h-full min-h-0 grid-cols-[300px_minmax(0,1fr)] overflow-hidden bg-surface">
      <aside aria-label="Codex 授权列表" className="flex min-h-0 flex-col overflow-hidden border-r border-line">
        <PaneHeader
          actions={
            <>
              <Tooltip title="导入当前正在使用的 Codex 授权">
                <Button
                  aria-label="导入当前授权"
                  disabled={busy}
                  icon={<ImportOutlined />}
                  loading={importMutation.isPending}
                  onClick={() => {
                    setInlineError(null);
                    importMutation.mutate();
                  }}
                  size="small"
                  type="text"
                />
              </Tooltip>
              <Tooltip title="新增授权">
                <Button aria-label="新增授权" disabled={busy} icon={<PlusOutlined />} onClick={handleNewProfile} size="small" type="text" />
              </Tooltip>
            </>
          }
          meta={profiles.length ? filteredProfiles.length === profiles.length ? profiles.length : `${filteredProfiles.length}/${profiles.length}` : null}
          title="Codex 授权"
        />
        <div className="flex shrink-0 items-center gap-1 border-b border-line px-2 py-2">
          <Input
            allowClear
            aria-label="搜索授权"
            onChange={(event) => setQueryText(event.target.value)}
            placeholder="搜索名称或目录"
            prefix={<SearchOutlined className="text-fg-3" />}
            value={queryText}
          />
          <Tooltip title="刷新列表">
            <Button
              aria-label="刷新授权列表"
              disabled={busy}
              icon={<ReloadOutlined />}
              loading={profilesQuery.isFetching}
              onClick={() => void profilesQuery.refetch()}
              type="text"
            />
          </Tooltip>
        </div>
        <div className="flex h-7 shrink-0 items-center justify-between px-3 text-[12px] text-fg-3">
          <Tooltip title="当前使用优先，其余按 5 小时与 7 天剩余额度排序">
            <span>按可用额度排序</span>
          </Tooltip>
          <AutoRefreshStatusTag intervalSeconds={autoRefreshIntervalSeconds} status={autoRefreshStatus} />
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto p-1">
          {!desktopRuntimeAvailable ? (
            <Empty description="请在桌面应用中管理 Codex 授权" image={Empty.PRESENTED_IMAGE_SIMPLE} />
          ) : profilesQuery.isLoading ? (
            <div className="flex h-full items-center justify-center">
              <Spin size="small" />
            </div>
          ) : filteredProfiles.length ? (
            filteredProfiles.map((profile) => (
              <button
                aria-current={selectedId === profile.id ? "true" : undefined}
                className={cn(
                  "flex w-full flex-col gap-0.5 rounded-md px-2.5 py-2 text-left transition-colors disabled:cursor-not-allowed",
                  selectedId === profile.id ? "bg-selected" : "hover:bg-hover",
                )}
                disabled={busy}
                key={profile.id}
                onClick={() => setSelectedId(profile.id)}
                title={profile.codexHome}
                type="button"
              >
                <span className="flex min-w-0 items-center gap-2">
                  <span className="min-w-0 flex-1 truncate text-[13px] font-medium text-fg">{profile.name}</span>
                  {profile.isActive ? (
                    <span className="shrink-0 rounded bg-primary px-1.5 text-[11px] leading-[18px] text-white">当前</span>
                  ) : null}
                </span>
                <QuotaInlineSummary profile={profile} />
              </button>
            ))
          ) : (
            <Empty
              description={queryText.trim() ? "没有匹配的授权" : "还没有 Codex 授权"}
              image={Empty.PRESENTED_IMAGE_SIMPLE}
            />
          )}
        </div>
      </aside>

      <main className="flex min-h-0 min-w-0 flex-col overflow-hidden">
        <PaneHeader
          actions={
            <>
              {draft.id ? (
                <>
                  <Button
                    disabled={busy || autoRefreshing}
                    icon={<ReloadOutlined />}
                    loading={quotaMutation.isPending}
                    onClick={() => {
                      setInlineError(null);
                      quotaMutation.mutate(draft.id!);
                    }}
                    size="small"
                  >
                    查询额度
                  </Button>
                  <Button
                    disabled={busy}
                    icon={<SwapOutlined />}
                    loading={switchMutation.isPending}
                    onClick={() => {
                      setInlineError(null);
                      switchMutation.mutate(draft.id!);
                    }}
                    size="small"
                    type="primary"
                  >
                    切换使用
                  </Button>
                </>
              ) : null}
              <Tooltip title="保存到 Bexo Studio；切换使用后才会写入 Codex 配置目录">
                <Button
                  disabled={busy}
                  icon={<SaveOutlined />}
                  loading={saveMutation.isPending}
                  onClick={handleSave}
                  size="small"
                  type={draft.id ? "default" : "primary"}
                >
                  保存
                </Button>
              </Tooltip>
              {draft.id ? (
                <Popconfirm
                  cancelText="取消"
                  description="只删除 Bexo Studio 中的记录，不删除磁盘上的 auth.json 或 config.toml。"
                  okButtonProps={{ danger: true }}
                  okText="删除"
                  onConfirm={() => {
                    setInlineError(null);
                    deleteMutation.mutate(draft.id!);
                  }}
                  title="删除这条授权记录？"
                >
                  <Tooltip title="删除">
                    <Button
                      aria-label="删除授权"
                      danger
                      disabled={busy}
                      icon={<DeleteOutlined />}
                      loading={deleteMutation.isPending}
                      size="small"
                      type="text"
                    />
                  </Tooltip>
                </Popconfirm>
              ) : null}
            </>
          }
          meta={draft.id ? null : "未保存"}
          title={draft.id ? draft.name || "未命名授权" : "新增授权"}
        />

        {inlineError ? (
          <Alert
            banner
            closable
            description={inlineError}
            message="操作失败"
            onClose={() => setInlineError(null)}
            showIcon
            type="error"
          />
        ) : null}

        <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-hidden p-3">
          {selectedQuota ? <QuotaPanel quota={selectedQuota} /> : null}

          <div className="grid shrink-0 grid-cols-[72px_minmax(0,1fr)_72px_minmax(0,1fr)] items-center gap-x-3 gap-y-2">
            <FieldLabel htmlFor="codex-auth-name" title="名称" />
            <Input
              disabled={busy}
              id="codex-auth-name"
              onChange={(event) => setDraft((current) => ({ ...current, name: event.target.value }))}
              value={draft.name}
            />
            <FieldLabel htmlFor="codex-auth-description" title="说明" />
            <Input
              disabled={busy}
              id="codex-auth-description"
              onChange={(event) => setDraft((current) => ({ ...current, description: event.target.value }))}
              placeholder="可选"
              value={draft.description}
            />
            <FieldLabel htmlFor="codex-auth-home" title="配置目录" />
            <Input
              className="col-span-3 font-mono"
              disabled={busy}
              id="codex-auth-home"
              onChange={(event) => setDraft((current) => ({ ...current, codexHome: event.target.value }))}
              value={draft.codexHome}
            />
          </div>

          <div className="grid min-h-0 flex-1 grid-cols-1 gap-3 xl:grid-cols-2">
            <ConfigEditor
              disabled={busy}
              label="auth.json"
              onChange={(value) => setDraft((current) => ({ ...current, authJson: value }))}
              value={draft.authJson}
            />
            <ConfigEditor
              disabled={busy}
              label="config.toml"
              onChange={(value) => setDraft((current) => ({ ...current, configToml: value }))}
              value={draft.configToml}
            />
          </div>
        </div>
      </main>
    </div>
  );
}

function FieldLabel({ htmlFor, title }: { htmlFor: string; title: string }) {
  return (
    <label className="text-[12px] text-fg-2" htmlFor={htmlFor}>
      {title}
    </label>
  );
}

function ConfigEditor({
  disabled,
  label,
  value,
  onChange,
}: {
  disabled: boolean;
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <div className="flex min-h-0 flex-col">
      <div className="mb-1 flex items-center justify-between text-[12px]">
        <span className="font-mono font-medium text-fg">{label}</span>
        <span className="text-fg-3">{value.length.toLocaleString()} 字符</span>
      </div>
      <Input.TextArea
        aria-label={label}
        autoSize={false}
        className="allow-text-selection !h-full !min-h-0 !resize-none font-mono !text-[12px]"
        disabled={disabled}
        onChange={(event) => onChange(event.target.value)}
        spellCheck={false}
        value={value}
      />
    </div>
  );
}

function AutoRefreshStatusTag({
  intervalSeconds,
  status,
}: {
  intervalSeconds: number;
  status: AutoRefreshStatus;
}) {
  if (status.phase === "refreshing") {
    return <span className="text-primary">额度刷新中…</span>;
  }

  if (status.phase === "idle") {
    return <span>每 {intervalSeconds}s 刷新额度</span>;
  }

  const title = status.error
    ? `上轮刷新失败：${status.error}`
    : status.lastResult
      ? `上轮完成：成功 ${status.lastResult.refreshed}，失败 ${status.lastResult.failed}，共 ${status.lastResult.total}。完成时间：${formatDateTime(status.lastResult.finishedAt)}`
      : "等待下一轮额度刷新";

  return (
    <Tooltip title={title}>
      <span className={status.error ? "text-warning" : undefined}>
        {status.error ? "刷新失败，" : ""}
        {status.countdownSeconds}s 后刷新
      </span>
    </Tooltip>
  );
}

function QuotaInlineSummary({ profile }: { profile: CodexAuthProfileSummary }) {
  const quota = profile.lastQuota;
  if (!quota) {
    return <span className="truncate text-[12px] text-fg-3">{profile.description || "未查询额度"}</span>;
  }
  if (!quota.success) {
    return <span className="truncate text-[12px] text-warning">额度不可用：{quota.credentialStatus}</span>;
  }
  const weeklyQuotaExhausted = isWeeklyQuotaExhausted(profile);
  return (
    <span className="flex min-w-0 items-center gap-3 text-[12px] text-fg-3">
      {quota.tiers.slice(0, 2).map((tier) => (
        <span className="shrink-0" key={tier.name}>
          {formatTierName(tier.name)}{" "}
          <span className={quotaTextClassName(tier.utilization)}>{formatPercent(tier.utilization)}</span>
        </span>
      ))}
      {weeklyQuotaExhausted ? <span className="shrink-0 text-danger">周限额已满</span> : null}
      {profile.description ? <span className="min-w-0 truncate">{profile.description}</span> : null}
    </span>
  );
}

function QuotaPanel({ quota }: { quota: NonNullable<CodexAuthProfileSummary["lastQuota"]> }) {
  return (
    <section aria-label="Codex 额度" className="shrink-0 rounded-md border border-line px-3 py-2">
      <div className="flex items-center gap-2 text-[12px]">
        {quota.success ? <CheckCircleOutlined className="text-success" /> : null}
        <span className="font-medium text-fg">{quota.success ? "额度" : "额度查询失败"}</span>
        <span className="text-fg-3">{quota.credentialStatus}</span>
        <span className="ml-auto text-fg-3">更新于 {formatDateTime(quota.queriedAt)}</span>
      </div>
      {quota.success ? (
        quota.tiers.length ? (
          <div className="mt-2 grid gap-x-6 gap-y-2 md:grid-cols-2">
            {quota.tiers.map((tier) => (
              <div key={tier.name}>
                <div className="mb-1 flex justify-between text-[12px]">
                  <span className="text-fg-2">{formatTierName(tier.name)}</span>
                  <span className="text-fg">
                    已用 {formatPercent(tier.utilization)}
                    {tier.resetsAt ? <span className="ml-2 text-fg-3">{formatDateTime(tier.resetsAt)} 重置</span> : null}
                  </span>
                </div>
                <div className="h-1.5 overflow-hidden rounded-full bg-hover">
                  <div
                    className={cn(
                      "h-full rounded-full",
                      tier.utilization >= 90 ? "bg-danger" : tier.utilization >= 70 ? "bg-warning" : "bg-success",
                    )}
                    style={{ width: `${Math.max(0, Math.min(100, tier.utilization))}%` }}
                  />
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="mt-1 text-[12px] text-fg-3">查询成功，但没有返回额度窗口。</div>
        )
      ) : (
        <div className="mt-1 text-[12px] text-fg-2">
          {quota.credentialMessage ?? quota.error ?? "当前授权无法查询官方 Codex 额度。"}
        </div>
      )}
    </section>
  );
}

function quotaTextClassName(utilization: number) {
  if (utilization >= 90) return "text-danger";
  if (utilization >= 70) return "text-warning";
  return "text-fg-2";
}

type QuotaSortMetrics = {
  group: number;
  constrainedRemaining: number;
  fiveHourRemaining: number;
  sevenDayRemaining: number;
  queriedAtMs: number;
};

function sortCodexAuthProfiles(profiles: CodexAuthProfileSummary[]) {
  return profiles
    .map((profile, index) => ({ profile, index, metrics: getQuotaSortMetrics(profile) }))
    .sort((left, right) => {
      if (left.metrics.group !== right.metrics.group) {
        return left.metrics.group - right.metrics.group;
      }

      if (left.profile.isActive !== right.profile.isActive) {
        return left.profile.isActive ? -1 : 1;
      }

      const constrainedCompare = compareNumberDesc(
        left.metrics.constrainedRemaining,
        right.metrics.constrainedRemaining,
      );
      if (constrainedCompare !== 0) return constrainedCompare;

      const fiveHourCompare = compareNumberDesc(
        left.metrics.fiveHourRemaining,
        right.metrics.fiveHourRemaining,
      );
      if (fiveHourCompare !== 0) return fiveHourCompare;

      const sevenDayCompare = compareNumberDesc(
        left.metrics.sevenDayRemaining,
        right.metrics.sevenDayRemaining,
      );
      if (sevenDayCompare !== 0) return sevenDayCompare;

      const queriedAtCompare = compareNumberDesc(left.metrics.queriedAtMs, right.metrics.queriedAtMs);
      if (queriedAtCompare !== 0) return queriedAtCompare;

      const nameCompare = left.profile.name.localeCompare(right.profile.name, undefined, {
        numeric: true,
        sensitivity: "base",
      });
      if (nameCompare !== 0) return nameCompare;

      return left.index - right.index;
    })
    .map((item) => item.profile);
}

function getQuotaSortMetrics(profile: CodexAuthProfileSummary): QuotaSortMetrics {
  const quota = profile.lastQuota;
  if (!quota) {
    return buildQuotaSortMetrics(3, null, null, profile.lastQuotaCheckedAt ?? profile.updatedAt);
  }

  if (!quota.success) {
    return buildQuotaSortMetrics(3, null, null, quota.queriedAt);
  }

  const fiveHourRemaining = getTierRemaining(profile, "five_hour");
  const sevenDayRemaining = getTierRemaining(profile, "seven_day");

  if (sevenDayRemaining !== null && sevenDayRemaining <= 0) {
    return buildQuotaSortMetrics(4, fiveHourRemaining, sevenDayRemaining, quota.queriedAt);
  }

  if (fiveHourRemaining === null || sevenDayRemaining === null) {
    return buildQuotaSortMetrics(3, fiveHourRemaining, sevenDayRemaining, quota.queriedAt);
  }

  if (fiveHourRemaining <= 0) {
    return buildQuotaSortMetrics(2, fiveHourRemaining, sevenDayRemaining, quota.queriedAt);
  }

  return buildQuotaSortMetrics(
    profile.isActive ? 0 : 1,
    fiveHourRemaining,
    sevenDayRemaining,
    quota.queriedAt,
  );
}

function buildQuotaSortMetrics(
  group: number,
  fiveHourRemaining: number | null,
  sevenDayRemaining: number | null,
  queriedAt?: string | null,
): QuotaSortMetrics {
  const knownRemainingValues = [fiveHourRemaining, sevenDayRemaining].filter(
    (value): value is number => value !== null,
  );
  return {
    group,
    constrainedRemaining: knownRemainingValues.length ? Math.min(...knownRemainingValues) : -1,
    fiveHourRemaining: fiveHourRemaining ?? -1,
    sevenDayRemaining: sevenDayRemaining ?? -1,
    queriedAtMs: parseDateMs(queriedAt),
  };
}

function getTierRemaining(profile: CodexAuthProfileSummary, tierName: "five_hour" | "seven_day") {
  const tier = profile.lastQuota?.tiers.find((item) => item.name === tierName);
  if (!tier || !Number.isFinite(tier.utilization)) {
    return null;
  }
  return Math.max(0, Math.min(100, 100 - tier.utilization));
}

function isWeeklyQuotaExhausted(profile: CodexAuthProfileSummary) {
  const sevenDayRemaining = getTierRemaining(profile, "seven_day");
  return sevenDayRemaining !== null && sevenDayRemaining <= 0;
}

function compareNumberDesc(left: number, right: number) {
  if (left === right) {
    return 0;
  }
  return right - left;
}

function parseDateMs(value?: string | null) {
  if (!value) {
    return 0;
  }
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? 0 : date.getTime();
}

function buildEmptyDraft(codexHome: string): DraftState {
  return {
    name: "",
    description: "",
    codexHome,
    authJson: EMPTY_AUTH_JSON,
    configToml: "",
  };
}

function profileToDraft(profile: CodexAuthProfileDetail): DraftState {
  return {
    id: profile.id,
    name: profile.name,
    description: profile.description ?? "",
    codexHome: profile.codexHome,
    authJson: profile.authJson,
    configToml: profile.configToml,
  };
}

function formatTierName(value: string) {
  switch (value) {
    case "five_hour":
      return "5 小时";
    case "seven_day":
      return "7 天";
    default:
      return value.replaceAll("_", " ");
  }
}

function formatPercent(value: number) {
  if (!Number.isFinite(value)) {
    return "-";
  }
  return `${Math.round(value)}%`;
}

function formatDateTime(value?: string | null) {
  if (!value) {
    return "-";
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return date.toLocaleString();
}

function normalizeCodexAuthQuotaRefreshInterval(value?: number | null) {
  if (!Number.isFinite(value)) {
    return defaultAppPreferences.codexAuth.quotaRefreshIntervalSeconds;
  }
  const rounded = Math.round(value as number);
  if (
    rounded < CODEX_AUTH_QUOTA_REFRESH_INTERVAL_MIN ||
    rounded > CODEX_AUTH_QUOTA_REFRESH_INTERVAL_MAX
  ) {
    return defaultAppPreferences.codexAuth.quotaRefreshIntervalSeconds;
  }
  return rounded;
}
