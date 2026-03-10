import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  CaretRightOutlined,
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
  Tag,
  Tooltip,
  Typography,
} from "antd";
import { Reorder, useDragControls } from "motion/react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Controller, useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { toast } from "sonner";

import { ResourceBrowserPane } from "@/features/resource-browser/resource-browser-pane";
import { cn } from "@/lib/cn";
import {
  deleteLaunchTask,
  getErrorSummary,
  hasDesktopRuntime,
  listWorkspaces,
  runWorkspaceTerminalCommand,
  runWorkspaceTerminalCommands,
  upsertLaunchTask,
} from "@/lib/command-client";
import { sendDesktopNotification } from "@/lib/desktop-notification";
import {
  buildTerminalCommandLine,
  sortTerminalCommandTemplates,
  parseTerminalCommandLine,
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
      workspaceQuery.data?.find((workspace) => workspace.id === selectedHomeWorkspaceId) ?? null
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

      const parsed = parseTerminalCommandLine(values.commandLine);
      return upsertLaunchTask({
        id: editingTask?.id,
        projectId: selectedProject.id,
        name: values.name.trim(),
        taskType: "terminal_command",
        enabled: editingTask?.enabled ?? true,
        command: parsed.command,
        args: parsed.args,
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

      for (const [index, task] of nextTasks.entries()) {
        await upsertLaunchTask(
          buildLaunchTaskPayload(task, selectedProject.id, workspacePath, {
            sortOrder: index,
          }),
        );
      }
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

  if (!desktopRuntimeAvailable) {
    return (
      <div className="flex h-full min-h-0 flex-col bg-white">
        <div className="border-b border-[#e6edf5] px-4 py-4">
          <Typography.Text className="block text-[11px] font-semibold uppercase tracking-[0.22em] text-[#1f2937]">
            Workbench
          </Typography.Text>
        </div>

        <div className="flex min-h-0 flex-1 items-center justify-center px-4 py-4">
          <Alert
            className="w-full max-w-[520px]"
            message="当前页面需要在 Tauri 桌面 runtime 内管理工作区命令。"
            showIcon
            type="info"
          />
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col bg-white">
      <div className="border-b border-[#e6edf5] px-4 py-4">
        <Typography.Text className="block text-[13px] font-semibold text-[#1f2937]">
          {workspaceFolderName} 工作区设置
        </Typography.Text>
      </div>

      <div className="flex min-h-0 flex-1 flex-col gap-4 px-0 py-0">
        {workspaceQuery.error ? (
          <Alert
            message="读取工作区失败"
            description={getErrorSummary(workspaceQuery.error).message}
            showIcon
            type="error"
          />
        ) : null}

        {workspaceQuery.isLoading ? (
          <div className="flex min-h-0 flex-1 items-center justify-center">
            <Spin size="small" />
          </div>
        ) : !selectedWorkspace ? (
          <div className="flex min-h-0 flex-1 items-center justify-center rounded-[12px] border border-dashed border-[#e6edf5] bg-[#fbfcfe]">
            <Empty description="请先在左侧选择一个工作区" image={Empty.PRESENTED_IMAGE_SIMPLE} />
          </div>
        ) : !selectedProject ? (
          <Alert
            message="当前工作区缺少项目目录"
            description="请先为这个工作区关联至少一个项目目录，再配置终端命令。"
            showIcon
            type="warning"
          />
        ) : (
          <div className="flex min-h-0 flex-1">
            <ResourceBrowserPane
              workspaceId={selectedWorkspace.id}
              workspaceName={workspaceFolderName}
              workspacePath={workspacePath}
            />

            <div className="flex min-w-0 flex-1 flex-col gap-4 px-0 py-0">
              <div className="rounded-[0] border border-[#eef2f6] bg-white">
                <div className="grid grid-cols-[140px_minmax(0,1fr)] items-center gap-3 px-4 py-4">
                  <Typography.Text className="text-[12px] font-medium text-[#1f2937]">
                    工作区目录
                  </Typography.Text>
                  <div
                    className="flex h-[32px] items-center overflow-hidden rounded-[8px] border border-[#d8e1eb] bg-[#f8fafc] px-3 font-mono text-[12px] text-[#475467] select-none"
                    title={workspacePath || "未配置工作区目录"}
                  >
                    <span className="truncate">{workspacePath || "未配置工作区目录"}</span>
                  </div>
                </div>
              </div>

              <div className="flex min-h-0 flex-1 flex-col rounded-[0] border border-[#eef2f6] bg-white">
                <div className="flex items-center justify-between border-b border-[#eef2f6] px-4 py-3">
                  <div>
                    <Typography.Text className="block text-[12px] font-semibold uppercase tracking-[0.16em] text-[#1f2937]">
                      终端命令组
                    </Typography.Text>
                    <Typography.Text className="block text-[11px] text-[#667085]">
                      新增、编辑、删除并拖动排序；打开的命令才会参与执行。
                    </Typography.Text>
                  </div>
                  <div className="flex items-center gap-2">
                    <Button
                      className="!h-[30px] !px-3 !text-[12px]"
                      disabled={!orderedTasks.some((task) => task.enabled)}
                      icon={<CaretRightOutlined />}
                      loading={runAllTasksMutation.isPending}
                      onClick={() => void handleRunAllTasks()}
                      size="small"
                      type="default"
                    >
                      运行全部
                    </Button>
                    <Button
                      className="!h-[30px] !px-3 !text-[12px]"
                      icon={<PlusOutlined />}
                      onClick={openCreateEditor}
                      size="small"
                      type="default"
                    >
                      新增命令
                    </Button>
                  </div>
                </div>

                <div className="min-h-0 flex-1 overflow-auto px-4 py-4">
                  {orderedTasks.length ? (
                    <Reorder.Group
                      as="div"
                      axis="y"
                      className="flex flex-col gap-2"
                      onReorder={handlePreviewReorder}
                      values={orderedTasks}
                    >
                      {orderedTasks.map((task, index) => {
                        return (
                          <CommandTaskReorderItem
                            dragging={draggingTaskId === task.id}
                            index={index}
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
                        );
                      })}
                    </Reorder.Group>
                  ) : (
                    <div className="flex min-h-[180px] items-center justify-center rounded-[12px] border border-dashed border-[#e6edf5] bg-[#fbfcfe]">
                      <Empty
                        description="还没有终端命令。点击右上角“新增命令”开始配置。"
                        image={Empty.PRESENTED_IMAGE_SIMPLE}
                      />
                    </div>
                  )}
                </div>
              </div>
            </div>
          </div>
        )}
      </div>

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
            <Typography.Text className="text-[12px] font-medium text-[#1f2937]">
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
                  : "请先到 Settings > General 管理终端模板"
              }
              value={selectedTemplateKey}
            />
            <Typography.Text className="block text-[11px] text-[#98a2b3]">
              模板来自 Settings &gt; General，这里只负责快速回填，后续仍可继续修改。
            </Typography.Text>
          </div>

          <div className="space-y-1">
            <Typography.Text className="text-[12px] font-medium text-[#1f2937]">
              命令名称
            </Typography.Text>
            <Input
              {...form.register("name")}
              placeholder="例如：启动前端开发服务器"
              status={form.formState.errors.name ? "error" : ""}
            />
            {form.formState.errors.name ? (
              <Typography.Text className="block text-[11px] text-[#cf5a4a]">
                {form.formState.errors.name.message}
              </Typography.Text>
            ) : null}
          </div>

          <div className="space-y-1">
            <Typography.Text className="text-[12px] font-medium text-[#1f2937]">
              终端命令
            </Typography.Text>
            <Controller
              control={form.control}
              name="commandLine"
              render={({ field }) => (
                <Input
                  {...field}
                  placeholder="例如：npm run dev"
                  status={form.formState.errors.commandLine ? "error" : ""}
                />
              )}
            />
            {form.formState.errors.commandLine ? (
              <Typography.Text className="block text-[11px] text-[#cf5a4a]">
                {form.formState.errors.commandLine.message}
              </Typography.Text>
            ) : null}
            <Typography.Text className="block text-[11px] text-[#98a2b3]">
              仅支持单行命令，基础校验会检查空值、换行和未闭合引号。
            </Typography.Text>
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
  index: number;
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
  index,
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

  return (
    <Reorder.Item
      as="div"
      className={cn(
        "relative rounded-[12px] border border-[#e6edf5] bg-[#fbfcfe] px-3 py-3 transition-[border-color,background-color,opacity] duration-150",
        !task.enabled && "border-[#e5e7eb] bg-[#f8fafc] opacity-72",
        dragging && "border-[#1697c5] bg-[#f0fbff] opacity-95",
      )}
      dragControls={dragControls}
      dragListener={false}
      layout="position"
      onDragEnd={onDragEnd}
      onDragStart={onDragStart}
      transition={reorderLayoutTransition}
      value={task}
      whileDrag={{
        scale: 1.015,
        zIndex: 3,
        boxShadow: "0 22px 44px -28px rgba(22,151,197,0.45)",
      }}
    >
      <div className="flex items-start justify-between gap-3">
        <div className="flex min-w-0 flex-1 items-start gap-3">
          <div
            aria-label={`拖动排序 ${task.name}`}
            className="mt-0.5 flex h-6 w-6 cursor-grab touch-none items-center justify-center rounded-[6px] border border-[#d8e1eb] bg-white text-[#667085] active:cursor-grabbing"
            onPointerDown={(event) => dragControls.start(event)}
            role="button"
            tabIndex={-1}
            title="拖动调整命令顺序"
          >
            <HolderOutlined />
          </div>

          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <Typography.Text className="text-[13px] font-medium text-[#1f2937]">
                {task.name}
              </Typography.Text>
              {!task.enabled ? (
                <Tag
                  bordered={false}
                  className="m-0 rounded-full bg-[#f3f4f6] px-2 py-0 text-[10px] font-semibold uppercase tracking-[0.12em] text-[#6b7280]"
                >
                  已关闭
                </Tag>
              ) : null}
              <Tag
                bordered={false}
                className="m-0 rounded-full bg-[#eef6fb] px-2 py-0 text-[10px] font-semibold uppercase tracking-[0.12em] text-[#1283ab]"
              >
                #{index + 1}
              </Tag>
            </div>
            <Typography.Paragraph
              className="!mb-0 !mt-1 font-mono !text-[12px] !leading-5 !text-[#475467]"
              ellipsis={{ rows: 1, tooltip: commandLine }}
            >
              {commandLine}
            </Typography.Paragraph>
            <Typography.Text className="mt-1 block text-[11px] text-[#98a2b3]">
              工作目录：{task.workingDir || workspacePath}
            </Typography.Text>
          </div>
        </div>

        <div className="flex items-center gap-1">
          <Tooltip placement="bottom" title={task.enabled ? "关闭后将不参与执行" : "打开后将参与执行"}>
            <Switch
              checked={task.enabled}
              loading={switchLoading}
              onChange={(checked) => onToggleEnabled(checked)}
              onClick={(_, event) => event?.stopPropagation()}
              size="small"
            />
          </Tooltip>
          <Tooltip placement="bottom" title="独立窗口运行这条终端命令">
            <Button
              className="!h-7 !w-7 !min-w-7 !p-0"
              disabled={runDisabled}
              icon={<CaretRightOutlined />}
              loading={runButtonLoading}
              onClick={onRun}
              size="small"
              type="text"
            />
          </Tooltip>
          <Tooltip placement="bottom" title="编辑命令">
            <Button
              className="!h-7 !w-7 !min-w-7 !p-0"
              icon={<EditOutlined />}
              onClick={onEdit}
              size="small"
              type="text"
            />
          </Tooltip>
          <Popconfirm
            description="删除后不会影响工作区目录，只会移除这条终端命令。"
            okButtonProps={{ danger: true }}
            okText="删除"
            onConfirm={onDelete}
            title="是否删除这条终端命令？"
          >
            <Button
              className="!h-7 !w-7 !min-w-7 !p-0"
              danger
              icon={<DeleteOutlined />}
              size="small"
              type="text"
            />
          </Popconfirm>
        </div>
      </div>
    </Reorder.Item>
  );
}

function isSameTaskOrder(previousOrder: string[], nextTasks: LaunchTaskRecord[]) {
  if (previousOrder.length !== nextTasks.length) {
    return false;
  }

  return previousOrder.every((taskId, index) => nextTasks[index]?.id === taskId);
}
