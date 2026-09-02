import { CheckCircleOutlined, CloudDownloadOutlined, CloudUploadOutlined, PauseCircleOutlined } from "@ant-design/icons";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Button, Drawer, Empty, Popconfirm, Progress, Tag, Typography } from "antd";
import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";

import {
  cancelOssTransfer,
  confirmOssTransfer,
  getErrorSummary,
  hasDesktopRuntime,
  listenToOssTransferProgressEvents,
  listenToOssTransferStateEvents,
  listOssTransferTasks,
  resumeOssTransfer,
} from "@/lib/command-client";
import { ossTransferTasksQueryKey } from "@/queries/oss";
import type { OssTransferTaskView } from "@/types/backend";

type OssTransferQueueProps = {
  onTaskStateChanged?: (task: OssTransferTaskView) => void;
};

type TransferMetric = {
  bytes: number;
  timestamp: number;
  speedBytesPerSecond: number | null;
};

export function OssTransferQueue({ onTaskStateChanged }: OssTransferQueueProps) {
  const desktopRuntimeAvailable = hasDesktopRuntime();
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(false);
  const [metrics, setMetrics] = useState<Record<string, TransferMetric>>({});
  const tasksQuery = useQuery({
    queryKey: ossTransferTasksQueryKey,
    queryFn: listOssTransferTasks,
    enabled: desktopRuntimeAvailable,
    refetchInterval: desktopRuntimeAvailable ? 5_000 : false,
  });
  const cancelMutation = useMutation({
    mutationFn: (operationId: string) => cancelOssTransfer({ operationId }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ossTransferTasksQueryKey });
      toast.success("已请求取消传输");
    },
    onError: (error) => toast.error(getErrorSummary(error).message),
  });
  const resumeMutation = useMutation({
    mutationFn: (operationId: string) => resumeOssTransfer({ operationId }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ossTransferTasksQueryKey });
      toast.success("已恢复传输");
    },
    onError: (error) => toast.error(getErrorSummary(error).message),
  });
  const confirmMutation = useMutation({
    mutationFn: (operationId: string) => confirmOssTransfer({ operationId }),
    onSuccess: (task) => {
      void queryClient.invalidateQueries({ queryKey: ossTransferTasksQueryKey });
      if (task.status === "completed") {
        toast.success("已确认远端对象完成");
      } else {
        toast.info("未发现已完成对象，任务已恢复为可操作状态");
      }
    },
    onError: (error) => toast.error(getErrorSummary(error).message),
  });

  useEffect(() => {
    if (!desktopRuntimeAvailable) {
      return;
    }
    let disposed = false;
    let unlistenProgress: (() => void) | undefined;
    let unlistenState: (() => void) | undefined;
    const updateTask = (task: OssTransferTaskView) => {
      queryClient.setQueryData<OssTransferTaskView[]>(ossTransferTasksQueryKey, (current) => {
        const next = current ?? [];
        const index = next.findIndex((item) => item.id === task.id);
        if (index < 0) {
          return [task, ...next];
        }
        const result = [...next];
        result[index] = task;
        return result;
      });
      setMetrics((current) => {
        const timestamp = Date.parse(task.updatedAt);
        if (!Number.isFinite(timestamp)) {
          return current;
        }
        const previous = current[task.id];
        if (task.status === "queued" && task.bytesCompleted === 0) {
          if (!previous) return current;
          const next = { ...current };
          delete next[task.id];
          return next;
        }
        if (previous && timestamp <= previous.timestamp) {
          return current;
        }
        const elapsedSeconds = previous ? (timestamp - previous.timestamp) / 1_000 : 0;
        const deltaBytes = previous ? task.bytesCompleted - previous.bytes : 0;
        const speedBytesPerSecond = elapsedSeconds > 0 && deltaBytes > 0
          ? deltaBytes / elapsedSeconds
          : previous?.speedBytesPerSecond ?? null;
        return {
          ...current,
          [task.id]: { bytes: task.bytesCompleted, timestamp, speedBytesPerSecond },
        };
      });
    };
    const updateStateTask = (task: OssTransferTaskView) => {
      updateTask(task);
      onTaskStateChanged?.(task);
    };
    void (async () => {
      try {
        const [progressListener, stateListener] = await Promise.all([
          listenToOssTransferProgressEvents(updateTask),
          listenToOssTransferStateEvents(updateStateTask),
        ]);
        if (disposed) {
          progressListener();
          stateListener();
          return;
        }
        unlistenProgress = progressListener;
        unlistenState = stateListener;
      } catch (error) {
        console.error("Failed to listen OSS transfer events", getErrorSummary(error));
      }
    })();
    return () => {
      disposed = true;
      unlistenProgress?.();
      unlistenState?.();
    };
  }, [desktopRuntimeAvailable, onTaskStateChanged, queryClient]);

  const activeCount = useMemo(
    () =>
      (tasksQuery.data ?? []).filter((task) =>
        ["queued", "running", "cancelling"].includes(task.status),
      ).length,
    [tasksQuery.data],
  );
  const resumableCount = useMemo(
    () => (tasksQuery.data ?? []).filter((task) => ["paused", "failed"].includes(task.status)).length,
    [tasksQuery.data],
  );

  if (!desktopRuntimeAvailable) {
    return null;
  }

  return (
    <>
      <div className="oss-surface-muted oss-border border-t px-4 py-2.5">
        <div className="flex items-center justify-between gap-3">
          <div className="flex min-w-0 items-center gap-2">
            <CloudUploadOutlined className="oss-text-accent" />
            <Typography.Text className="oss-text-primary text-[12px] font-medium">
              传输队列
            </Typography.Text>
            {activeCount ? <Tag color="processing">{activeCount} 进行中</Tag> : null}
            {resumableCount ? <Tag color="warning">{resumableCount} 可恢复</Tag> : null}
          </div>
          <Button onClick={() => setOpen(true)} size="small" type="link">
            查看全部
          </Button>
        </div>
      </div>
      <Drawer
        destroyOnClose={false}
        onClose={() => setOpen(false)}
        open={open}
        title="OSS 传输队列"
        width={520}
      >
        {tasksQuery.error ? (
          <Typography.Text type="danger">{getErrorSummary(tasksQuery.error).message}</Typography.Text>
        ) : tasksQuery.isLoading ? (
          <div className="py-12 text-center">
            <Typography.Text type="secondary">正在读取传输任务…</Typography.Text>
          </div>
        ) : tasksQuery.data?.length ? (
          <div className="space-y-2">
            {tasksQuery.data.map((task) => (
              <TransferTaskItem
                cancelLoading={cancelMutation.isPending && cancelMutation.variables === task.id}
                confirmLoading={confirmMutation.isPending && confirmMutation.variables === task.id}
                key={task.id}
                onCancel={() => cancelMutation.mutate(task.id)}
                onConfirm={() => confirmMutation.mutate(task.id)}
                onResume={() => resumeMutation.mutate(task.id)}
                resumeLoading={resumeMutation.isPending && resumeMutation.variables === task.id}
                metric={metrics[task.id]}
                task={task}
              />
            ))}
          </div>
        ) : (
          <Empty description="还没有传输任务" image={Empty.PRESENTED_IMAGE_SIMPLE} />
        )}
      </Drawer>
    </>
  );
}

function TransferTaskItem({
  task,
  cancelLoading,
  confirmLoading,
  metric,
  resumeLoading,
  onCancel,
  onConfirm,
  onResume,
}: {
  task: OssTransferTaskView;
  cancelLoading: boolean;
  confirmLoading: boolean;
  resumeLoading: boolean;
  metric?: TransferMetric;
  onCancel: () => void;
  onConfirm: () => void;
  onResume: () => void;
}) {
  const percent = task.totalBytes > 0
    ? Math.min(100, Math.round((task.bytesCompleted / task.totalBytes) * 100))
    : task.status === "completed"
      ? 100
      : 0;
  const active = ["queued", "running", "cancelling"].includes(task.status);
  const resumable = ["paused", "failed"].includes(task.status);
  const needsConfirmation = task.status === "needs_confirmation";
  const Icon = task.operation === "download" ? CloudDownloadOutlined : CloudUploadOutlined;
  return (
    <div className="oss-surface-muted oss-border rounded-[12px] border px-3 py-3">
      <div className="flex items-start gap-2">
        <Icon className="oss-text-accent mt-0.5" />
        <div className="min-w-0 flex-1">
          <Typography.Text className="oss-text-primary block truncate text-[12px] font-medium" title={task.objectKey}>
            {task.objectKey}
          </Typography.Text>
          <Typography.Text className="oss-text-secondary mt-0.5 block truncate text-[11px]" title={`${task.accountDisplayName} · ${task.targetDisplayName} · ${task.bucket}`}>
            {task.accountDisplayName} · {task.targetDisplayName} · {task.bucket} · {task.region}
          </Typography.Text>
          <Typography.Text className="oss-text-tertiary mt-0.5 block truncate text-[11px]" title={task.localPath}>
            {task.operation === "download" ? "下载" : "上传"} · {task.localPath}
          </Typography.Text>
        </div>
        <StatusTag status={task.status} />
      </div>
      <Progress
        className="!mb-0 !mt-2"
        percent={percent}
        showInfo
        size="small"
        status={task.status === "failed" ? "exception" : task.status === "completed" ? "success" : "active"}
      />
      <div className="oss-text-tertiary mt-1 flex items-center justify-between gap-2 text-[10px]">
        <span>{formatBytes(task.bytesCompleted)} / {formatBytes(task.totalBytes)}</span>
        <span>{formatSpeed(metric?.speedBytesPerSecond)}{task.status === "running" ? ` · ${formatEta(task, metric)}` : ""}</span>
      </div>
      {task.lastErrorMessage ? (
        <Typography.Text className="oss-text-danger mt-1 block text-[11px]">
          {task.lastErrorCode ? `[${task.lastErrorCode}] ` : ""}{task.lastErrorMessage}
        </Typography.Text>
      ) : null}
      <div className="mt-2 flex justify-end gap-1.5">
        {needsConfirmation ? (
          <Button loading={confirmLoading} onClick={onConfirm} size="small" type="primary">
            检查远端
          </Button>
        ) : null}
        {active ? (
          <Button
            disabled={task.status === "cancelling"}
            icon={<PauseCircleOutlined />}
            loading={cancelLoading}
            onClick={onCancel}
            size="small"
          >
            取消
          </Button>
        ) : null}
        {resumable ? (
          <>
            <Popconfirm
              cancelText="返回"
              okButtonProps={{ danger: true }}
              okText="放弃并清理"
              onConfirm={onCancel}
              title="放弃这个可恢复任务？"
              description="如果存在未完成的 OSS 分片，系统会尝试调用 Abort 清理。"
            >
              <Button disabled={cancelLoading} loading={cancelLoading} size="small">
                放弃任务
              </Button>
            </Popconfirm>
            <Button icon={<CheckCircleOutlined />} loading={resumeLoading} onClick={onResume} size="small" type="primary">
              恢复
            </Button>
          </>
        ) : null}
      </div>
    </div>
  );
}

function StatusTag({ status }: { status: string }) {
  const config: Record<string, { color: string; label: string }> = {
    queued: { color: "processing", label: "排队中" },
    running: { color: "processing", label: "传输中" },
    cancelling: { color: "warning", label: "取消中" },
    paused: { color: "warning", label: "已暂停" },
    failed: { color: "error", label: "失败" },
    completed: { color: "success", label: "已完成" },
    cancelled: { color: "default", label: "已取消" },
    needs_confirmation: { color: "warning", label: "待确认" },
  };
  const resolved = config[status] ?? { color: "default", label: status };
  return <Tag color={resolved.color}>{resolved.label}</Tag>;
}

function formatBytes(value: number) {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  if (value < 1024 * 1024 * 1024) return `${(value / (1024 * 1024)).toFixed(1)} MB`;
  return `${(value / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

function formatSpeed(value?: number | null) {
  return value && value > 0 ? `${formatBytes(value)}/s` : "速度计算中";
}

function formatEta(task: OssTransferTaskView, metric?: TransferMetric) {
  if (!metric?.speedBytesPerSecond || task.totalBytes <= task.bytesCompleted) return "剩余时间计算中";
  const seconds = Math.ceil((task.totalBytes - task.bytesCompleted) / metric.speedBytesPerSecond);
  if (seconds < 60) return `约 ${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  const rest = seconds % 60;
  return `约 ${minutes}m ${rest}s`;
}
