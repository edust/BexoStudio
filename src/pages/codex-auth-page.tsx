import {
  CheckCircleOutlined,
  DeleteOutlined,
  ImportOutlined,
  PlusOutlined,
  ReloadOutlined,
  SaveOutlined,
  SwapOutlined,
} from "@ant-design/icons";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Alert, Button, Empty, Input, Popconfirm, Spin, Tag, Tooltip, Typography } from "antd";
import { useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";

import { cn } from "@/lib/cn";
import { defaultAppPreferences } from "@/lib/app-preferences";
import { getCodexHomeDirectory, getErrorSummary, hasDesktopRuntime } from "@/lib/command-client";
import {
  codexAuthProfilesQueryKey,
  deleteCodexAuthProfile,
  importCurrentCodexAuthProfile,
  listCodexAuthProfiles,
  queryCodexAuthQuota,
  refreshAllCodexAuthQuotas,
  switchCodexAuthProfile,
  upsertCodexAuthProfile,
} from "@/queries/codex-auth";
import { appPreferencesQueryKey, getAppPreferences } from "@/queries/preferences";
import { useShellStore } from "@/stores/shell-store";
import type {
  CodexAuthProfileRecord,
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
  const themeMode = useShellStore((state) => state.themeMode);
  const isDark = themeMode === "dark";
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
    if (selectedProfile) {
      setDraft(profileToDraft(selectedProfile));
      setInlineError(null);
      return;
    }
    const homePath = codexHomeQuery.data?.path ?? "";
    setDraft(buildEmptyDraft(homePath));
  }, [codexHomeQuery.data?.path, selectedProfile]);

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
    importMutation.isPending ||
    saveMutation.isPending ||
    switchMutation.isPending ||
    quotaMutation.isPending ||
    deleteMutation.isPending;

  return (
    <div className="flex h-full max-h-full min-h-0 flex-col overflow-hidden bg-panel">
      <header
        className={cn(
          "flex min-h-[76px] shrink-0 items-center justify-between border-b px-5",
          isDark ? "border-[#353535]" : "border-[#e6edf5]",
        )}
      >
        <div className="min-w-0">
          <Typography.Title className="!mb-1 !text-[22px]" level={2}>
            Codex Auth
          </Typography.Title>
          <Typography.Paragraph className="!mb-0 !text-[12px] !text-[#6f7f93]">
            只管理 Codex 的 auth.json 与 config.toml；不包含其他 AI CLI 或 ChatGPT 登录托管。
          </Typography.Paragraph>
        </div>
        <div className="flex items-center gap-2">
          <AutoRefreshStatusTag
            intervalSeconds={autoRefreshIntervalSeconds}
            status={autoRefreshStatus}
          />
          <Button
            icon={<ImportOutlined />}
            loading={importMutation.isPending}
            onClick={() => {
              setInlineError(null);
              importMutation.mutate();
            }}
          >
            导入当前
          </Button>
          <Button icon={<PlusOutlined />} onClick={handleNewProfile}>
            新增授权
          </Button>
        </div>
      </header>

      <div className="grid min-h-0 flex-1 grid-cols-[390px_minmax(0,1fr)] overflow-hidden">
        <aside
          className={cn(
            "flex min-h-0 flex-col overflow-hidden border-r",
            isDark ? "border-[#353535]" : "border-[#e6edf5]",
          )}
        >
          <div className="shrink-0 border-b border-[color:var(--border)] p-4">
            <Input
              allowClear
              placeholder="搜索授权名称或目录"
              size="middle"
              value={queryText}
              onChange={(event) => setQueryText(event.target.value)}
            />
            <div className="mt-3 flex items-center justify-between text-[12px] text-muted-foreground">
              <span>{filteredProfiles.length} / {profiles.length}</span>
              <div className="flex items-center gap-1">
                <Tooltip title="当前使用优先；其余按 5 小时和 7 天剩余额度冗余排序">
                  <Tag className="m-0" color="blue">
                    可用优先
                  </Tag>
                </Tooltip>
                <Tooltip title="刷新列表">
                  <Button
                    icon={<ReloadOutlined />}
                    loading={profilesQuery.isFetching}
                    size="small"
                    type="text"
                    onClick={() => void profilesQuery.refetch()}
                  />
                </Tooltip>
              </div>
            </div>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto p-2">
            {!desktopRuntimeAvailable ? (
              <Alert message="需要在桌面应用中管理 Codex 授权" showIcon type="warning" />
            ) : profilesQuery.isLoading ? (
              <div className="flex h-full items-center justify-center">
                <Spin />
              </div>
            ) : filteredProfiles.length ? (
              <div className="space-y-2">
                {filteredProfiles.map((profile) => (
                  <button
                    className={cn(
                      "w-full rounded-[10px] border px-3 py-3 text-left transition-colors",
                      selectedId === profile.id
                        ? "border-[#087fc5] bg-[#0b76b7]/20"
                        : isDark
                          ? "border-[#353535] bg-[#252526] hover:bg-[#2d2d30]"
                          : "border-[#e3eaf2] bg-white hover:bg-[#f7fafc]",
                    )}
                    key={profile.id}
                    onClick={() => setSelectedId(profile.id)}
                    type="button"
                  >
                    <div className="flex items-center justify-between gap-2">
                      <span className="truncate text-[14px] font-semibold">{profile.name}</span>
                      {profile.isActive ? <Tag color="blue">当前</Tag> : null}
                    </div>
                    <div className="mt-1 line-clamp-2 text-[12px] text-muted-foreground">
                      {profile.description || "未填写说明"}
                    </div>
                    <div className="mt-2 truncate font-mono text-[11px] text-[#8aa0bd]">
                      {profile.codexHome}
                    </div>
                    <QuotaInlineSummary profile={profile} />
                  </button>
                ))}
              </div>
            ) : (
              <Empty description="没有 Codex 授权记录" image={Empty.PRESENTED_IMAGE_SIMPLE} />
            )}
          </div>
        </aside>

        <main className="flex min-h-0 min-w-0 flex-col overflow-hidden">
          {inlineError ? (
            <Alert
              className="mx-4 mt-4 shrink-0"
              closable
              description={inlineError}
              message="Codex Auth 操作失败"
              showIcon
              type="error"
              onClose={() => setInlineError(null)}
            />
          ) : null}

          <section
            className={cn(
              "flex min-h-0 flex-1 flex-col overflow-hidden",
              isDark ? "bg-[#242424]" : "bg-white",
            )}
          >
            <div className="flex shrink-0 items-center justify-between border-b border-[color:var(--border)] px-4 py-3">
              <div className="min-w-0">
                <Typography.Title className="!mb-0 !text-[16px]" level={3}>
                  {draft.id ? "编辑授权" : "新增授权"}
                </Typography.Title>
                <Typography.Text className="text-[12px] text-muted-foreground">
                  保存后才会进入列表；点击“切换使用”才会写入 Codex 真实配置目录。
                </Typography.Text>
              </div>
              <div className="flex items-center gap-2">
                {draft.id ? (
                  <>
                    <Button
                      icon={<ReloadOutlined />}
                      disabled={autoRefreshing}
                      loading={quotaMutation.isPending}
                      onClick={() => {
                        setInlineError(null);
                        quotaMutation.mutate(draft.id!);
                      }}
                    >
                      查询额度
                    </Button>
                    <Button
                      icon={<SwapOutlined />}
                      loading={switchMutation.isPending}
                      type="primary"
                      onClick={() => {
                        setInlineError(null);
                        switchMutation.mutate(draft.id!);
                      }}
                    >
                      切换使用
                    </Button>
                  </>
                ) : null}
                <Button icon={<SaveOutlined />} loading={saveMutation.isPending} onClick={handleSave}>
                  保存
                </Button>
                {draft.id ? (
                  <Popconfirm
                    title="删除这条 Codex 授权记录？"
                    description="只删除 Bexo Studio 中的记录，不删除磁盘上的 auth.json 或 config.toml。"
                    okText="删除"
                    cancelText="取消"
                    onConfirm={() => {
                      setInlineError(null);
                      deleteMutation.mutate(draft.id!);
                    }}
                  >
                    <Button danger icon={<DeleteOutlined />} loading={deleteMutation.isPending}>
                      删除
                    </Button>
                  </Popconfirm>
                ) : null}
              </div>
            </div>

            <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-hidden p-4">
              {selectedQuota ? <QuotaPanel quota={selectedQuota} /> : null}

              <div className="grid shrink-0 grid-cols-[220px_minmax(0,1fr)] gap-3">
                <FieldLabel title="名称" />
                <Input
                  value={draft.name}
                  onChange={(event) => setDraft((current) => ({ ...current, name: event.target.value }))}
                />
                <FieldLabel title="说明" />
                <Input
                  value={draft.description}
                  onChange={(event) =>
                    setDraft((current) => ({ ...current, description: event.target.value }))
                  }
                />
                <FieldLabel title="Codex 配置目录" />
                <Input
                  className="font-mono"
                  value={draft.codexHome}
                  onChange={(event) =>
                    setDraft((current) => ({ ...current, codexHome: event.target.value }))
                  }
                />
              </div>

              <div className="grid min-h-0 flex-1 grid-cols-1 gap-4 xl:grid-cols-2">
                <ConfigEditor
                  label="auth.json"
                  value={draft.authJson}
                  onChange={(value) => setDraft((current) => ({ ...current, authJson: value }))}
                />
                <ConfigEditor
                  label="config.toml"
                  value={draft.configToml}
                  onChange={(value) => setDraft((current) => ({ ...current, configToml: value }))}
                />
              </div>
            </div>
          </section>

          {busy ? (
            <div className="pointer-events-none fixed bottom-4 right-4 rounded-full border border-[color:var(--border)] bg-panel-elevated px-3 py-2 text-[12px] shadow-lg">
              正在处理 Codex Auth 操作...
            </div>
          ) : null}
        </main>
      </div>
    </div>
  );
}

function FieldLabel({ title, description }: { title: string; description?: string }) {
  return (
    <div>
      <div className="text-[13px] font-semibold">{title}</div>
      {description ? <div className="mt-0.5 text-[12px] text-muted-foreground">{description}</div> : null}
    </div>
  );
}

function ConfigEditor({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <div className="flex min-h-0 flex-col">
      <div className="mb-2 flex items-center justify-between">
        <span className="text-[13px] font-semibold">{label}</span>
        <Tag>{value.length.toLocaleString()} chars</Tag>
      </div>
      <Input.TextArea
        className="allow-text-selection !h-full !min-h-0 !resize-none font-mono !text-[12px]"
        spellCheck={false}
        value={value}
        autoSize={false}
        onChange={(event) => onChange(event.target.value)}
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
    return <Tag color="processing">自动刷新中</Tag>;
  }

  if (status.phase === "idle") {
    return <Tag>自动刷新 {intervalSeconds}s</Tag>;
  }

  const title = status.error
    ? `上轮刷新失败：${status.error}`
    : status.lastResult
      ? `上轮完成：成功 ${status.lastResult.refreshed}，失败 ${status.lastResult.failed}，共 ${status.lastResult.total}。完成时间：${formatDateTime(status.lastResult.finishedAt)}`
      : "等待下一轮 Codex Auth 额度刷新";

  return (
    <Tooltip title={title}>
      <Tag color={status.error ? "gold" : "blue"}>
        {status.error ? "刷新失败，" : "下次刷新 "}
        {status.countdownSeconds}s
      </Tag>
    </Tooltip>
  );
}

function QuotaInlineSummary({ profile }: { profile: CodexAuthProfileRecord }) {
  const quota = profile.lastQuota;
  if (!quota) {
    return <div className="mt-2 text-[11px] text-muted-foreground">未查询额度</div>;
  }
  if (!quota.success) {
    return <div className="mt-2 text-[11px] text-[#d59a35]">额度不可用：{quota.credentialStatus}</div>;
  }
  const weeklyQuotaExhausted = isWeeklyQuotaExhausted(profile);
  return (
    <div className="mt-2 flex flex-wrap gap-1">
      {quota.tiers.slice(0, 2).map((tier) => (
        <Tag color={quotaColor(tier.utilization)} key={tier.name}>
          {formatTierName(tier.name)} {formatPercent(tier.utilization)}
        </Tag>
      ))}
      {weeklyQuotaExhausted ? <Tag color="red">周限额满</Tag> : null}
    </div>
  );
}

function QuotaPanel({ quota }: { quota: NonNullable<CodexAuthProfileRecord["lastQuota"]> }) {
  return (
    <div className="rounded-[10px] border border-[color:var(--border)] bg-panel-muted p-3">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          {quota.success ? <CheckCircleOutlined className="text-[#3ca267]" /> : null}
          <span className="text-[13px] font-semibold">
            {quota.success ? "Codex 额度" : "额度查询失败"}
          </span>
          <Tag>{quota.credentialStatus}</Tag>
        </div>
        <span className="text-[12px] text-muted-foreground">{formatDateTime(quota.queriedAt)}</span>
      </div>
      {quota.success ? (
        <div className="mt-3 grid gap-2">
          {quota.tiers.length ? (
            quota.tiers.map((tier) => (
              <div key={tier.name}>
                <div className="mb-1 flex justify-between text-[12px]">
                  <span>{formatTierName(tier.name)}</span>
                  <span>{formatPercent(tier.utilization)}</span>
                </div>
                <div className="h-2 overflow-hidden rounded-full bg-black/10">
                  <div
                    className={cn(
                      "h-full rounded-full",
                      tier.utilization >= 90
                        ? "bg-[#c94d4d]"
                        : tier.utilization >= 70
                          ? "bg-[#d49a32]"
                          : "bg-[#3ca267]",
                    )}
                    style={{ width: `${Math.max(0, Math.min(100, tier.utilization))}%` }}
                  />
                </div>
                {tier.resetsAt ? (
                  <div className="mt-1 text-[11px] text-muted-foreground">
                    重置：{formatDateTime(tier.resetsAt)}
                  </div>
                ) : null}
              </div>
            ))
          ) : (
            <Typography.Text className="text-[12px] text-muted-foreground">
              API 返回成功，但没有额度窗口。
            </Typography.Text>
          )}
        </div>
      ) : (
        <Typography.Paragraph className="!mb-0 !mt-2 !text-[12px] !text-muted-foreground">
          {quota.credentialMessage ?? quota.error ?? "当前授权无法查询官方 Codex 额度。"}
        </Typography.Paragraph>
      )}
    </div>
  );
}

type QuotaSortMetrics = {
  group: number;
  constrainedRemaining: number;
  fiveHourRemaining: number;
  sevenDayRemaining: number;
  queriedAtMs: number;
};

function sortCodexAuthProfiles(profiles: CodexAuthProfileRecord[]) {
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

function getQuotaSortMetrics(profile: CodexAuthProfileRecord): QuotaSortMetrics {
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

function getTierRemaining(profile: CodexAuthProfileRecord, tierName: "five_hour" | "seven_day") {
  const tier = profile.lastQuota?.tiers.find((item) => item.name === tierName);
  if (!tier || !Number.isFinite(tier.utilization)) {
    return null;
  }
  return Math.max(0, Math.min(100, 100 - tier.utilization));
}

function isWeeklyQuotaExhausted(profile: CodexAuthProfileRecord) {
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

function profileToDraft(profile: CodexAuthProfileRecord): DraftState {
  return {
    id: profile.id,
    name: profile.name,
    description: profile.description ?? "",
    codexHome: profile.codexHome,
    authJson: profile.authJson,
    configToml: profile.configToml,
  };
}

function quotaColor(value: number) {
  if (value >= 90) return "red";
  if (value >= 70) return "gold";
  return "green";
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
