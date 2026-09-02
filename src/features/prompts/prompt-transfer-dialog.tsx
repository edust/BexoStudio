import {
  CheckCircleOutlined,
  CopyOutlined,
  ExclamationCircleOutlined,
  InfoCircleOutlined,
  ReloadOutlined,
} from "@ant-design/icons";
import { useMutation } from "@tanstack/react-query";
import { Alert, Button, Empty, Modal, Select, Spin, Tag, Typography } from "antd";
import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";

import { cn } from "@/lib/cn";
import { getErrorSummary } from "@/lib/command-client";
import {
  applyPromptListImport,
  previewPromptListImport,
} from "@/queries/prompts";
import type {
  PromptImportAction,
  PromptImportApplyResult,
  PromptImportItemPreview,
  PromptImportPreview,
} from "@/types/backend";

import {
  buildInitialPromptImportActions,
  buildPromptImportSelections,
  getVirtualPromptImportRange,
  summarizePromptImportActions,
  type PromptImportActionMap,
} from "./prompt-transfer-model";

const PROMPT_IMPORT_ROW_HEIGHT = 104;
const PROMPT_IMPORT_VIEWPORT_HEIGHT = 416;
const PROMPT_IMPORT_ACTION_WIDTH = 142;

type PromptTransferDialogProps = {
  open: boolean;
  sourcePath: string | null;
  onClose: () => void;
  onImported: (result: PromptImportApplyResult) => Promise<void> | void;
};

export function PromptTransferDialog({
  open: dialogOpen,
  sourcePath,
  onClose,
  onImported,
}: PromptTransferDialogProps) {
  const [preview, setPreview] = useState<PromptImportPreview | null>(null);
  const [actions, setActions] = useState<PromptImportActionMap>({});
  const [scrollTop, setScrollTop] = useState(0);

  const previewMutation = useMutation({
    mutationFn: previewPromptListImport,
    onSuccess: (nextPreview, variables) => {
      if (!dialogOpen || variables.sourcePath !== sourcePath) {
        return;
      }
      setPreview(nextPreview);
      setActions((current) => buildInitialPromptImportActions(nextPreview, current));
      setScrollTop(0);
    },
  });
  const applyMutation = useMutation({ mutationFn: applyPromptListImport });

  useEffect(() => {
    if (!dialogOpen || !sourcePath) {
      return;
    }
    setPreview(null);
    setActions({});
    setScrollTop(0);
    applyMutation.reset();
    previewMutation.mutate({ sourcePath });
    // Preview is intentionally restarted only when a new source file is opened.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [dialogOpen, sourcePath]);

  const selections = useMemo(
    () => (preview ? buildPromptImportSelections(preview, actions) : []),
    [actions, preview],
  );
  const actionSummary = useMemo(
    () => summarizePromptImportActions(selections),
    [selections],
  );
  const virtualRange = getVirtualPromptImportRange(
    preview?.items.length ?? 0,
    scrollTop,
    PROMPT_IMPORT_VIEWPORT_HEIGHT,
    PROMPT_IMPORT_ROW_HEIGHT,
  );
  const visibleItems = preview?.items.slice(
    virtualRange.startIndex,
    virtualRange.endIndex,
  );
  const previewError = previewMutation.error ? getErrorSummary(previewMutation.error) : null;
  const applyError = applyMutation.error ? getErrorSummary(applyMutation.error) : null;
  const isBusy = previewMutation.isPending || applyMutation.isPending;
  const changeCount = actionSummary.create + actionSummary.update;

  function runPreview() {
    if (!sourcePath || isBusy) {
      return;
    }
    applyMutation.reset();
    previewMutation.mutate({ sourcePath });
  }

  async function handleApply() {
    if (!sourcePath || !preview || previewError || isBusy || changeCount === 0) {
      return;
    }
    try {
      const result = await applyMutation.mutateAsync({
        sourcePath,
        expectedFileHash: preview.fileHash,
        selections,
      });
      try {
        await onImported(result);
      } catch (error) {
        const resolved = getErrorSummary(error);
        toast.warning("导入已完成，但列表同步失败", {
          description: `${resolved.message}（${resolved.code}）。请手动刷新 Prompts。`,
          duration: 8_000,
        });
      }
      toast.success("Prompts 已导入", {
        description: `新建 ${result.createdPromptCount}，更新 ${result.updatedPromptCount}，跳过 ${result.skippedPromptCount}`,
      });
      onClose();
    } catch (error) {
      const resolved = getErrorSummary(error);
      console.warn("[prompts] import apply failed and remains retryable", {
        code: resolved.code,
        message: resolved.message,
      });
    }
  }

  return (
    <Modal
      centered
      destroyOnHidden
      footer={
        <div className="flex flex-wrap items-center justify-between gap-2">
          <Typography.Text className="text-[11px] text-muted-foreground">
            将新建 {actionSummary.create}、更新 {actionSummary.update}、跳过 {actionSummary.skip}
          </Typography.Text>
          <div className="flex items-center gap-2">
            <Button disabled={isBusy} onClick={onClose}>
              取消
            </Button>
            <Button
              disabled={!sourcePath || applyMutation.isPending}
              icon={<ReloadOutlined />}
              loading={previewMutation.isPending}
              onClick={runPreview}
            >
              重新预检
            </Button>
            <Button
              disabled={
                !preview ||
                Boolean(previewError) ||
                previewMutation.isPending ||
                changeCount === 0
              }
              loading={applyMutation.isPending}
              onClick={() => void handleApply()}
              type="primary"
            >
              {changeCount === 0 ? "没有待导入项" : `确认导入 ${changeCount} 项`}
            </Button>
          </div>
        </div>
      }
      keyboard={!isBusy}
      maskClosable={!isBusy}
      onCancel={isBusy ? undefined : onClose}
      open={dialogOpen}
      title="导入常用 Prompts"
      width={900}
    >
      <div className="space-y-3 pt-1">
        <Alert
          description={
            <div className="space-y-1">
              <Typography.Text className="allow-text-selection block break-all text-[11px] text-muted-foreground">
                {sourcePath}
              </Typography.Text>
              <Typography.Text className="block text-[11px] leading-5 text-muted-foreground">
                只合并已保存的 Prompt，不会删除本地记录，也不会更改快捷粘贴热键。相同 ID 的内容冲突默认跳过。
              </Typography.Text>
            </div>
          }
          message="先预检，再确认每条记录的处理方式"
          showIcon
          type="info"
        />

        {previewError ? (
          <Alert
            action={
              <Button disabled={isBusy} onClick={runPreview} size="small">
                重试
              </Button>
            }
            description={`${previewError.message}（${previewError.code}）`}
            message="无法预检 Prompts 文件"
            showIcon
            type="error"
          />
        ) : null}
        {applyError ? (
          <Alert
            description={`${applyError.message}（${applyError.code}）`}
            message="导入未完成，数据库没有写入部分结果"
            showIcon
            type="error"
          />
        ) : null}
        {actionSummary.update > 0 ? (
          <Alert
            description="更新只针对相同 UUID 的记录，并保持本地排序和快捷粘贴绑定；标题和完整正文会被导入文件覆盖。"
            message={`已选择更新 ${actionSummary.update} 条现有 Prompt`}
            showIcon
            type="warning"
          />
        ) : null}

        {preview ? (
          <>
            <div className="bexo-panel-muted flex flex-wrap items-center gap-2 rounded-[10px] px-3 py-2">
              <Tag color="blue">共 {preview.summary.totalPromptCount} 条</Tag>
              <Tag color="success">可新建 {preview.summary.readyPromptCount}</Tag>
              <Tag>已存在 {preview.summary.existingPromptCount}</Tag>
              <Tag color={preview.summary.conflictPromptCount ? "warning" : "default"}>
                内容冲突 {preview.summary.conflictPromptCount}
              </Tag>
              <Tag color={preview.summary.duplicatePromptCount ? "processing" : "default"}>
                重复项 {preview.summary.duplicatePromptCount}
              </Tag>
              <Typography.Text className="ml-auto text-[10px] text-muted-foreground">
                格式版本 v{preview.schemaVersion}
              </Typography.Text>
            </div>

            <Spin spinning={previewMutation.isPending} tip="正在重新核对文件与本地冲突...">
              {preview.items.length ? (
                <div className="overflow-hidden rounded-[12px] border border-[color:var(--border)] bg-panel-elevated">
                  <div className="grid grid-cols-[minmax(0,1fr)_142px] items-center gap-3 border-b border-[color:var(--border)] px-3 py-2 text-[11px] font-semibold text-muted-foreground">
                    <span>Prompt 与预检状态</span>
                    <span>处理方式</span>
                  </div>
                  <div
                    aria-label="待导入 Prompt 列表"
                    className="overflow-y-auto"
                    onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
                    style={{ height: PROMPT_IMPORT_VIEWPORT_HEIGHT }}
                    tabIndex={0}
                  >
                    <div className="relative" style={{ height: virtualRange.totalHeight }}>
                      <div
                        className="absolute inset-x-0 top-0"
                        style={{ transform: `translateY(${virtualRange.offsetTop}px)` }}
                      >
                        {visibleItems?.map((item) => (
                          <PromptImportRow
                            action={actions[item.promptIndex] ?? item.recommendedAction}
                            busy={isBusy}
                            item={item}
                            key={`${item.promptIndex}:${item.id}`}
                            onActionChange={(action) => {
                              applyMutation.reset();
                              setActions((current) => ({
                                ...current,
                                [item.promptIndex]: action,
                              }));
                            }}
                          />
                        ))}
                      </div>
                    </div>
                  </div>
                </div>
              ) : (
                <div className="flex min-h-[240px] items-center justify-center rounded-[12px] border border-dashed border-[color:var(--border)] bg-panel-elevated">
                  <Empty description="文件中没有 Prompt；无需导入" image={Empty.PRESENTED_IMAGE_SIMPLE} />
                </div>
              )}
            </Spin>
          </>
        ) : !previewError ? (
          <div className="flex min-h-[320px] items-center justify-center">
            <Spin tip="正在读取文件并核对本地 Prompts..." />
          </div>
        ) : null}
      </div>
    </Modal>
  );
}

function PromptImportRow({
  item,
  action,
  busy,
  onActionChange,
}: {
  item: PromptImportItemPreview;
  action: PromptImportAction;
  busy: boolean;
  onActionChange: (action: PromptImportAction) => void;
}) {
  const options = [
    item.canCreate ? { value: "create", label: "新建" } : null,
    item.canUpdate ? { value: "update", label: "更新现有" } : null,
    { value: "skip", label: "跳过" },
  ].filter(Boolean) as Array<{ value: PromptImportAction; label: string }>;

  return (
    <div
      className="grid grid-cols-[minmax(0,1fr)_142px] items-center gap-3 border-b border-[color:var(--border)] px-3"
      style={{ height: PROMPT_IMPORT_ROW_HEIGHT }}
    >
      <div className="flex min-w-0 items-start gap-2.5">
        <PromptImportStatusIcon status={item.status} />
        <div className="min-w-0 flex-1">
          <div className="flex min-w-0 items-center gap-2">
            <Typography.Text className="truncate text-[12px] font-semibold text-foreground">
              {item.title}
            </Typography.Text>
            <PromptImportStatusTag status={item.status} />
          </div>
          <Typography.Text
            className="mt-1 block line-clamp-2 whitespace-pre-wrap break-words text-[10px] leading-4 text-muted-foreground"
            title={item.contentPreview}
          >
            {item.contentPreview}
          </Typography.Text>
          <Typography.Text className="mt-1 block truncate text-[10px] text-muted-foreground" title={item.message}>
            {item.message}
          </Typography.Text>
        </div>
      </div>
      <Select<PromptImportAction>
        aria-label={`“${item.title}”的导入处理方式`}
        disabled={busy || options.length === 1}
        onChange={onActionChange}
        options={options}
        popupMatchSelectWidth={PROMPT_IMPORT_ACTION_WIDTH}
        size="small"
        style={{ width: PROMPT_IMPORT_ACTION_WIDTH }}
        value={action}
      />
    </div>
  );
}

function PromptImportStatusIcon({ status }: { status: PromptImportItemPreview["status"] }) {
  const className = "mt-0.5 shrink-0 text-[14px]";
  switch (status) {
    case "ready":
      return <CheckCircleOutlined className={cn(className, "text-success")} />;
    case "conflict":
      return <ExclamationCircleOutlined className={cn(className, "text-warning")} />;
    case "duplicate":
      return <CopyOutlined className={cn(className, "text-accent")} />;
    case "existing":
      return <InfoCircleOutlined className={cn(className, "text-muted-foreground")} />;
  }
}

function PromptImportStatusTag({ status }: { status: PromptImportItemPreview["status"] }) {
  const config = {
    ready: { color: "success", label: "可新建" },
    existing: { color: "default", label: "已存在" },
    conflict: { color: "warning", label: "内容冲突" },
    duplicate: { color: "processing", label: "重复项" },
  }[status];
  return (
    <Tag bordered={false} className="m-0 shrink-0 px-1.5 text-[9px] leading-4" color={config.color}>
      {config.label}
    </Tag>
  );
}
