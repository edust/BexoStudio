import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  DeleteOutlined,
  FolderOpenOutlined,
  HolderOutlined,
  PlusOutlined,
  SettingOutlined,
} from "@ant-design/icons";
import { zodResolver } from "@hookform/resolvers/zod";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  Alert,
  Button,
  Empty,
  Input,
  Modal,
  Popconfirm,
  Spin,
  Typography,
} from "antd";
import { Reorder, useDragControls } from "motion/react";
import { Controller, useForm } from "react-hook-form";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";

import {
  CommandClientError,
  getErrorSummary,
  hasDesktopRuntime,
} from "@/lib/command-client";
import { defaultAppPreferences } from "@/lib/app-preferences";
import {
  assignTerminalCommandTemplateSortOrder,
  createEmptyTerminalCommandTemplate,
  createTerminalCommandTemplateId,
  sortTerminalCommandTemplates,
  terminalCommandTemplateSchema,
  type TerminalCommandTemplateFormInputValues,
  type TerminalCommandTemplateFormValues,
} from "@/lib/terminal-command";
import { cn } from "@/lib/cn";
import { reorderLayoutTransition } from "@/lib/reorder-motion";
import {
  appPreferencesQueryKey,
  codexHomeDirectoryQueryKey,
  getAppPreferences,
  getCodexHomeDirectory,
  updateAppPreferences,
} from "@/queries/preferences";
import type { AppPreferences, TerminalCommandTemplateRecord } from "@/types/backend";

export default function SettingsPage() {
  const [pathInlineError, setPathInlineError] = useState<string | null>(null);
  const [codexHomeInlineError, setCodexHomeInlineError] = useState<string | null>(null);
  const [templateInlineError, setTemplateInlineError] = useState<string | null>(null);
  const [isTemplateManagerOpen, setIsTemplateManagerOpen] = useState(false);
  const [selectedTemplateId, setSelectedTemplateId] = useState<string | null>(null);
  const [orderedTemplates, setOrderedTemplates] = useState<TerminalCommandTemplateRecord[]>([]);
  const [draggingTemplateId, setDraggingTemplateId] = useState<string | null>(null);

  const desktopRuntimeAvailable = hasDesktopRuntime();
  const queryClient = useQueryClient();
  const latestOrderedTemplatesRef = useRef<TerminalCommandTemplateRecord[]>([]);
  const dragStartOrderRef = useRef<string[] | null>(null);
  const preferencesQuery = useQuery({
    queryKey: appPreferencesQueryKey,
    queryFn: getAppPreferences,
    enabled: desktopRuntimeAvailable,
    staleTime: 30_000,
  });
  const codexHomeQuery = useQuery({
    queryKey: codexHomeDirectoryQueryKey,
    queryFn: getCodexHomeDirectory,
    enabled: desktopRuntimeAvailable,
    staleTime: 30_000,
  });
  const updatePreferencesMutation = useMutation({
    mutationFn: updateAppPreferences,
    onSuccess: (preferences) => {
      queryClient.setQueryData(appPreferencesQueryKey, preferences);
    },
  });

  const resolvedPreferences = preferencesQuery.data ?? defaultAppPreferences;
  const windowsTerminalPath = resolvedPreferences.terminal.windowsTerminalPath?.trim() ?? "";
  const codexHomePath = codexHomeQuery.data?.path?.trim() ?? "";
  const codexHomeExists = codexHomeQuery.data?.exists ?? false;
  const codexHomeDisplayValue = codexHomePath || "未能解析 Codex 配置目录";
  const codexHomeResolveError = codexHomeQuery.error
    ? getErrorSummary(codexHomeQuery.error).message
    : null;
  const canOpenCodexHomeDirectory =
    desktopRuntimeAvailable &&
    Boolean(codexHomePath) &&
    codexHomeExists &&
    !codexHomeQuery.isFetching;
  const codexHomeDescription = codexHomeResolveError
    ? `读取失败：${codexHomeResolveError}`
    : !codexHomePath
      ? "当前环境未解析到 Codex 配置目录。"
      : codexHomeExists
        ? codexHomeQuery.data?.source === "env"
          ? "当前目录由环境变量 CODEX_HOME 指定。"
          : "当前目录按 Codex 默认位置自动解析。"
        : codexHomeQuery.data?.source === "env"
          ? "当前目录由环境变量 CODEX_HOME 指定，但目录当前不存在。"
          : "当前默认目录尚不存在，按钮已禁用。";
  const terminalTemplates = useMemo(
    () => sortTerminalCommandTemplates(resolvedPreferences.terminal.commandTemplates ?? []),
    [resolvedPreferences.terminal.commandTemplates],
  );

  useEffect(() => {
    setOrderedTemplates(terminalTemplates);
    latestOrderedTemplatesRef.current = terminalTemplates;
  }, [terminalTemplates]);

  const templateForm = useForm<
    TerminalCommandTemplateFormInputValues,
    unknown,
    TerminalCommandTemplateFormValues
  >({
    resolver: zodResolver(terminalCommandTemplateSchema),
    defaultValues: createEmptyTerminalCommandTemplate(),
    mode: "onChange",
  });

  const selectedOrderedTemplate = orderedTemplates.find(
    (template) => template.id === selectedTemplateId,
  );
  const hasPersistedSelectedTemplate = Boolean(selectedOrderedTemplate);

  function preventCopy(event: {
    preventDefault: () => void;
    stopPropagation: () => void;
  }) {
    event.preventDefault();
    event.stopPropagation();
  }

  function resetTemplateEditor(template?: TerminalCommandTemplateRecord | null) {
    const nextTemplate =
      template ?? createEmptyTerminalCommandTemplate(orderedTemplates.length);
    setSelectedTemplateId(template?.id ?? null);
    setTemplateInlineError(null);
    templateForm.reset({
      id: nextTemplate.id,
      name: nextTemplate.name,
      commandLine: nextTemplate.commandLine,
    });
  }

  function openTemplateManager() {
    setIsTemplateManagerOpen(true);
    resetTemplateEditor(orderedTemplates[0] ?? null);
  }

  function closeTemplateManager() {
    setIsTemplateManagerOpen(false);
    setTemplateInlineError(null);
  }

  async function persistPreferences(nextPreferences: AppPreferences) {
    return updatePreferencesMutation.mutateAsync(nextPreferences);
  }

  async function persistTerminalTemplates(
    nextTemplates: TerminalCommandTemplateRecord[],
  ) {
    return persistPreferences({
      ...resolvedPreferences,
      terminal: {
        ...resolvedPreferences.terminal,
        commandTemplates: assignTerminalCommandTemplateSortOrder(nextTemplates),
      },
    });
  }

  async function handlePickWindowsTerminalDirectory() {
    if (
      !desktopRuntimeAvailable ||
      updatePreferencesMutation.isPending ||
      preferencesQuery.isError
    ) {
      return;
    }

    setPathInlineError(null);

    try {
      const selectedDirectory = await open({
        directory: true,
        multiple: false,
        recursive: true,
        title: "选择 Windows Terminal 所在目录",
        defaultPath: windowsTerminalPath || undefined,
      });

      if (!selectedDirectory || Array.isArray(selectedDirectory)) {
        return;
      }

      const normalizedPath = selectedDirectory.trim();
      if (!normalizedPath || normalizedPath === windowsTerminalPath) {
        return;
      }

      await persistPreferences({
        ...resolvedPreferences,
        terminal: {
          ...resolvedPreferences.terminal,
          windowsTerminalPath: normalizedPath,
        },
      });

      setPathInlineError(null);
      toast.success("Windows Terminal 路径已保存");
    } catch (error) {
      if (error instanceof CommandClientError) {
        return;
      }

      const summary = getErrorSummary(error);
      setPathInlineError(summary.message);
      toast.error(summary.message);
    }
  }

  async function handleOpenCodexHomeDirectory() {
    if (!canOpenCodexHomeDirectory) {
      return;
    }

    setCodexHomeInlineError(null);

    try {
      await revealItemInDir(codexHomePath);
      toast.success("已打开 Codex 配置目录");
    } catch (error) {
      if (error instanceof CommandClientError) {
        return;
      }

      const summary = getErrorSummary(error);
      setCodexHomeInlineError(summary.message);
      toast.error(summary.message);
    }
  }

  async function handleSaveTemplate(values: TerminalCommandTemplateFormValues) {
    setTemplateInlineError(null);
    const currentTemplates = orderedTemplates;
    const templateId = values.id?.trim() || createTerminalCommandTemplateId();
    const persistedCurrentTemplate = currentTemplates.find(
      (template) => template.id === templateId,
    );
    const nextTemplate: TerminalCommandTemplateRecord = {
      id: templateId,
      name: values.name.trim(),
      commandLine: values.commandLine.trim(),
      sortOrder: persistedCurrentTemplate?.sortOrder ?? currentTemplates.length,
    };

    const templateExists = Boolean(persistedCurrentTemplate);
    const nextTemplates = templateExists
      ? currentTemplates.map((template) =>
          template.id === nextTemplate.id ? nextTemplate : template,
        )
      : [...currentTemplates, nextTemplate];

    try {
      await persistTerminalTemplates(nextTemplates);
      resetTemplateEditor(null);
      toast.success(templateExists ? "终端模板已更新" : "终端模板已保存");
    } catch (error) {
      const summary = getErrorSummary(error);
      setTemplateInlineError(summary.message);
      toast.error(summary.message);
    }
  }

  async function handleDeleteTemplate(templateId: string) {
    setTemplateInlineError(null);
    try {
      const nextTemplates = orderedTemplates.filter(
        (template) => template.id !== templateId,
      );
      const updatedPreferences = await persistTerminalTemplates(nextTemplates);

      const nextSelected = sortTerminalCommandTemplates(
        updatedPreferences.terminal.commandTemplates,
      )[0] ?? null;
      resetTemplateEditor(nextSelected);
      toast.success("终端模板已删除");
    } catch (error) {
      const summary = getErrorSummary(error);
      setTemplateInlineError(summary.message);
      toast.error(summary.message);
    }
  }

  const handlePreviewReorder = useCallback(
    (nextTemplates: TerminalCommandTemplateRecord[]) => {
      latestOrderedTemplatesRef.current = nextTemplates;
      setOrderedTemplates(nextTemplates);
    },
    [],
  );

  const handleTemplateDragStart = useCallback(
    (templateId: string) => {
      if (updatePreferencesMutation.isPending) {
        return;
      }

      dragStartOrderRef.current = latestOrderedTemplatesRef.current.map(
        (template) => template.id,
      );
      setDraggingTemplateId(templateId);
    },
    [updatePreferencesMutation.isPending],
  );

  const handleTemplateDragEnd = useCallback(async () => {
    const previousOrder = dragStartOrderRef.current;
    const nextTemplates = latestOrderedTemplatesRef.current;
    dragStartOrderRef.current = null;
    setDraggingTemplateId(null);

    if (!previousOrder || updatePreferencesMutation.isPending) {
      return;
    }

    if (isSameTemplateOrder(previousOrder, nextTemplates)) {
      return;
    }

    try {
      setTemplateInlineError(null);
      await persistTerminalTemplates(nextTemplates);
      toast.success("终端模板顺序已更新");
    } catch (error) {
      latestOrderedTemplatesRef.current = terminalTemplates;
      setOrderedTemplates(terminalTemplates);
      const summary = getErrorSummary(error);
      setTemplateInlineError(summary.message);
      toast.error(summary.message);
    }
  }, [persistTerminalTemplates, terminalTemplates, updatePreferencesMutation.isPending]);

  return (
    <div className="flex h-full min-h-0 flex-col bg-white">
      <div className="border-b border-[#e6edf5] px-5 py-4">
        <Typography.Text className="block text-[11px] font-semibold uppercase tracking-[0.22em] text-[#1f2937]">
          General
        </Typography.Text>
      </div>

      <div className="flex min-h-0 flex-1 flex-col px-4 py-4">
        {!desktopRuntimeAvailable ? (
          <Alert
            className="mb-4"
            message="当前页面需要在 Tauri 桌面 runtime 内设置终端偏好。"
            showIcon
            type="info"
          />
        ) : null}

        {preferencesQuery.error ? (
          <Alert
            className="mb-4"
            message="读取设置失败"
            description={getErrorSummary(preferencesQuery.error).message}
            showIcon
            type="error"
          />
        ) : null}

        {pathInlineError ? (
          <Alert
            className="mb-4"
            closable
            message="保存设置失败"
            onClose={() => setPathInlineError(null)}
            showIcon
            type="error"
            description={pathInlineError}
          />
        ) : null}

        {codexHomeInlineError ? (
          <Alert
            className="mb-4"
            closable
            message="打开目录失败"
            onClose={() => setCodexHomeInlineError(null)}
            showIcon
            type="error"
            description={codexHomeInlineError}
          />
        ) : null}

        {desktopRuntimeAvailable && (preferencesQuery.isLoading || codexHomeQuery.isLoading) ? (
          <div className="flex min-h-0 flex-1 items-center justify-center">
            <Spin size="small" />
          </div>
        ) : (
          <div className="flex min-h-0 flex-1 flex-col gap-4">
            <div className="rounded-[0] border border-[#eef2f6] bg-white">
              <div className="grid grid-cols-[180px_minmax(0,1fr)_auto] items-center gap-3 px-4 py-4">
                <div className="min-w-0">
                  <Typography.Text className="block text-[12px] font-medium text-[#1f2937]">
                    Windows Terminal 位置
                  </Typography.Text>
                </div>

                <div
                  className="flex h-[32px] items-center overflow-hidden rounded-[8px] border border-[#d8e1eb] bg-[#f8fafc] px-3 font-mono text-[12px] text-[#475467] select-none"
                  onContextMenu={preventCopy}
                  onCopy={preventCopy}
                  onCut={preventCopy}
                  onMouseDown={(event) => {
                    if (event.detail > 1) {
                      preventCopy(event);
                    }
                  }}
                  onSelect={(event) => preventCopy(event)}
                  title={windowsTerminalPath || "未配置 Windows Terminal 路径"}
                >
                  <span className="truncate">
                    {windowsTerminalPath || "未配置 Windows Terminal 路径"}
                  </span>
                </div>

                <Button
                  className="!h-[32px] !px-3 !text-[12px]"
                  disabled={
                    !desktopRuntimeAvailable ||
                    preferencesQuery.isFetching ||
                    preferencesQuery.isError
                  }
                  icon={<FolderOpenOutlined />}
                  loading={updatePreferencesMutation.isPending}
                  onClick={() => void handlePickWindowsTerminalDirectory()}
                  size="small"
                >
                  选择目录
                </Button>
              </div>
            </div>

            <div className="rounded-[0] border border-[#eef2f6] bg-white">
              <div className="grid grid-cols-[180px_minmax(0,1fr)_auto] items-center gap-3 px-4 py-4">
                <div className="min-w-0">
                  <Typography.Text className="block text-[12px] font-medium text-[#1f2937]">
                    Codex 配置目录
                  </Typography.Text>
                </div>

                <div className="min-w-0">
                  <div
                    className="flex h-[32px] items-center overflow-hidden rounded-[8px] border border-[#d8e1eb] bg-[#f8fafc] px-3 font-mono text-[12px] text-[#475467] select-none"
                    onContextMenu={preventCopy}
                    onCopy={preventCopy}
                    onCut={preventCopy}
                    onMouseDown={(event) => {
                      if (event.detail > 1) {
                        preventCopy(event);
                      }
                    }}
                    onSelect={(event) => preventCopy(event)}
                    title={codexHomeDisplayValue}
                  >
                    <span className="truncate">{codexHomeDisplayValue}</span>
                  </div>
                  <Typography.Text className="mt-1 block text-[11px] text-[#667085]">
                    {codexHomeDescription}
                  </Typography.Text>
                </div>

                <Button
                  className="!h-[32px] !px-3 !text-[12px]"
                  disabled={!canOpenCodexHomeDirectory}
                  icon={<FolderOpenOutlined />}
                  onClick={() => void handleOpenCodexHomeDirectory()}
                  size="small"
                >
                  打开目录
                </Button>
              </div>
            </div>

            <div className="rounded-[0] border border-[#eef2f6] bg-white">
              <div className="flex items-center justify-between gap-4 px-4 py-4">
                <div className="min-w-0">
                  <Typography.Text className="block text-[12px] font-medium text-[#1f2937]">
                    终端模板管理
                  </Typography.Text>
                  <Typography.Text className="mt-1 block text-[11px] text-[#667085]">
                    首页“新增命令”弹窗会直接读取这里维护的终端命令模板。当前已保存{" "}
                    {terminalTemplates.length} 个模板。
                  </Typography.Text>
                </div>
                <Button
                  className="!h-[32px] !px-3 !text-[12px]"
                  disabled={!desktopRuntimeAvailable || preferencesQuery.isError}
                  icon={<SettingOutlined />}
                  onClick={openTemplateManager}
                  size="small"
                >
                  管理模板
                </Button>
              </div>
            </div>
          </div>
        )}
      </div>

      <Modal
        cancelText="关闭"
        footer={null}
        onCancel={closeTemplateManager}
        open={isTemplateManagerOpen}
        title="终端模板管理"
        width={760}
      >
        <div className="grid grid-cols-[220px_minmax(0,1fr)] gap-4 pt-1">
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <Typography.Text className="text-[12px] font-semibold text-[#1f2937]">
                已保存模板
              </Typography.Text>
              <Button
                className="!h-[28px] !px-2 !text-[12px]"
                icon={<PlusOutlined />}
                onClick={() => resetTemplateEditor(null)}
                size="small"
                type="default"
              >
                新建
              </Button>
            </div>

            {orderedTemplates.length ? (
              <Reorder.Group
                as="div"
                axis="y"
                className="flex max-h-[360px] flex-col gap-2 overflow-y-auto pr-1"
                onReorder={handlePreviewReorder}
                values={orderedTemplates}
              >
                {orderedTemplates.map((template) => {
                  const active = template.id === selectedTemplateId;

                  return (
                    <TemplateReorderItem
                      active={active}
                      dragging={draggingTemplateId === template.id}
                      key={template.id}
                      onDragEnd={() => void handleTemplateDragEnd()}
                      onDragStart={() => handleTemplateDragStart(template.id)}
                      onSelect={() => resetTemplateEditor(template)}
                      template={template}
                    />
                  );
                })}
              </Reorder.Group>
            ) : (
              <div className="flex min-h-[220px] items-center justify-center rounded-[12px] border border-dashed border-[#e6edf5] bg-[#fbfcfe]">
                <Empty
                  description="还没有终端模板，先点右上角“新建”。"
                  image={Empty.PRESENTED_IMAGE_SIMPLE}
                />
              </div>
            )}
          </div>

          <div className="flex min-h-[420px] flex-col rounded-[12px] border border-[#eef2f6] bg-[#fbfcfe] p-4">
            {templateInlineError ? (
              <Alert
                className="mb-4"
                closable
                message="保存模板失败"
                onClose={() => setTemplateInlineError(null)}
                showIcon
                type="error"
                description={templateInlineError}
              />
            ) : null}

            <div className="space-y-1">
              <Typography.Text className="text-[12px] font-medium text-[#1f2937]">
                模板名称
              </Typography.Text>
              <Controller
                control={templateForm.control}
                name="name"
                render={({ field }) => (
                  <Input
                    onChange={(event) => field.onChange(event.target.value)}
                    onBlur={field.onBlur}
                    placeholder="例如：Node 开发服务器"
                    ref={field.ref}
                    status={templateForm.formState.errors.name ? "error" : ""}
                    value={field.value ?? ""}
                  />
                )}
              />
              {templateForm.formState.errors.name ? (
                <Typography.Text className="block text-[11px] text-[#cf5a4a]">
                  {templateForm.formState.errors.name.message}
                </Typography.Text>
              ) : null}
            </div>

            <div className="mt-4 space-y-1">
              <Typography.Text className="text-[12px] font-medium text-[#1f2937]">
                终端命令
              </Typography.Text>
              <Controller
                control={templateForm.control}
                name="commandLine"
                render={({ field }) => (
                  <Input
                    onChange={(event) => field.onChange(event.target.value)}
                    onBlur={field.onBlur}
                    placeholder="例如：npm run dev"
                    ref={field.ref}
                    status={templateForm.formState.errors.commandLine ? "error" : ""}
                    value={field.value ?? ""}
                  />
                )}
              />
              {templateForm.formState.errors.commandLine ? (
                <Typography.Text className="block text-[11px] text-[#cf5a4a]">
                  {templateForm.formState.errors.commandLine.message}
                </Typography.Text>
              ) : null}
              <Typography.Text className="block text-[11px] text-[#98a2b3]">
                模板会同步出现在首页“新增命令”的模板下拉里。
              </Typography.Text>
            </div>

            <div className="mt-auto flex items-center justify-end gap-2 border-t border-[#e6edf5] pt-4">
              {hasPersistedSelectedTemplate && selectedOrderedTemplate ? (
                <Popconfirm
                  description="删除后首页模板下拉会同步移除这条模板。"
                  okButtonProps={{ danger: true, loading: updatePreferencesMutation.isPending }}
                  okText="删除"
                  onConfirm={() => void handleDeleteTemplate(selectedOrderedTemplate.id)}
                  title="是否删除这条终端模板？"
                >
                  <Button
                    danger
                    disabled={updatePreferencesMutation.isPending}
                    icon={<DeleteOutlined />}
                    size="small"
                    type="default"
                  >
                    删除
                  </Button>
                </Popconfirm>
              ) : null}

              <Button onClick={closeTemplateManager} size="small" type="default">
                关闭
              </Button>
              <Button
                loading={updatePreferencesMutation.isPending}
                onClick={() => void templateForm.handleSubmit(handleSaveTemplate)()}
                size="small"
                type="primary"
              >
                保存模板
              </Button>
            </div>
          </div>
        </div>
      </Modal>
    </div>
  );
}

type TemplateReorderItemProps = {
  active: boolean;
  dragging: boolean;
  onDragEnd: () => void;
  onDragStart: () => void;
  onSelect: () => void;
  template: TerminalCommandTemplateRecord;
};

function TemplateReorderItem({
  active,
  dragging,
  onDragEnd,
  onDragStart,
  onSelect,
  template,
}: TemplateReorderItemProps) {
  const dragControls = useDragControls();

  return (
    <Reorder.Item
      as="div"
      className={cn(
        "w-full rounded-[12px] border px-3 py-3 text-left transition-[border-color,background-color,opacity] duration-150",
        active
          ? "border-[#8fd4ec] bg-[#f4fbfe]"
          : "border-[#e6edf5] bg-[#fbfcfe] hover:border-[#d9e2ec] hover:bg-[#f8fafc]",
        dragging && "border-[#1697c5] bg-[#eef9fe] opacity-95",
      )}
      dragControls={dragControls}
      dragListener={false}
      layout="position"
      onDragEnd={onDragEnd}
      onDragStart={onDragStart}
      transition={reorderLayoutTransition}
      value={template}
      whileDrag={{
        scale: 1.015,
        zIndex: 3,
        boxShadow: "0 22px 44px -28px rgba(22,151,197,0.45)",
      }}
    >
      <div className="flex items-start gap-3">
        <div
          aria-label={`拖动排序 ${template.name}`}
          className="mt-0.5 flex h-6 w-6 cursor-grab touch-none items-center justify-center rounded-[6px] border border-[#d8e1eb] bg-white text-[#667085] active:cursor-grabbing"
          onPointerDown={(event) => dragControls.start(event)}
          role="button"
          tabIndex={-1}
          title="拖动调整模板顺序"
        >
          <HolderOutlined />
        </div>

        <button
          className="min-w-0 flex-1 text-left"
          onClick={onSelect}
          type="button"
        >
          <div className="flex items-center gap-2">
            <Typography.Text className="block text-[12px] font-medium text-[#1f2937]">
              {template.name}
            </Typography.Text>
            <Typography.Text className="text-[10px] text-[#98a2b3]">
              #{template.sortOrder + 1}
            </Typography.Text>
          </div>
          <Typography.Text className="mt-1 block truncate font-mono text-[11px] text-[#667085]">
            {template.commandLine}
          </Typography.Text>
        </button>
      </div>
    </Reorder.Item>
  );
}

function isSameTemplateOrder(
  previousOrder: string[],
  nextTemplates: TerminalCommandTemplateRecord[],
) {
  if (previousOrder.length !== nextTemplates.length) {
    return false;
  }

  return previousOrder.every((templateId, index) => nextTemplates[index]?.id === templateId);
}
