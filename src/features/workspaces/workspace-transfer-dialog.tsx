import {
  CheckCircleOutlined,
  ExclamationCircleOutlined,
  FolderOpenOutlined,
  ReloadOutlined,
} from "@ant-design/icons";
import { useMutation } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { Alert, Button, Modal, Select, Spin, Tag, Typography } from "antd";
import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";

import { cn } from "@/lib/cn";
import {
  applyWorkspaceListImport,
  getErrorSummary,
  previewWorkspaceListImport,
} from "@/lib/command-client";
import type {
  WorkspaceImportAction,
  WorkspaceImportApplyResult,
  WorkspaceImportItemPreview,
  WorkspaceImportProjectPreview,
  WorkspaceImportRelocation,
  WorkspaceImportPreview,
} from "@/types/backend";

import {
  buildInitialWorkspaceImportActions,
  buildWorkspaceImportSelections,
  getVirtualWorkspaceRange,
  summarizeWorkspaceImportActions,
  type WorkspaceImportActionMap,
} from "./workspace-transfer-model";

const WORKSPACE_ROW_HEIGHT = 84;
const WORKSPACE_LIST_VIEWPORT_HEIGHT = 352;
const IMPORT_ACTION_SELECT_WIDTH = 168;

type WorkspaceTransferDialogProps = {
  open: boolean;
  sourcePath: string | null;
  onClose: () => void;
  onImported: (result: WorkspaceImportApplyResult) => Promise<void> | void;
};

export function WorkspaceTransferDialog({
  open: dialogOpen,
  sourcePath,
  onClose,
  onImported,
}: WorkspaceTransferDialogProps) {
  const [preview, setPreview] = useState<WorkspaceImportPreview | null>(null);
  const [actions, setActions] = useState<WorkspaceImportActionMap>({});
  const [relocations, setRelocations] = useState<WorkspaceImportRelocation[]>([]);
  const [selectedWorkspaceIndex, setSelectedWorkspaceIndex] = useState<number | null>(null);
  const [workspaceScrollTop, setWorkspaceScrollTop] = useState(0);

  const previewMutation = useMutation({
    mutationFn: previewWorkspaceListImport,
    onSuccess: (nextPreview, variables) => {
      if (!dialogOpen || variables.sourcePath !== sourcePath) {
        return;
      }
      setPreview(nextPreview);
      setActions((current) => buildInitialWorkspaceImportActions(nextPreview, current));
      setSelectedWorkspaceIndex((current) =>
        nextPreview.items.some((item) => item.workspaceIndex === current)
          ? current
          : (nextPreview.items[0]?.workspaceIndex ?? null),
      );
    },
  });
  const applyMutation = useMutation({ mutationFn: applyWorkspaceListImport });

  useEffect(() => {
    if (!dialogOpen || !sourcePath) {
      return;
    }
    setPreview(null);
    setActions({});
    setRelocations([]);
    setSelectedWorkspaceIndex(null);
    setWorkspaceScrollTop(0);
    previewMutation.mutate({ sourcePath, relocations: [] });
    // Preview is intentionally restarted only when a new file is opened.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [dialogOpen, sourcePath]);

  const selections = useMemo(
    () => (preview ? buildWorkspaceImportSelections(preview, actions) : []),
    [actions, preview],
  );
  const actionSummary = useMemo(
    () => summarizeWorkspaceImportActions(selections),
    [selections],
  );
  const selectedItem = preview?.items.find(
    (item) => item.workspaceIndex === selectedWorkspaceIndex,
  );
  const virtualRange = getVirtualWorkspaceRange(
    preview?.items.length ?? 0,
    workspaceScrollTop,
    WORKSPACE_LIST_VIEWPORT_HEIGHT,
    WORKSPACE_ROW_HEIGHT,
  );
  const visibleItems = preview?.items.slice(
    virtualRange.startIndex,
    virtualRange.endIndex,
  );
  const previewError = previewMutation.error ? getErrorSummary(previewMutation.error) : null;
  const applyError = applyMutation.error ? getErrorSummary(applyMutation.error) : null;
  const isBusy = previewMutation.isPending || applyMutation.isPending;
  const changeCount = actionSummary.create + actionSummary.update;

  function runPreview(nextRelocations = relocations) {
    if (!sourcePath || previewMutation.isPending || applyMutation.isPending) {
      return;
    }
    previewMutation.mutate({ sourcePath, relocations: nextRelocations });
  }

  async function handleRelocateProject(item: WorkspaceImportItemPreview, projectIndex: number) {
    if (!sourcePath || isBusy) {
      return;
    }
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "重新定位项目目录",
      });
      if (typeof selected !== "string") {
        return;
      }
      const nextRelocations = upsertRelocation(relocations, {
        workspaceIndex: item.workspaceIndex,
        projectIndex,
        path: selected,
      });
      setRelocations(nextRelocations);
      runPreview(nextRelocations);
    } catch (error) {
      const resolved = getErrorSummary(error);
      toast.error("无法选择新的项目目录", {
        description: `${resolved.message}（${resolved.code}）`,
      });
    }
  }

  function handleResetRelocation(item: WorkspaceImportItemPreview, projectIndex: number) {
    if (isBusy) {
      return;
    }
    const nextRelocations = relocations.filter(
      (relocation) =>
        !(
          relocation.workspaceIndex === item.workspaceIndex &&
          relocation.projectIndex === projectIndex
        ),
    );
    setRelocations(nextRelocations);
    runPreview(nextRelocations);
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
        relocations,
      });
      try {
        await onImported(result);
      } catch (error) {
        const resolved = getErrorSummary(error);
        toast.warning("导入已完成，但列表刷新失败", {
          description: `${resolved.message}（${resolved.code}）。请手动刷新工作区列表。`,
          duration: 8_000,
        });
      }
      toast.success("工作区项目列表已导入", {
        description: `新建 ${result.createdWorkspaceCount}，更新 ${result.updatedWorkspaceCount}，跳过 ${result.skippedWorkspaceCount}`,
      });
      if (result.warnings.length) {
        toast.warning("导入完成，但有需要留意的提示", {
          description: result.warnings.join("；"),
          duration: 8_000,
        });
      }
      onClose();
    } catch {
      // The mutation error is rendered inline and remains retryable.
    }
  }

  return (
    <Modal
      centered
      destroyOnHidden
      footer={
        <div className="flex flex-wrap items-center justify-between gap-2">
          <Typography.Text className="text-[11px] text-[#667085]">
            将新建 {actionSummary.create}、更新 {actionSummary.update}、跳过 {actionSummary.skip}
          </Typography.Text>
          <div className="flex items-center gap-2">
            <Button disabled={applyMutation.isPending} onClick={onClose}>
              取消
            </Button>
            <Button
              disabled={!sourcePath || applyMutation.isPending}
              icon={<ReloadOutlined />}
              loading={previewMutation.isPending}
              onClick={() => runPreview()}
            >
              重新预检
            </Button>
            <Button
              disabled={!preview || Boolean(previewError) || previewMutation.isPending || changeCount === 0}
              loading={applyMutation.isPending}
              onClick={() => void handleApply()}
              type="primary"
            >
              {changeCount === 0 ? "没有待导入项" : `确认导入 ${changeCount} 项`}
            </Button>
          </div>
        </div>
      }
      keyboard={!applyMutation.isPending}
      maskClosable={!isBusy}
      onCancel={applyMutation.isPending ? undefined : onClose}
      open={dialogOpen}
      title="导入工作区项目列表"
      width={980}
    >
      <div className="space-y-3 pt-1">
        <Alert
          description={
            <div className="space-y-1">
              <Typography.Text className="block break-all text-[11px] text-[#475467]">
                {sourcePath}
              </Typography.Text>
              <Typography.Text className="block text-[11px] leading-5 text-[#667085]">
                导入只合并本地配置，不会删除现有工作区或磁盘文件。绝对路径与启动命令会在写入前由 Rust 再次校验。
              </Typography.Text>
            </div>
          }
          message="先预检，再选择处理方式"
          showIcon
          type="info"
        />

        {previewError ? (
          <Alert
            action={
              <Button onClick={() => runPreview()} size="small">
                重试
              </Button>
            }
            description={`${previewError.message}（${previewError.code}）`}
            message="无法预检导入文件"
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

        {preview ? (
          <>
            <div className="flex flex-wrap items-center gap-2 rounded-[10px] border border-[#e6edf5] bg-[#fbfcfe] px-3 py-2">
              <Tag color="blue">共 {preview.summary.totalWorkspaceCount} 个工作区</Tag>
              <Tag color="success">可新建 {preview.summary.readyWorkspaceCount}</Tag>
              <Tag color="default">已存在 {preview.summary.existingWorkspaceCount}</Tag>
              <Tag color={preview.summary.blockedWorkspaceCount ? "error" : "default"}>
                被阻止 {preview.summary.blockedWorkspaceCount}
              </Tag>
              {preview.summary.missingProjectCount ? (
                <Tag color="warning">缺失目录 {preview.summary.missingProjectCount}</Tag>
              ) : null}
              {preview.summary.invalidProjectCount ? (
                <Tag color="error">无效项目 {preview.summary.invalidProjectCount}</Tag>
              ) : null}
              <Typography.Text className="ml-auto text-[10px] text-[#98a2b3]">
                格式版本 v{preview.schemaVersion}
              </Typography.Text>
            </div>

            {preview.warnings.length ? (
              <Alert
                description={preview.warnings.join("；")}
                message="部分逻辑引用无法在本机恢复"
                showIcon
                type="warning"
              />
            ) : null}

            <Spin spinning={previewMutation.isPending} tip="正在重新预检目录与冲突...">
              <div className="grid min-h-[352px] grid-cols-1 gap-3 lg:grid-cols-[minmax(280px,0.9fr)_minmax(0,1.35fr)]">
                <div className="overflow-hidden rounded-[12px] border border-[#e6edf5] bg-white">
                  <div className="border-b border-[#e6edf5] px-3 py-2">
                    <Typography.Text className="text-[12px] font-semibold text-[#344054]">
                      工作区列表
                    </Typography.Text>
                  </div>
                  <div
                    aria-label="待导入工作区列表"
                    className="overflow-y-auto"
                    onScroll={(event) => setWorkspaceScrollTop(event.currentTarget.scrollTop)}
                    style={{ height: WORKSPACE_LIST_VIEWPORT_HEIGHT }}
                    tabIndex={0}
                  >
                    <div className="relative" style={{ height: virtualRange.totalHeight }}>
                      <div
                        className="absolute inset-x-0 top-0 px-1.5 py-1"
                        style={{ transform: `translateY(${virtualRange.offsetTop}px)` }}
                      >
                        {visibleItems?.map((item) => (
                          <WorkspaceImportListItem
                            action={actions[item.workspaceIndex] ?? item.recommendedAction}
                            item={item}
                            key={item.workspaceIndex}
                            onSelect={() => setSelectedWorkspaceIndex(item.workspaceIndex)}
                            selected={item.workspaceIndex === selectedWorkspaceIndex}
                          />
                        ))}
                      </div>
                    </div>
                  </div>
                </div>

                <WorkspaceImportDetail
                  action={
                    selectedItem
                      ? (actions[selectedItem.workspaceIndex] ?? selectedItem.recommendedAction)
                      : "skip"
                  }
                  item={selectedItem}
                  onActionChange={(action) => {
                    if (!selectedItem) {
                      return;
                    }
                    setActions((current) => ({
                      ...current,
                      [selectedItem.workspaceIndex]: action,
                    }));
                  }}
                  onRelocate={(projectIndex) =>
                    selectedItem
                      ? void handleRelocateProject(selectedItem, projectIndex)
                      : undefined
                  }
                  onResetRelocation={(projectIndex) => {
                    if (selectedItem) {
                      handleResetRelocation(selectedItem, projectIndex);
                    }
                  }}
                  relocationKeys={new Set(
                    relocations.map(
                      (relocation) =>
                        `${relocation.workspaceIndex}:${relocation.projectIndex}`,
                    ),
                  )}
                />
              </div>
            </Spin>
          </>
        ) : !previewError ? (
          <div className="flex min-h-[320px] items-center justify-center">
            <Spin tip="正在读取文件并核对本机目录..." />
          </div>
        ) : null}
      </div>
    </Modal>
  );
}

function WorkspaceImportListItem({
  item,
  action,
  selected,
  onSelect,
}: {
  item: WorkspaceImportItemPreview;
  action: WorkspaceImportAction;
  selected: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      className={cn(
        "mb-2 flex h-[76px] w-full items-start gap-2 rounded-[10px] border px-2.5 py-2 text-left transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#24a8d8]",
        selected
          ? "border-[#8fd4ec] bg-[#f4fbfe]"
          : "border-transparent bg-transparent hover:border-[#d9e2ec] hover:bg-[#f8fafc]",
      )}
      onClick={onSelect}
      type="button"
    >
      <ImportStatusIcon item={item} />
      <span className="min-w-0 flex-1">
        <span className="block truncate text-[12px] font-semibold text-[#1f2937]">
          {item.name}
        </span>
        <span className="mt-1 block truncate text-[10px] text-[#667085]">
          {item.projects[0]?.resolvedPath ?? "没有项目目录"}
        </span>
        <span className="mt-1 flex items-center gap-1.5 text-[10px] text-[#98a2b3]">
          <ImportActionTag action={action} />
          {item.projects.length} 个项目
        </span>
      </span>
    </button>
  );
}

function WorkspaceImportDetail({
  item,
  action,
  relocationKeys,
  onActionChange,
  onRelocate,
  onResetRelocation,
}: {
  item?: WorkspaceImportItemPreview;
  action: WorkspaceImportAction;
  relocationKeys: Set<string>;
  onActionChange: (action: WorkspaceImportAction) => void;
  onRelocate: (projectIndex: number) => void;
  onResetRelocation: (projectIndex: number) => void;
}) {
  if (!item) {
    return (
      <div className="flex min-h-[352px] items-center justify-center rounded-[12px] border border-dashed border-[#d9e2ec] bg-[#fbfcfe] text-[12px] text-[#98a2b3]">
        请选择一个工作区查看预检详情
      </div>
    );
  }
  const options = [
    item.canCreate ? { value: "create", label: "新建工作区" } : null,
    item.canUpdate ? { value: "update", label: "更新现有配置" } : null,
    { value: "skip", label: "跳过" },
  ].filter(Boolean) as Array<{ value: WorkspaceImportAction; label: string }>;

  return (
    <div className="min-h-[352px] overflow-hidden rounded-[12px] border border-[#e6edf5] bg-white">
      <div className="flex flex-wrap items-start justify-between gap-2 border-b border-[#e6edf5] px-3 py-2.5">
        <div className="min-w-0 flex-[1_1_220px]">
          <Typography.Text className="block truncate text-[13px] font-semibold text-[#1f2937]">
            {item.name}
          </Typography.Text>
          <Typography.Text className="mt-0.5 block text-[10px] text-[#667085]">
            {item.existingWorkspaceName
              ? `匹配现有工作区：${item.existingWorkspaceName}`
              : item.suggestedName !== item.name
                ? `将保存为：${item.suggestedName}`
                : "未发现同路径工作区"}
          </Typography.Text>
        </div>
        <Select<WorkspaceImportAction>
          aria-label="导入处理方式"
          className="ml-auto shrink-0"
          onChange={onActionChange}
          options={options}
          popupMatchSelectWidth={IMPORT_ACTION_SELECT_WIDTH}
          size="small"
          style={{ width: IMPORT_ACTION_SELECT_WIDTH }}
          value={action}
          variant="outlined"
        />
      </div>

      <div className="max-h-[292px] space-y-2 overflow-y-auto p-3">
        {action === "update" ? (
          <Alert
            description="会更新工作区备注、图标、颜色与归档状态；覆盖匹配项目的 IDE/终端/Codex 配置和启动任务。现有工作区中未被文件匹配的其他项目不会删除。"
            message="更新会覆盖现有项目配置"
            showIcon
            type="warning"
          />
        ) : null}
        {item.status === "blocked" ? (
          <Alert
            description="请重新定位缺失目录或修复文件中的重复/无效路径；在此之前该工作区只能跳过。"
            message="当前工作区不能导入"
            showIcon
            type="error"
          />
        ) : null}
        {item.warnings.map((warning) => (
          <Alert key={warning} message={warning} showIcon type="warning" />
        ))}
        {item.projects.map((project) => {
          const relocationKey = `${item.workspaceIndex}:${project.projectIndex}`;
          return (
            <ProjectPreviewRow
              hasRelocation={relocationKeys.has(relocationKey)}
              key={project.projectIndex}
              onRelocate={() => onRelocate(project.projectIndex)}
              onResetRelocation={() => onResetRelocation(project.projectIndex)}
              project={project}
            />
          );
        })}
      </div>
    </div>
  );
}

function ProjectPreviewRow({
  project,
  hasRelocation,
  onRelocate,
  onResetRelocation,
}: {
  project: WorkspaceImportProjectPreview;
  hasRelocation: boolean;
  onRelocate: () => void;
  onResetRelocation: () => void;
}) {
  return (
    <div className="rounded-[10px] border border-[#e6edf5] bg-[#fbfcfe] px-3 py-2.5">
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <Typography.Text className="truncate text-[12px] font-semibold text-[#344054]">
              {project.name}
            </Typography.Text>
            <ProjectStatusTag status={project.status} />
          </div>
          <Typography.Text
            className="mt-1 block break-all text-[10px] leading-4 text-[#667085]"
            title={project.resolvedPath}
          >
            {project.resolvedPath}
          </Typography.Text>
          {project.message ? (
            <Typography.Text className="mt-1 block text-[10px] leading-4 text-[#7b8794]">
              {project.message}
            </Typography.Text>
          ) : null}
        </div>
        <div className="flex shrink-0 items-center gap-1">
          {hasRelocation ? (
            <Button onClick={onResetRelocation} size="small" type="text">
              使用原路径
            </Button>
          ) : null}
          <Button icon={<FolderOpenOutlined />} onClick={onRelocate} size="small">
            重新定位
          </Button>
        </div>
      </div>
    </div>
  );
}

function ImportStatusIcon({ item }: { item: WorkspaceImportItemPreview }) {
  return item.status === "blocked" ? (
    <ExclamationCircleOutlined className="mt-0.5 text-[14px] text-[#cf5a4a]" />
  ) : (
    <CheckCircleOutlined
      className={cn(
        "mt-0.5 text-[14px]",
        item.status === "existing" ? "text-[#667085]" : "text-[#027a48]",
      )}
    />
  );
}

function ImportActionTag({ action }: { action: WorkspaceImportAction }) {
  const config = {
    create: { color: "success", label: "新建" },
    update: { color: "warning", label: "更新" },
    skip: { color: "default", label: "跳过" },
  }[action];
  return (
    <Tag bordered={false} color={config.color} className="m-0 px-1.5 text-[9px] leading-4">
      {config.label}
    </Tag>
  );
}

function ProjectStatusTag({ status }: { status: WorkspaceImportProjectPreview["status"] }) {
  const config = {
    ready: { color: "success", label: "可导入" },
    existing: { color: "default", label: "已存在" },
    missing: { color: "warning", label: "目录缺失" },
    invalid: { color: "error", label: "无效" },
  }[status];
  return (
    <Tag bordered={false} color={config.color} className="m-0 text-[9px]">
      {config.label}
    </Tag>
  );
}

function upsertRelocation(
  relocations: WorkspaceImportRelocation[],
  next: WorkspaceImportRelocation,
) {
  return [
    ...relocations.filter(
      (relocation) =>
        !(
          relocation.workspaceIndex === next.workspaceIndex &&
          relocation.projectIndex === next.projectIndex
        ),
    ),
    next,
  ];
}
