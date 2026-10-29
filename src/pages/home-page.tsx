import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  CaretRightOutlined,
  CopyOutlined,
  DeleteOutlined,
  EditOutlined,
  HolderOutlined,
  PlusOutlined,
} from "@ant-design/icons";
import {
  Alert,
  Button,
  Empty,
  Input,
  Modal,
  Popconfirm,
  Select,
  Spin,
  Switch,
  Tooltip,
  Typography,
} from "antd";
import { Reorder, useDragControls } from "motion/react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Controller, useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { toast } from "sonner";

import { PaneHeader } from "@/components/shell/pane-header";
import { ResourceBrowserPane } from "@/features/resource-browser/resource-browser-pane";
import { copyTextToClipboard, getClipboardErrorMessage } from "@/lib/clipboard";
import { cn } from "@/lib/cn";
import {
  deleteLaunchTask,
  getErrorSummary,
  hasDesktopRuntime,
  listWorkspaces,
  reorderLaunchTasks,
  runWorkspaceTerminalCommand,
  runWorkspaceTerminalCommands,
  upsertLaunchTask,
} from "@/lib/command-client";
import { sendDesktopNotification } from "@/lib/desktop-notification";
import {
  buildTerminalCommandLine,
  sortTerminalCommandTemplates,
  terminalCommandLineSchema,
} from "@/lib/terminal-command";
import { reorderLayoutTransition } from "@/lib/reorder-motion";
import { appPreferencesQueryKey, getAppPreferences } from "@/queries/preferences";
import { sidebarWorkspacesQueryKey, workspacesQueryKey } from "@/queries/workspaces";
import { useShellStore } from "@/stores/shell-store";
import type { LaunchTaskRecord, UpsertLaunchTaskPayload } from "@/types/backend";

const editorSchema = z.object({
  name: z
    .string()
    .trim()
    .min(1, "请输入命令名称")
    .max(80, "命令名称不能超过 80 个字符"),
  commandLine: terminalCommandLineSchema,
});

type EditorFormValues = z.infer<typeof editorSchema>;

const emptyEditorValues: EditorFormValues = {
  name: "",
  commandLine: "",
};

export default function HomePage() {
  const [isEditorOpen, setIsEditorOpen] = useState(false);
  const [editingTask, setEditingTask] = useState<LaunchTaskRecord | null>(null);
  const [selectedTemplateKey, setSelectedTemplateKey] = useState<string | undefined>(undefined);
  const [orderedTasks, setOrderedTasks] = useState<LaunchTaskRecord[]>([]);
  const [draggingTaskId, setDraggingTaskId] = useState<string | null>(null);
  const [inlineError, setInlineError] = useState<string | null>(null);

  const desktopRuntimeAvailable = hasDesktopRuntime();
  const selectedHomeWorkspaceId = useShellStore((state) => state.selectedHomeWorkspaceId);
  const queryClient = useQueryClient();
  const latestOrderedTasksRef = useRef<LaunchTaskRecord[]>([]);
  const dragStartOrderRef = useRef<string[] | null>(null);

  const workspaceQuery = useQuery({
    queryKey: sidebarWorkspacesQueryKey,
    queryFn: listWorkspaces,
    enabled: desktopRuntimeAvailable,
    staleTime: 30_000,
  });
  const preferencesQuery = useQuery({
    queryKey: appPreferencesQueryKey,
    queryFn: getAppPreferences,
    enabled: desktopRuntimeAvailable,
    staleTime: 30_000,
  });

  const selectedWorkspace = useMemo(() => {
    if (!selectedHomeWorkspaceId) {
      return null;
    }

    return (
      workspaceQuery.data?.find((workspace) => workspace.id === selectedHomeWorkspaceId && !workspace.isArchived) ?? null
    );
  }, [selectedHomeWorkspaceId, workspaceQuery.data]);

  const selectedProject = selectedWorkspace?.projects[0] ?? null;
  const workspacePath = selectedProject?.path?.trim() ?? "";
  const workspaceFolderName = resolveWorkspaceFolderName(
    workspacePath,
    selectedWorkspace?.name ?? "工作区",
  );

  const terminalTasks = useMemo(() => {
    if (!selectedProject) {
      return [];
    }

    return [...selectedProject.launchTasks]
      .filter((task) => task.taskType === "terminal_command")
      .sort((left, right) => left.sortOrder - right.sortOrder);
  }, [selectedProject]);

  const terminalCommandTemplates = useMemo(
    () => sortTerminalCommandTemplates(preferencesQuery.data?.terminal.commandTemplates ?? []),
    [preferencesQuery.data?.terminal.commandTemplates],
  );

  useEffect(() => {
    setOrderedTasks(terminalTasks);
    latestOrderedTasksRef.current = terminalTasks;
  }, [terminalTasks, selectedProject?.id]);

  const form = useForm<EditorFormValues>({
    resolver: zodResolver(editorSchema),
    defaultValues: emptyEditorValues,
    mode: "onChange",
  });

  const saveTaskMutation = useMutation({
    mutationFn: async (values: EditorFormValues) => {
      if (!selectedProject) {
        throw new Error("当前工作区没有可配置的项目目录");
      }

      const commandLine = values.commandLine.trim();
      return upsertLaunchTask({
        id: editingTask?.id,
        projectId: selectedProject.id,
        name: values.name.trim(),
        taskType: "terminal_command",
        enabled: editingTask?.enabled ?? true,
        command: commandLine,
        args: [],
        workingDir: workspacePath || undefined,
        timeoutMs: editingTask?.timeoutMs ?? 30_000,
        continueOnFailure: editingTask?.continueOnFailure ?? false,
        retryPolicy: editingTask?.retryPolicy ?? {
          maxAttempts: 1,
          backoffMs: 0,
        },
        sortOrder: editingTask?.sortOrder ?? orderedTasks.length,
      });
    },
    onSuccess: async () => {
      await invalidateWorkspaceQueries(queryClient);
      setInlineError(null);
      setIsEditorOpen(false);
      setEditingTask(null);
      setSelectedTemplateKey(undefined);
      form.reset(emptyEditorValues);
      toast.success(editingTask ? "终端命令已更新" : "终端命令已添加");
    },
    onError: (error) => {
      const summary = getErrorSummary(error);
      setInlineError(summary.message);
      toast.error(summary.message);
    },
  });

  const deleteTaskMutation = useMutation({
    mutationFn: deleteLaunchTask,
    onSuccess: async () => {
      await invalidateWorkspaceQueries(queryClient);
      toast.success("终端命令已删除");
    },
    onError: (error) => {
      toast.error(getErrorSummary(error).message);
    },
  });

  const reorderTasksMutation = useMutation({
    mutationFn: async ({
      nextTasks,
    }: {
      nextTasks: LaunchTaskRecord[];
    }) => {
      if (!selectedProject) {
        throw new Error("当前工作区没有可配置的项目目录");
      }

      return reorderLaunchTasks({
        projectId: selectedProject.id,
        launchTaskIds: nextTasks.map((task) => task.id),
      });
    },
    onMutate: ({ nextTasks }) => {
      setOrderedTasks(nextTasks);
      latestOrderedTasksRef.current = nextTasks;
    },
    onSuccess: async () => {
      await invalidateWorkspaceQueries(queryClient);
      toast.success("终端命令顺序已更新");
    },
    onError: (error) => {
      setOrderedTasks(terminalTasks);
      latestOrderedTasksRef.current = terminalTasks;
      toast.error(getErrorSummary(error).message);
    },
  });

  const handlePreviewReorder = useCallback((nextTasks: LaunchTaskRecord[]) => {
    latestOrderedTasksRef.current = nextTasks;
    setOrderedTasks(nextTasks);
  }, []);

  const handleTaskDragStart = useCallback(
    (taskId: string) => {
      if (reorderTasksMutation.isPending) {
        return;
      }

      dragStartOrderRef.current = latestOrderedTasksRef.current.map((task) => task.id);
      setDraggingTaskId(taskId);
    },
    [reorderTasksMutation.isPending, dragStartOrderRef, latestOrderedTasksRef],
  );

  const handleTaskDragEnd = useCallback(async () => {
    const previousOrder = dragStartOrderRef.current;
    const nextTasks = latestOrderedTasksRef.current;
    dragStartOrderRef.current = null;
    setDraggingTaskId(null);

    if (!previousOrder || reorderTasksMutation.isPending) {
      return;
    }

    if (isSameTaskOrder(previousOrder, nextTasks)) {
      return;
    }

    try {
      await reorderTasksMutation.mutateAsync({ nextTasks });
    } catch {
      return;
    }
  }, [dragStartOrderRef, latestOrderedTasksRef, reorderTasksMutation]);

  const runSingleTaskMutation = useMutation({
    mutationFn: ({
      workspaceId,
      launchTaskId,
    }: {
      workspaceId: string;
      launchTaskId: string;
    }) => runWorkspaceTerminalCommand(workspaceId, launchTaskId),
    onSuccess: (result) => {
      toast.success("已打开独立终端窗口", {
        description: result.commandLine,
      });
      void sendDesktopNotification({
        title: "Bexo Studio",
        body: `已在独立终端窗口执行：${result.commandLine}`,
      });
    },
    onError: (error) => {
      const summary = getErrorSummary(error);
      toast.error(summary.message);
      void sendDesktopNotification({
        title: "Bexo Studio",
        body: `独立运行失败：${summary.message}`,
      });
    },
  });

  const runAllTasksMutation = useMutation({
    mutationFn: (workspaceId: string) => runWorkspaceTerminalCommands(workspaceId),
    onSuccess: (result) => {
      toast.success(`已开始打开 ${result.launchedCount} 个终端标签`, {
        description: `同一窗口内按顺序启动，间隔 ${Math.round(result.staggerMs / 1000)} 秒`,
      });
      void sendDesktopNotification({
        title: "Bexo Studio",
        body: `已开始打开 ${result.launchedCount} 个终端标签，同一窗口内按顺序启动，间隔 ${Math.round(
          result.staggerMs / 1000,
        )} 秒`,
      });
    },
    onError: (error) => {
      const summary = getErrorSummary(error);
      toast.error(summary.message);
      void sendDesktopNotification({
        title: "Bexo Studio",
        body: `运行全部失败：${summary.message}`,
      });
    },
  });
  const toggleTaskEnabledMutation = useMutation({
    mutationFn: async ({
      task,
      enabled,
    }: {
      task: LaunchTaskRecord;
      enabled: boolean;
    }) => {
      if (!selectedProject) {
        throw new Error("当前工作区没有可配置的项目目录");
      }

      return upsertLaunchTask(buildLaunchTaskPayload(task, selectedProject.id, workspacePath, {
        enabled,
      }));
    },
    onSuccess: async (task) => {
      await invalidateWorkspaceQueries(queryClient);
      toast.success(task.enabled ? `已启用：${task.name}` : `已停用：${task.name}`);
    },
    onError: (error) => {
      toast.error(getErrorSummary(error).message);
    },
  });

  useEffect(() => {
    if (!isEditorOpen) {
      return;
    }

    setInlineError(null);
    form.reset(
      editingTask
        ? {
            name: editingTask.name,
            commandLine: buildTerminalCommandLine(editingTask.command, editingTask.args),
          }
        : emptyEditorValues,
    );
    setSelectedTemplateKey(undefined);
  }, [editingTask, form, isEditorOpen]);

  function closeEditor() {
    setInlineError(null);
    setSelectedTemplateKey(undefined);
    setEditingTask(null);
    setIsEditorOpen(false);
    form.reset(emptyEditorValues);
  }

  function openCreateEditor() {
    setEditingTask(null);
    setIsEditorOpen(true);
  }

  function openEditEditor(task: LaunchTaskRecord) {
    setEditingTask(task);
    setIsEditorOpen(true);
  }

  function handleTemplateChange(templateKey?: string) {
    setSelectedTemplateKey(templateKey);

    if (!templateKey) {
      return;
    }

    const template = terminalCommandTemplates.find((item) => item.id === templateKey);
    if (!template) {
      return;
    }

    form.setValue("commandLine", template.commandLine, {
      shouldDirty: true,
      shouldTouch: true,
      shouldValidate: true,
    });

    const currentName = form.getValues("name").trim();
    if (!currentName) {
      form.setValue("name", template.name, {
        shouldDirty: true,
        shouldTouch: true,
        shouldValidate: true,
      });
    }
  }

  async function handleSubmitEditor(values: EditorFormValues) {
    setInlineError(null);
    await saveTaskMutation.mutateAsync(values);
  }

  async function handleDeleteTask(taskId: string) {
    await deleteTaskMutation.mutateAsync(taskId);
  }

  async function handleRunSingleTask(taskId: string) {
    if (!selectedWorkspace) {
      toast.error("请先选择一个工作区");
      return;
    }

    await runSingleTaskMutation.mutateAsync({
      workspaceId: selectedWorkspace.id,
      launchTaskId: taskId,
    });
  }

  async function handleRunAllTasks() {
    if (!selectedWorkspace) {
      toast.error("请先选择一个工作区");
      return;
    }

    await runAllTasksMutation.mutateAsync(selectedWorkspace.id);
  }

  async function handleToggleTaskEnabled(task: LaunchTaskRecord, enabled: boolean) {
    await toggleTaskEnabledMutation.mutateAsync({ task, enabled });
  }

  async function handleCopyWorkspacePath() {
    try {
      await copyTextToClipboard(workspacePath, "当前工作区没有目录");
      toast.success("已复制工作区路径");
    } catch (error) {
      toast.error(getClipboardErrorMessage(error, "复制工作区路径失败"));
    }
  }

  if (!desktopRuntimeAvailable) {
    return (
      <div className="flex h-full min-h-0 flex-col bg-surface">
        <PaneHeader title="工作台" />
        <div className="flex min-h-0 flex-1 items-center justify-center px-4">
          <Empty description="请在 Bexo Studio 桌面应用中管理工作区命令" image={Empty.PRESENTED_IMAGE_SIMPLE} />
        </div>
      </div>
    );
  }

  const enabledTaskCount = orderedTasks.filter((task) => task.enabled).length;

  return (
    <div className="flex h-full min-h-0 flex-col bg-surface">
      <PaneHeader
        actions={
          workspacePath ? (
            <Tooltip title="复制工作区路径">
              <Button
                aria-label="复制工作区路径"
                icon={<CopyOutlined />}
                onClick={() => void handleCopyWorkspacePath()}
                size="small"
                type="text"
              />
            </Tooltip>
          ) : null
        }
        meta={
          workspacePath ? (
            <span className="allow-text-selection font-mono text-[12px]" title={workspacePath}>
              {workspacePath}
            </span>
          ) : null
        }
        title={selectedWorkspace ? selectedWorkspace.name : "工作台"}
      />

      {workspaceQuery.error ? (
        <Alert
          banner
          description={getErrorSummary(workspaceQuery.error).message}
          message="读取工作区失败"
          showIcon
          type="error"
        />
      ) : null}

      {workspaceQuery.isLoading ? (
        <div className="flex min-h-0 flex-1 items-center justify-center">
          <Spin size="small" />
        </div>
      ) : !selectedWorkspace ? (
        <div className="flex min-h-0 flex-1 items-center justify-center">
          <Empty description="在左侧选择一个工作区" image={Empty.PRESENTED_IMAGE_SIMPLE} />
        </div>
      ) : !selectedProject ? (
        <div className="flex min-h-0 flex-1 items-center justify-center">
          <Empty description="这个工作区还没有关联项目目录" image={Empty.PRESENTED_IMAGE_SIMPLE} />
        </div>
      ) : (
        <div className="flex min-h-0 flex-1">
          <ResourceBrowserPane
            workspaceId={selectedWorkspace.id}
            workspaceName={workspaceFolderName}
            workspacePath={workspacePath}
          />

          <section aria-label="启动命令" data-workspace-commands className="flex min-h-0 min-w-0 flex-1 flex-col">
            <PaneHeader
              actions={
                <>
                  <Button
                    disabled={!enabledTaskCount}
                    icon={<CaretRightOutlined />}
                    loading={runAllTasksMutation.isPending}
                    onClick={() => void handleRunAllTasks()}
                    size="small"
                    type="primary"
                  >
                    运行全部
                  </Button>
                  <Button icon={<PlusOutlined />} onClick={openCreateEditor} size="small">
                    新增命令
                  </Button>
                </>
              }
              meta={orderedTasks.length ? `${enabledTaskCount}/${orderedTasks.length} 启用` : null}
              title="启动命令"
            />

            <div className="min-h-0 flex-1 overflow-auto p-1">
              {orderedTasks.length ? (
                <Reorder.Group
                  as="div"
                  axis="y"
                  className="flex flex-col"
                  onReorder={handlePreviewReorder}
                  values={orderedTasks}
                >
                  {orderedTasks.map((task) => (
                    <CommandTaskReorderItem
                      dragging={draggingTaskId === task.id}
                      key={task.id}
                      onDelete={() => void handleDeleteTask(task.id)}
                      onDragEnd={() => void handleTaskDragEnd()}
                      onDragStart={() => handleTaskDragStart(task.id)}
                      onEdit={() => openEditEditor(task)}
                      onRun={() => void handleRunSingleTask(task.id)}
                      onToggleEnabled={(enabled) => void handleToggleTaskEnabled(task, enabled)}
                      switchLoading={
                        toggleTaskEnabledMutation.isPending &&
                        toggleTaskEnabledMutation.variables?.task.id === task.id
                      }
                      runButtonLoading={
                        runSingleTaskMutation.isPending &&
                        runSingleTaskMutation.variables?.launchTaskId === task.id
                      }
                      runDisabled={
                        !task.enabled ||
                        runAllTasksMutation.isPending ||
                        reorderTasksMutation.isPending ||
                        toggleTaskEnabledMutation.isPending
                      }
                      task={task}
                      workspacePath={workspacePath}
                    />
                  ))}
                </Reorder.Group>
              ) : (
                <div className="flex min-h-[200px] items-center justify-center">
                  <Empty description="还没有启动命令" image={Empty.PRESENTED_IMAGE_SIMPLE}>
                    <Button icon={<PlusOutlined />} onClick={openCreateEditor} size="small" type="primary">
                      新增命令
                    </Button>
                  </Empty>
                </div>
              )}
            </div>
          </section>
        </div>
      )}

      <Modal
        cancelText="取消"
        confirmLoading={saveTaskMutation.isPending}
        okText={editingTask ? "保存" : "添加"}
        onCancel={closeEditor}
        onOk={() => void form.handleSubmit(handleSubmitEditor)()}
        open={isEditorOpen}
        title={editingTask ? "编辑终端命令" : "新增终端命令"}
        width={560}
      >
        <div className="space-y-4 pt-1">
          {inlineError ? (
            <Alert
              message="保存终端命令失败"
              showIcon
              type="error"
              description={inlineError}
            />
          ) : null}

          {preferencesQuery.error ? (
            <Alert
              message="读取终端模板失败"
              showIcon
              type="error"
              description={getErrorSummary(preferencesQuery.error).message}
            />
          ) : null}

          <div className="space-y-1">
            <Typography.Text className="text-[12px] font-medium text-fg">
              命令模板
            </Typography.Text>
            <Select
              allowClear
              className="w-full"
              disabled={preferencesQuery.isLoading || !terminalCommandTemplates.length}
              onChange={(value) => handleTemplateChange(value)}
              options={terminalCommandTemplates.map((template) => ({
                label: template.name,
                value: template.id,
              }))}
              placeholder={
                terminalCommandTemplates.length
                  ? "从已保存的终端模板中选择"
                  : "还没有模板，可在 设置 > 通用 中添加"
              }
              value={selectedTemplateKey}
            />
          </div>

          <div className="space-y-1">
            <Typography.Text className="text-[12px] font-medium text-fg">
              命令名称
            </Typography.Text>
            <Controller
              control={form.control}
              name="name"
              render={({ field }) => (
                <Input
                  {...field}
                  placeholder="例如：启动前端开发服务器"
                  status={form.formState.errors.name ? "error" : ""}
                />
              )}
            />
            {form.formState.errors.name ? (
              <Typography.Text className="block text-[11px] text-danger">
                {form.formState.errors.name.message}
              </Typography.Text>
            ) : null}
          </div>

          <div className="space-y-1">
            <Typography.Text className="text-[12px] font-medium text-fg">
              终端命令
            </Typography.Text>
            <Controller
              control={form.control}
              name="commandLine"
              render={({ field }) => (
                <Input
                  {...field}
                  placeholder="单行命令，例如：npm run dev"
                  status={form.formState.errors.commandLine ? "error" : ""}
                />
              )}
            />
            {form.formState.errors.commandLine ? (
              <Typography.Text className="block text-[11px] text-danger">
                {form.formState.errors.commandLine.message}
              </Typography.Text>
            ) : null}
          </div>
        </div>
      </Modal>
    </div>
  );
}

async function invalidateWorkspaceQueries(queryClient: ReturnType<typeof useQueryClient>) {
  await Promise.all([
    queryClient.invalidateQueries({ queryKey: sidebarWorkspacesQueryKey }),
    queryClient.invalidateQueries({ queryKey: workspacesQueryKey }),
  ]);
}

function resolveWorkspaceFolderName(path: string, fallbackName: string) {
  const normalized = path.trim().replace(/[\\/]+$/, "");
  if (!normalized) {
    return fallbackName.trim() || "工作区";
  }

  const segments = normalized.split(/[\\/]/).filter(Boolean);
  return segments.at(-1) ?? (fallbackName.trim() || "工作区");
}

function buildLaunchTaskPayload(
  task: LaunchTaskRecord,
  projectId: string,
  workspacePath: string,
  overrides?: Partial<UpsertLaunchTaskPayload>,
): UpsertLaunchTaskPayload {
  return {
    id: task.id,
    projectId,
    name: task.name,
    taskType: task.taskType,
    enabled: task.enabled,
    command: task.command,
    args: task.args,
    workingDir: task.workingDir || workspacePath || undefined,
    timeoutMs: task.timeoutMs,
    continueOnFailure: task.continueOnFailure,
    retryPolicy: task.retryPolicy,
    sortOrder: task.sortOrder,
    ...overrides,
  };
}

type CommandTaskReorderItemProps = {
  dragging: boolean;
  onDelete: () => void;
  onDragEnd: () => void;
  onDragStart: () => void;
  onEdit: () => void;
  onRun: () => void;
  onToggleEnabled: (enabled: boolean) => void;
  runButtonLoading: boolean;
  runDisabled: boolean;
  switchLoading: boolean;
  task: LaunchTaskRecord;
  workspacePath: string;
};

function CommandTaskReorderItem({
  dragging,
  onDelete,
  onDragEnd,
  onDragStart,
  onEdit,
  onRun,
  onToggleEnabled,
  runButtonLoading,
  runDisabled,
  switchLoading,
  task,
  workspacePath,
}: CommandTaskReorderItemProps) {
  const dragControls = useDragControls();
  const commandLine = buildTerminalCommandLine(task.command, task.args);
  const workingDir = task.workingDir?.trim() ?? "";
  // The working directory only adds information when it differs from the workspace root.
  const showWorkingDir = Boolean(workingDir) && normalizeDirectory(workingDir) !== normalizeDirectory(workspacePath);

  return (
    <Reorder.Item
      as="div"
      className={cn(
        "group relative flex h-10 items-center gap-2 rounded-md px-1.5 transition-colors",
        dragging ? "bg-selected" : "hover:bg-hover",
      )}
      dragControls={dragControls}
      dragListener={false}
      layout="position"
      onDoubleClick={onEdit}
      onDragEnd={onDragEnd}
      onDragStart={onDragStart}
      transition={reorderLayoutTransition}
      value={task}
      whileDrag={{ zIndex: 3, boxShadow: "0 8px 24px -12px rgba(15,23,42,0.35)" }}
    >
      <div
        aria-label={`拖动排序 ${task.name}`}
        className="reveal-on-hover flex h-6 w-3.5 shrink-0 cursor-grab touch-none items-center justify-center text-[11px] text-fg-3 active:cursor-grabbing"
        onPointerDown={(event) => dragControls.start(event)}
        role="button"
        tabIndex={-1}
        title="拖动调整顺序"
      >
        <HolderOutlined />
      </div>
      <Tooltip mouseEnterDelay={0.4} title={task.enabled ? "已启用：参与“运行全部”" : "已停用：不参与“运行全部”"}>
        <Switch
          aria-label={`${task.enabled ? "停用" : "启用"} ${task.name}`}
          checked={task.enabled}
          loading={switchLoading}
          onChange={(checked) => onToggleEnabled(checked)}
          onClick={(_, event) => event?.stopPropagation()}
          size="small"
        />
      </Tooltip>
      <div className={cn("flex min-w-0 flex-1 items-baseline gap-3", !task.enabled && "opacity-55")}>
        <span className="w-[180px] max-w-[40%] shrink-0 truncate text-[13px] font-medium text-fg" title={task.name}>
          {task.name}
        </span>
        <span className="min-w-0 truncate font-mono text-[12px] text-fg-2" title={commandLine}>
          {commandLine}
        </span>
        {showWorkingDir ? (
          <span className="min-w-0 max-w-[35%] shrink truncate text-[12px] text-fg-3" title={`工作目录：${workingDir}`}>
            {workingDir}
          </span>
        ) : null}
      </div>
      <div className="flex shrink-0 items-center">
        <Tooltip title="在新终端窗口运行">
          <Button
            aria-label={`运行 ${task.name}`}
            disabled={runDisabled}
            icon={<CaretRightOutlined />}
            loading={runButtonLoading}
            onClick={onRun}
            size="small"
            type="text"
          />
        </Tooltip>
        <Tooltip title="编辑">
          <Button aria-label={`编辑 ${task.name}`} className="reveal-on-hover" icon={<EditOutlined />} onClick={onEdit} size="small" type="text" />
        </Tooltip>
        <Popconfirm
          cancelText="取消"
          description="只移除这条命令，不影响项目文件。"
          okButtonProps={{ danger: true }}
          okText="删除"
          onConfirm={onDelete}
          title={`删除“${task.name}”？`}
        >
          <Button aria-label={`删除 ${task.name}`} className="reveal-on-hover" danger icon={<DeleteOutlined />} size="small" type="text" />
        </Popconfirm>
      </div>
    </Reorder.Item>
  );
}

function normalizeDirectory(path: string) {
  return path.trim().replace(/[\\/]+/g, "/").replace(/\/$/, "").toLowerCase();
}

function isSameTaskOrder(previousOrder: string[], nextTasks: LaunchTaskRecord[]) {
  if (previousOrder.length !== nextTasks.length) {
    return false;
  }

  return previousOrder.every((taskId, index) => nextTasks[index]?.id === taskId);
}
