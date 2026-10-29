import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  DeleteOutlined,
  FolderOpenOutlined,
  HolderOutlined,
  PlusOutlined,
  SearchOutlined,
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
  InputNumber,
  Modal,
  Popconfirm,
  Select,
  Spin,
  Switch,
  Tooltip,
  Typography,
} from "antd";
import { Reorder, useDragControls } from "motion/react";
import { Controller, useForm } from "react-hook-form";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { toast } from "sonner";

import { PromptQuickPasteSettings } from "@/features/settings/prompt-quick-paste-settings";
import { PathValue, SettingsGroup, SettingsRow } from "@/features/settings/settings-layout";
import { useHotkeyRecorder } from "@/features/settings/use-hotkey-recorder";
import {
  CommandClientError,
  detectEditorsFromPath,
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
import { settingsSections } from "@/lib/navigation";
import { reorderLayoutTransition } from "@/lib/reorder-motion";
import {
  appPreferencesQueryKey,
  codexHomeDirectoryQueryKey,
  getAppPreferences,
  getCodexHomeDirectory,
  getHotkeyHealth,
  hotkeyHealthQueryKey,
  retryHotkeyRegistration,
  updateAppPreferences,
} from "@/queries/preferences";
import { listPrompts, promptsQueryKey } from "@/queries/prompts";
import type {
  AdapterAvailability,
  AppPreferences,
  AppPreferencesPatch,
  CodexAuthPreferences,
  CodexAuthProxyMode,
  CodexAuthProxyPreferences,
  CodexHistoryViewPreferences,
  CustomEditorRecord,
  EditorPathDetectionResult,
  PromptQuickPasteHotkeySlot,
  TerminalCommandShell,
  TerminalCommandTemplateRecord,
} from "@/types/backend";

type EditorKey = "vscode" | "jetbrains";
type SettingsSection = "general" | "hotkeys";
type HotkeyRecorderStatus = "idle" | "recording" | "saving" | "error";

const DEFAULT_SCREENSHOT_CAPTURE_HOTKEY = "Ctrl+Shift+X";
const CODEX_HISTORY_FONT_SIZE_MIN = 10;
const CODEX_HISTORY_FONT_SIZE_MAX = 24;
const CODEX_HISTORY_FONT_FAMILY_MAX_LENGTH = 160;
const CODEX_AUTH_QUOTA_REFRESH_INTERVAL_MIN = 10;
const CODEX_AUTH_QUOTA_REFRESH_INTERVAL_MAX = 3600;
const CODEX_AUTH_MANUAL_PROXY_URL_MAX_LENGTH = 512;
const CODEX_AUTH_PROXY_OPTIONS: Array<{
  label: string;
  value: CodexAuthProxyMode;
}> = [
  { label: "系统代理", value: "system" },
  { label: "手动代理", value: "manual" },
  { label: "不使用代理", value: "disabled" },
];
const TERMINAL_COMMAND_SHELL_OPTIONS: Array<{
  label: string;
  value: TerminalCommandShell;
}> = [
  { label: "PowerShell 7", value: "powershell7" },
  { label: "cmd.exe", value: "cmd" },
];
export default function SettingsPage() {
  const { section } = useParams<{ section?: string }>();
  const activeSection: SettingsSection = section === "hotkeys" ? "hotkeys" : "general";
  const [pathInlineError, setPathInlineError] = useState<string | null>(null);
  const [startupInlineError, setStartupInlineError] = useState<string | null>(null);
  const [codexHomeInlineError, setCodexHomeInlineError] = useState<string | null>(null);
  const [codexHistoryInlineError, setCodexHistoryInlineError] = useState<string | null>(null);
  const [codexAuthInlineError, setCodexAuthInlineError] = useState<string | null>(null);
  const [templateInlineError, setTemplateInlineError] = useState<string | null>(null);
  const [editorInlineError, setEditorInlineError] = useState<string | null>(null);
  const [hotkeyInlineError, setHotkeyInlineError] = useState<string | null>(null);
  const [hotkeyRecorderStatus, setHotkeyRecorderStatus] =
    useState<HotkeyRecorderStatus>("idle");
  const [hotkeyRecorderPreview, setHotkeyRecorderPreview] = useState("");
  const [hotkeyRecorderError, setHotkeyRecorderError] = useState<string | null>(null);
  const [isTemplateManagerOpen, setIsTemplateManagerOpen] = useState(false);
  const [isEditorManagerOpen, setIsEditorManagerOpen] = useState(false);
  const [selectedTemplateId, setSelectedTemplateId] = useState<string | null>(null);
  const [orderedTemplates, setOrderedTemplates] = useState<TerminalCommandTemplateRecord[]>([]);
  const [draggingTemplateId, setDraggingTemplateId] = useState<string | null>(null);
  const [editorDetectionResult, setEditorDetectionResult] =
    useState<EditorPathDetectionResult | null>(null);
  const [editorDraftPaths, setEditorDraftPaths] = useState<Record<EditorKey, string>>({
    vscode: "",
    jetbrains: "",
  });
  const [customEditorsDraft, setCustomEditorsDraft] = useState<CustomEditorRecord[]>([]);
  const [codexHistoryFontFamilyDraft, setCodexHistoryFontFamilyDraft] = useState(
    defaultAppPreferences.codexHistory.messageFontFamily,
  );
  const [codexAuthManualProxyUrlDraft, setCodexAuthManualProxyUrlDraft] = useState(
    defaultAppPreferences.codexAuth.proxy.manualProxyUrl,
  );

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
  const hotkeyHealthQuery = useQuery({
    queryKey: hotkeyHealthQueryKey,
    queryFn: getHotkeyHealth,
    enabled: desktopRuntimeAvailable,
    staleTime: 10_000,
  });
  const promptsQuery = useQuery({
    queryKey: promptsQueryKey,
    queryFn: listPrompts,
    enabled: desktopRuntimeAvailable && activeSection === "hotkeys",
    staleTime: 10_000,
  });
  const updatePreferencesMutation = useMutation({
    mutationFn: updateAppPreferences,
    onSuccess: (preferences) => {
      queryClient.setQueryData(appPreferencesQueryKey, preferences);
      void queryClient.invalidateQueries({ queryKey: hotkeyHealthQueryKey });
    },
  });
  const detectEditorsMutation = useMutation({
    mutationFn: detectEditorsFromPath,
  });
  const retryHotkeyMutation = useMutation({
    mutationFn: retryHotkeyRegistration,
    onSuccess: (health) => {
      queryClient.setQueryData(hotkeyHealthQueryKey, health);
      toast.success("全局热键已重新注册");
    },
    onError: (error) => {
      toast.error(formatHotkeyErrorSummary(getErrorSummary(error)));
      void hotkeyHealthQuery.refetch();
    },
  });

  const resolvedPreferences = preferencesQuery.data ?? defaultAppPreferences;
  const launchAtLoginEnabled = resolvedPreferences.startup.launchAtLogin;
  const startSilentlyEnabled = resolvedPreferences.startup.startSilently;
  const windowsTerminalPath = resolvedPreferences.terminal.windowsTerminalPath?.trim() ?? "";
  const terminalCommandShell =
    resolvedPreferences.terminal.commandShell ?? defaultAppPreferences.terminal.commandShell;
  const vscodePath = resolvedPreferences.ide.vscodePath?.trim() ?? "";
  const jetbrainsPath = resolvedPreferences.ide.jetbrainsPath?.trim() ?? "";
  const codexHistoryPreferences = resolveCodexHistoryPreferences(resolvedPreferences);
  const codexHistoryFontFamily = codexHistoryPreferences.messageFontFamily.trim();
  const codexHistoryFontSize = normalizeCodexHistoryFontSize(
    codexHistoryPreferences.messageFontSize,
  );
  const codexAuthPreferences = resolveCodexAuthPreferences(resolvedPreferences);
  const codexAuthQuotaRefreshIntervalSeconds =
    codexAuthPreferences.quotaRefreshIntervalSeconds;
  const codexAuthProxyPreferences = resolveCodexAuthProxyPreferences(
    codexAuthPreferences.proxy,
  );
  const codexAuthProxyMode = codexAuthProxyPreferences.mode;
  const codexAuthManualProxyUrl = codexAuthProxyPreferences.manualProxyUrl;
  const screenshotCaptureHotkey =
    resolvedPreferences.hotkey.screenshotCapture?.trim() || DEFAULT_SCREENSHOT_CAPTURE_HOTKEY;
  const screenshotHotkeyUsesCtrlAlt = isRiskyCtrlAltHotkey(screenshotCaptureHotkey);
  const promptQuickPasteSlots = resolvedPreferences.hotkey.promptQuickPasteSlots;
  const registeredPromptQuickPasteCount =
    hotkeyHealthQuery.data?.registeredBindings.filter((binding) =>
      binding.action.startsWith("prompt_quick_paste_"),
    ).length ?? 0;
  const configuredEditorCount = Number(Boolean(vscodePath)) + Number(Boolean(jetbrainsPath));
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
  const settingsPageLoading =
    activeSection === "general"
      ? preferencesQuery.isLoading || codexHomeQuery.isLoading
      : preferencesQuery.isLoading || hotkeyHealthQuery.isLoading;
  const hotkeyActionsDisabled =
    !desktopRuntimeAvailable || preferencesQuery.isError || updatePreferencesMutation.isPending;
  const preferenceActionsDisabled =
    !desktopRuntimeAvailable || preferencesQuery.isError || updatePreferencesMutation.isPending;

  useEffect(() => {
    setOrderedTemplates(terminalTemplates);
    latestOrderedTemplatesRef.current = terminalTemplates;
  }, [terminalTemplates]);

  useEffect(() => {
    setCodexHistoryFontFamilyDraft(codexHistoryFontFamily);
  }, [codexHistoryFontFamily]);

  useEffect(() => {
    setCodexAuthManualProxyUrlDraft(codexAuthManualProxyUrl);
  }, [codexAuthManualProxyUrl]);

  useEffect(() => {
    if (!isEditorManagerOpen) {
      return;
    }

    setEditorDraftPaths({
      vscode: vscodePath,
      jetbrains: jetbrainsPath,
    });
    setCustomEditorsDraft(resolvedPreferences.ide.customEditors ?? []);
  }, [
    isEditorManagerOpen,
    jetbrainsPath,
    resolvedPreferences.ide.customEditors,
    vscodePath,
  ]);

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

  function openEditorManager() {
    setIsEditorManagerOpen(true);
    setEditorInlineError(null);
    setEditorDraftPaths({
      vscode: vscodePath,
      jetbrains: jetbrainsPath,
    });
    setCustomEditorsDraft(resolvedPreferences.ide.customEditors ?? []);
  }

  function closeEditorManager() {
    setIsEditorManagerOpen(false);
    setEditorInlineError(null);
  }

  async function persistPreferences(patch: AppPreferencesPatch) {
    return updatePreferencesMutation.mutateAsync(patch);
  }

  async function handleToggleStartupSetting(
    settingKey: keyof AppPreferences["startup"],
    checked: boolean,
    successMessage: string,
  ) {
    if (
      !desktopRuntimeAvailable ||
      updatePreferencesMutation.isPending ||
      preferencesQuery.isError
    ) {
      return;
    }

    setStartupInlineError(null);

    try {
      const currentPreferences =
        queryClient.getQueryData<AppPreferences>(appPreferencesQueryKey) ??
        resolvedPreferences;
      const nextPreferences: AppPreferences = {
        ...currentPreferences,
        startup: {
          ...currentPreferences.startup,
          [settingKey]: checked,
        },
      };

      await persistPreferences({ startup: nextPreferences.startup });
      toast.success(successMessage);
    } catch (error) {
      const summary = getErrorSummary(error);
      setStartupInlineError(summary.message);
      toast.error(summary.message);
    }
  }

  async function persistTerminalTemplates(
    nextTemplates: TerminalCommandTemplateRecord[],
  ) {
    return persistPreferences({
      terminal: {
        ...resolvedPreferences.terminal,
        commandTemplates: assignTerminalCommandTemplateSortOrder(nextTemplates),
      },
    });
  }

  function setEditorDraftPath(editorKey: EditorKey, nextPath: string) {
    setEditorDraftPaths((current) => ({
      ...current,
      [editorKey]: nextPath,
    }));
  }

  async function handlePickEditorExecutable(editorKey: EditorKey) {
    if (
      !desktopRuntimeAvailable ||
      updatePreferencesMutation.isPending ||
      preferencesQuery.isError
    ) {
      return;
    }

    setEditorInlineError(null);

    try {
      const selectedExecutable = await open({
        directory: false,
        multiple: false,
        recursive: true,
        title: editorKey === "vscode" ? "选择 VS Code 可执行文件" : "选择 JetBrains 可执行文件",
        defaultPath: editorDraftPaths[editorKey] || undefined,
        filters: [{ name: "可执行文件", extensions: ["exe", "cmd", "bat"] }],
      });

      if (!selectedExecutable || Array.isArray(selectedExecutable)) {
        return;
      }

      setEditorDraftPath(editorKey, selectedExecutable.trim());
    } catch (error) {
      if (error instanceof CommandClientError) {
        return;
      }

      const summary = getErrorSummary(error);
      setEditorInlineError(summary.message);
      toast.error(summary.message);
    }
  }

  async function handleSaveEditorPath(editorKey: EditorKey) {
    if (
      !desktopRuntimeAvailable ||
      updatePreferencesMutation.isPending ||
      preferencesQuery.isError
    ) {
      return;
    }

    setEditorInlineError(null);

    try {
      const currentPreferences =
        queryClient.getQueryData<AppPreferences>(appPreferencesQueryKey) ??
        resolvedPreferences;
      const nextPath = editorDraftPaths[editorKey].trim();
      const nextPreferences: AppPreferences = {
        ...currentPreferences,
        ide: {
          ...currentPreferences.ide,
          vscodePath:
            editorKey === "vscode"
              ? nextPath || null
              : currentPreferences.ide.vscodePath ?? null,
          jetbrainsPath:
            editorKey === "jetbrains"
              ? nextPath || null
              : currentPreferences.ide.jetbrainsPath ?? null,
        },
      };

      await persistPreferences({ ide: nextPreferences.ide });
      toast.success(
        editorKey === "vscode" ? "VS Code 路径已保存" : "JetBrains IDE 路径已保存",
      );
    } catch (error) {
      const summary = getErrorSummary(error);
      setEditorInlineError(summary.message);
      toast.error(summary.message);
    }
  }

  async function handleClearEditorPath(editorKey: EditorKey) {
    if (
      !desktopRuntimeAvailable ||
      updatePreferencesMutation.isPending ||
      preferencesQuery.isError
    ) {
      return;
    }

    setEditorInlineError(null);

    try {
      const currentPreferences =
        queryClient.getQueryData<AppPreferences>(appPreferencesQueryKey) ??
        resolvedPreferences;
      const nextPreferences: AppPreferences = {
        ...currentPreferences,
        ide: {
          ...currentPreferences.ide,
          vscodePath:
            editorKey === "vscode" ? null : currentPreferences.ide.vscodePath ?? null,
          jetbrainsPath:
            editorKey === "jetbrains"
              ? null
              : currentPreferences.ide.jetbrainsPath ?? null,
        },
      };

      await persistPreferences({ ide: nextPreferences.ide });
      setEditorDraftPath(editorKey, "");
      toast.success(
        editorKey === "vscode"
          ? "VS Code 路径配置已删除"
          : "JetBrains IDE 路径配置已删除",
      );
    } catch (error) {
      const summary = getErrorSummary(error);
      setEditorInlineError(summary.message);
      toast.error(summary.message);
    }
  }

  function handleAddCustomEditor() {
    setEditorInlineError(null);
    setCustomEditorsDraft((current) => [
      ...current,
      {
        id: createCustomEditorId(),
        name: "",
        command: "",
      },
    ]);
  }

  function handleUpdateCustomEditor(
    editorId: string,
    field: "name" | "command",
    value: string,
  ) {
    setCustomEditorsDraft((current) =>
      current.map((editor) =>
        editor.id === editorId ? { ...editor, [field]: value } : editor,
      ),
    );
  }

  function handleRemoveCustomEditor(editorId: string) {
    setEditorInlineError(null);
    setCustomEditorsDraft((current) =>
      current.filter((editor) => editor.id !== editorId),
    );
  }

  async function handlePickCustomEditorCommand(editorId: string) {
    if (
      !desktopRuntimeAvailable ||
      updatePreferencesMutation.isPending ||
      preferencesQuery.isError
    ) {
      return;
    }

    setEditorInlineError(null);

    try {
      const selectedExecutable = await open({
        directory: false,
        multiple: false,
        recursive: true,
        title: "选择自定义编辑器可执行文件",
        filters: [{ name: "可执行文件", extensions: ["exe", "cmd", "bat"] }],
      });

      if (!selectedExecutable || Array.isArray(selectedExecutable)) {
        return;
      }

      handleUpdateCustomEditor(editorId, "command", selectedExecutable.trim());
    } catch (error) {
      if (error instanceof CommandClientError) {
        return;
      }

      const summary = getErrorSummary(error);
      setEditorInlineError(summary.message);
      toast.error(summary.message);
    }
  }

  async function handleSaveCustomEditors() {
    if (
      !desktopRuntimeAvailable ||
      updatePreferencesMutation.isPending ||
      preferencesQuery.isError
    ) {
      return;
    }

    setEditorInlineError(null);

    const normalizedEditors = customEditorsDraft.map((editor) => ({
      ...editor,
      id: editor.id.trim(),
      name: editor.name.trim(),
      command: editor.command.trim(),
    }));

    for (const editor of normalizedEditors) {
      if (!editor.name) {
        const message = "自定义编辑器名称不能为空";
        setEditorInlineError(message);
        toast.error(message);
        return;
      }

      if (!editor.command) {
        const message = `编辑器「${editor.name}」命令不能为空`;
        setEditorInlineError(message);
        toast.error(message);
        return;
      }
    }

    try {
      const currentPreferences =
        queryClient.getQueryData<AppPreferences>(appPreferencesQueryKey) ??
        resolvedPreferences;
      await persistPreferences({
        ide: {
          ...currentPreferences.ide,
          customEditors: normalizedEditors,
        },
      });
      toast.success("自定义编辑器列表已保存");
    } catch (error) {
      const summary = getErrorSummary(error);
      setEditorInlineError(summary.message);
      toast.error(summary.message);
    }
  }

  async function handleDetectEditorsFromPath() {
    if (
      !desktopRuntimeAvailable ||
      preferencesQuery.isError ||
      updatePreferencesMutation.isPending ||
      detectEditorsMutation.isPending
    ) {
      return;
    }

    setEditorInlineError(null);

    try {
      const detection = await detectEditorsMutation.mutateAsync();
      setEditorDetectionResult(detection);

      const currentPreferences =
        queryClient.getQueryData<AppPreferences>(appPreferencesQueryKey) ??
        resolvedPreferences;
      const detectedVscodePath = resolveDetectedEditorPath(detection.vscode);
      const detectedJetbrainsPath = resolveDetectedEditorPath(detection.jetbrains);
      const nextVscodePath = detectedVscodePath ?? currentPreferences.ide.vscodePath ?? null;
      const nextJetbrainsPath =
        detectedJetbrainsPath ?? currentPreferences.ide.jetbrainsPath ?? null;
      const shouldPersist =
        nextVscodePath !== (currentPreferences.ide.vscodePath ?? null) ||
        nextJetbrainsPath !== (currentPreferences.ide.jetbrainsPath ?? null);

      if (shouldPersist) {
        await persistPreferences({
          ide: {
            ...currentPreferences.ide,
            vscodePath: nextVscodePath,
            jetbrainsPath: nextJetbrainsPath,
          },
        });
      }
      setEditorDraftPaths({
        vscode: nextVscodePath ?? "",
        jetbrains: nextJetbrainsPath ?? "",
      });

      const detectedEditorCount =
        Number(Boolean(detectedVscodePath)) + Number(Boolean(detectedJetbrainsPath));
      if (detectedEditorCount > 0) {
        toast.success(`已从系统变量检测并更新 ${detectedEditorCount} 个编辑器路径`);
        return;
      }

      toast.warning("未在系统变量中检测到可用编辑器，已保留当前配置");
    } catch (error) {
      const summary = getErrorSummary(error);
      setEditorInlineError(summary.message);
      toast.error(summary.message);
    }
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

  async function handleSaveTerminalCommandShell(value: TerminalCommandShell) {
    if (value === terminalCommandShell || preferenceActionsDisabled) {
      return;
    }

    try {
      await persistPreferences({
        terminal: {
          ...resolvedPreferences.terminal,
          commandShell: value,
        },
      });
      setPathInlineError(null);
      toast.success(
        value === "powershell7"
          ? "终端命令 Shell 已切换为 PowerShell 7"
          : "终端命令 Shell 已切换为 cmd.exe",
      );
    } catch (error) {
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

  const applyRecordedScreenshotHotkey = useCallback(
    async (shortcut: string) => {
      if (
        !desktopRuntimeAvailable ||
        updatePreferencesMutation.isPending ||
        preferencesQuery.isError
      ) {
        return;
      }

      const normalizedShortcut = shortcut.trim();
      if (!normalizedShortcut) {
        setHotkeyRecorderError("截图热键不能为空");
        setHotkeyRecorderStatus("error");
        return;
      }

      setHotkeyInlineError(null);
      setHotkeyRecorderError(null);
      setHotkeyRecorderStatus("saving");

      try {
        const currentPreferences =
          queryClient.getQueryData<AppPreferences>(appPreferencesQueryKey) ??
          resolvedPreferences;
        await persistPreferences({
          hotkey: {
            ...currentPreferences.hotkey,
            screenshotCapture: normalizedShortcut,
          },
        });
        setHotkeyRecorderPreview(normalizedShortcut);
        setHotkeyRecorderStatus("idle");
        toast.success(`截图热键已更新为 ${normalizedShortcut}`);
        if (isRiskyCtrlAltHotkey(normalizedShortcut)) {
          toast.warning(
            `Windows 下 Ctrl+Alt 组合更容易与输入法或系统热键环境冲突，建议改用默认 ${DEFAULT_SCREENSHOT_CAPTURE_HOTKEY}。`,
          );
        }
      } catch (error) {
        const summary = getErrorSummary(error);
        const message = formatHotkeyErrorSummary(summary);
        setHotkeyInlineError(message);
        setHotkeyRecorderError(null);
        setHotkeyRecorderStatus("error");
      }
    },
    [
      desktopRuntimeAvailable,
      preferencesQuery.isError,
      queryClient,
      resolvedPreferences,
      updatePreferencesMutation.isPending,
    ],
  );

  function beginScreenshotHotkeyRecording() {
    if (
      !desktopRuntimeAvailable ||
      updatePreferencesMutation.isPending ||
      preferencesQuery.isError
    ) {
      return;
    }

    setHotkeyInlineError(null);
    setHotkeyRecorderError(null);
    setHotkeyRecorderPreview("");
    setHotkeyRecorderStatus("recording");
  }

  function cancelScreenshotHotkeyRecording() {
    setHotkeyRecorderPreview("");
    setHotkeyRecorderError(null);
    setHotkeyRecorderStatus("idle");
  }

  async function resetScreenshotHotkeyToDefault() {
    await applyRecordedScreenshotHotkey(DEFAULT_SCREENSHOT_CAPTURE_HOTKEY);
  }

  async function handleSavePromptQuickPasteSlots(slots: PromptQuickPasteHotkeySlot[]) {
    if (hotkeyActionsDisabled) {
      return;
    }
    setHotkeyInlineError(null);
    try {
      const currentPreferences =
        queryClient.getQueryData<AppPreferences>(appPreferencesQueryKey) ?? resolvedPreferences;
      await persistPreferences({
        hotkey: {
          ...currentPreferences.hotkey,
          promptQuickPasteSlots: slots,
        },
      });
    } catch (error) {
      const message = formatHotkeyErrorSummary(getErrorSummary(error));
      setHotkeyInlineError(message);
      throw error;
    }
  }

  async function handleSaveCodexHistoryFontFamily(
    nextValue = codexHistoryFontFamilyDraft,
  ) {
    if (preferenceActionsDisabled) {
      return;
    }

    const normalizedFontFamily = nextValue.trim();
    if (normalizedFontFamily.length > CODEX_HISTORY_FONT_FAMILY_MAX_LENGTH) {
      const message = "Codex 历史字体名称不能超过 160 个字符";
      setCodexHistoryInlineError(message);
      toast.error(message);
      return;
    }
    if (containsControlCharacter(normalizedFontFamily)) {
      const message = "Codex 历史字体名称不能包含换行或控制字符";
      setCodexHistoryInlineError(message);
      toast.error(message);
      return;
    }

    setCodexHistoryFontFamilyDraft(normalizedFontFamily);
    if (normalizedFontFamily === codexHistoryFontFamily) {
      return;
    }

    setCodexHistoryInlineError(null);

    try {
      const currentPreferences =
        queryClient.getQueryData<AppPreferences>(appPreferencesQueryKey) ??
        resolvedPreferences;
      await persistPreferences({
        codexHistory: {
          ...resolveCodexHistoryPreferences(currentPreferences),
          messageFontFamily: normalizedFontFamily,
        },
      });
      toast.success(
        normalizedFontFamily
          ? "Codex 历史字体已保存"
          : "Codex 历史字体已恢复系统默认",
      );
    } catch (error) {
      const summary = getErrorSummary(error);
      setCodexHistoryInlineError(summary.message);
      toast.error(summary.message);
    }
  }

  async function handleSaveCodexHistoryFontSize(value: number | string | null) {
    if (preferenceActionsDisabled || value === null) {
      return;
    }

    const numericValue = typeof value === "number" ? value : Number(value);
    if (!Number.isFinite(numericValue)) {
      return;
    }

    const nextFontSize = Math.round(numericValue);
    if (
      nextFontSize < CODEX_HISTORY_FONT_SIZE_MIN ||
      nextFontSize > CODEX_HISTORY_FONT_SIZE_MAX
    ) {
      const message = `Codex 历史字号必须在 ${CODEX_HISTORY_FONT_SIZE_MIN}px 到 ${CODEX_HISTORY_FONT_SIZE_MAX}px 之间`;
      setCodexHistoryInlineError(message);
      toast.error(message);
      return;
    }

    if (nextFontSize === codexHistoryFontSize) {
      return;
    }

    setCodexHistoryInlineError(null);

    try {
      const currentPreferences =
        queryClient.getQueryData<AppPreferences>(appPreferencesQueryKey) ??
        resolvedPreferences;
      await persistPreferences({
        codexHistory: {
          ...resolveCodexHistoryPreferences(currentPreferences),
          messageFontSize: nextFontSize,
        },
      });
      toast.success(`Codex 历史字号已设为 ${nextFontSize}px`);
    } catch (error) {
      const summary = getErrorSummary(error);
      setCodexHistoryInlineError(summary.message);
      toast.error(summary.message);
    }
  }

  async function handleSaveCodexAuthQuotaRefreshInterval(value: number | string | null) {
    if (preferenceActionsDisabled || value === null) {
      return;
    }

    const numericValue = typeof value === "number" ? value : Number(value);
    if (!Number.isFinite(numericValue)) {
      return;
    }

    const nextIntervalSeconds = Math.round(numericValue);
    if (
      nextIntervalSeconds < CODEX_AUTH_QUOTA_REFRESH_INTERVAL_MIN ||
      nextIntervalSeconds > CODEX_AUTH_QUOTA_REFRESH_INTERVAL_MAX
    ) {
      const message = `Codex Auth 额度刷新间隔必须在 ${CODEX_AUTH_QUOTA_REFRESH_INTERVAL_MIN} 秒到 ${CODEX_AUTH_QUOTA_REFRESH_INTERVAL_MAX} 秒之间`;
      setCodexAuthInlineError(message);
      toast.error(message);
      return;
    }

    if (nextIntervalSeconds === codexAuthQuotaRefreshIntervalSeconds) {
      return;
    }

    setCodexAuthInlineError(null);

    try {
      const currentPreferences =
        queryClient.getQueryData<AppPreferences>(appPreferencesQueryKey) ??
        resolvedPreferences;
      await persistPreferences({
        codexAuth: {
          ...resolveCodexAuthPreferences(currentPreferences),
          quotaRefreshIntervalSeconds: nextIntervalSeconds,
        },
      });
      toast.success(`Codex Auth 额度刷新间隔已设为 ${nextIntervalSeconds} 秒`);
    } catch (error) {
      const summary = getErrorSummary(error);
      setCodexAuthInlineError(summary.message);
      toast.error(summary.message);
    }
  }

  async function handleSaveCodexAuthProxyMode(nextMode: CodexAuthProxyMode) {
    if (preferenceActionsDisabled || nextMode === codexAuthProxyMode) {
      return;
    }

    const nextProxy = resolveCodexAuthProxyPreferences({
      ...codexAuthProxyPreferences,
      mode: nextMode,
    });

    if (nextProxy.mode === "manual" && !nextProxy.manualProxyUrl) {
      const message = "手动代理模式必须填写代理地址";
      setCodexAuthInlineError(message);
      toast.error(message);
      return;
    }

    setCodexAuthInlineError(null);

    try {
      const currentPreferences =
        queryClient.getQueryData<AppPreferences>(appPreferencesQueryKey) ??
        resolvedPreferences;
      await persistPreferences({
        codexAuth: {
          ...resolveCodexAuthPreferences(currentPreferences),
          proxy: nextProxy,
        },
      });
      toast.success(`Codex Auth 额度代理已切换为${formatCodexAuthProxyMode(nextMode)}`);
    } catch (error) {
      const summary = getErrorSummary(error);
      setCodexAuthInlineError(summary.message);
      toast.error(summary.message);
    }
  }

  async function handleSaveCodexAuthManualProxyUrl(value?: string) {
    if (preferenceActionsDisabled) {
      return;
    }

    const nextManualProxyUrl = (value ?? codexAuthManualProxyUrlDraft).trim();
    if (nextManualProxyUrl === codexAuthManualProxyUrl) {
      return;
    }

    if (nextManualProxyUrl.length > CODEX_AUTH_MANUAL_PROXY_URL_MAX_LENGTH) {
      const message = `Codex Auth 手动代理地址不能超过 ${CODEX_AUTH_MANUAL_PROXY_URL_MAX_LENGTH} 个字符`;
      setCodexAuthInlineError(message);
      toast.error(message);
      return;
    }
    if (containsControlCharacter(nextManualProxyUrl)) {
      const message = "Codex Auth 手动代理地址不能包含换行或控制字符";
      setCodexAuthInlineError(message);
      toast.error(message);
      return;
    }
    if (nextManualProxyUrl && !isSupportedCodexAuthProxyUrl(nextManualProxyUrl)) {
      const message = "Codex Auth 手动代理只支持 http、https、socks5、socks5h";
      setCodexAuthInlineError(message);
      toast.error(message);
      return;
    }
    if (codexAuthProxyMode === "manual" && !nextManualProxyUrl) {
      const message = "手动代理模式必须填写代理地址";
      setCodexAuthInlineError(message);
      toast.error(message);
      return;
    }

    setCodexAuthInlineError(null);

    try {
      const currentPreferences =
        queryClient.getQueryData<AppPreferences>(appPreferencesQueryKey) ??
        resolvedPreferences;
      await persistPreferences({
        codexAuth: {
          ...resolveCodexAuthPreferences(currentPreferences),
          proxy: {
            ...codexAuthProxyPreferences,
            manualProxyUrl: nextManualProxyUrl,
          },
        },
      });
      setCodexAuthManualProxyUrlDraft(nextManualProxyUrl);
      toast.success("Codex Auth 手动代理地址已保存");
    } catch (error) {
      const summary = getErrorSummary(error);
      setCodexAuthInlineError(summary.message);
      toast.error(summary.message);
    }
  }

  useHotkeyRecorder({
    active: hotkeyRecorderStatus === "recording",
    onCancel: cancelScreenshotHotkeyRecording,
    onComplete: applyRecordedScreenshotHotkey,
    onError: (message) => {
      setHotkeyRecorderError(message);
      setHotkeyRecorderStatus("error");
    },
    onPreviewChange: (preview) => {
      setHotkeyRecorderError(null);
      setHotkeyRecorderPreview(preview);
    },
  });

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

  const startupSwitchDisabled =
    !desktopRuntimeAvailable || preferencesQuery.isFetching || preferencesQuery.isError;

  return (
    <div className="flex min-h-full flex-col bg-surface">
      <header className="sticky top-0 z-10 flex h-10 shrink-0 items-center gap-4 border-b border-line bg-surface px-3">
        <h2 className="m-0 text-[13px] font-semibold text-fg">设置</h2>
        <nav aria-label="设置分组" className="flex h-full items-stretch gap-4 text-[13px]">
          {settingsSections.map((item) => {
            const active = item.key === activeSection;
            return (
              <Link
                aria-current={active ? "page" : undefined}
                className={cn(
                  "flex items-center border-b-2 transition-colors",
                  active ? "border-primary font-medium !text-fg" : "border-transparent !text-fg-3 hover:!text-fg-2",
                )}
                key={item.key}
                to={item.href}
              >
                {item.label}
              </Link>
            );
          })}
        </nav>
      </header>

      <div className="flex w-full max-w-[920px] flex-1 flex-col gap-5 px-4 pb-8 pt-4">
        {!desktopRuntimeAvailable ? (
          <Alert message="设置需要在 Bexo Studio 桌面应用中修改" showIcon type="info" />
        ) : null}

        {preferencesQuery.error ? (
          <Alert
            description={getErrorSummary(preferencesQuery.error).message}
            message="读取设置失败"
            showIcon
            type="error"
          />
        ) : null}

        {activeSection === "general" ? (
          <>
            {[
              { error: pathInlineError, title: "保存设置失败", clear: () => setPathInlineError(null) },
              { error: startupInlineError, title: "更新启动设置失败", clear: () => setStartupInlineError(null) },
              { error: codexHomeInlineError, title: "打开目录失败", clear: () => setCodexHomeInlineError(null) },
              { error: codexHistoryInlineError, title: "Codex 历史字体设置失败", clear: () => setCodexHistoryInlineError(null) },
              { error: codexAuthInlineError, title: "Codex 授权设置失败", clear: () => setCodexAuthInlineError(null) },
            ].map((item) =>
              item.error ? (
                <Alert
                  closable
                  description={item.error}
                  key={item.title}
                  message={item.title}
                  onClose={item.clear}
                  showIcon
                  type="error"
                />
              ) : null,
            )}

            {desktopRuntimeAvailable && settingsPageLoading ? (
              <div className="flex min-h-[240px] items-center justify-center">
                <Spin size="small" />
              </div>
            ) : (
              <div className="flex flex-col gap-5">
                <SettingsGroup title="启动">
                  <SettingsRow description="登录系统后自动启动 Bexo Studio" label="随系统启动">
                    <Switch
                      aria-label="随系统启动"
                      checked={launchAtLoginEnabled}
                      disabled={startupSwitchDisabled}
                      loading={updatePreferencesMutation.isPending}
                      onChange={(checked) =>
                        void handleToggleStartupSetting(
                          "launchAtLogin",
                          checked,
                          checked ? "已开启随系统启动" : "已关闭随系统启动",
                        )
                      }
                      size="small"
                    />
                  </SettingsRow>
                  <SettingsRow description="自启动时只驻留托盘，不弹出主窗口" label="静默启动">
                    <Switch
                      aria-label="静默启动"
                      checked={startSilentlyEnabled}
                      disabled={startupSwitchDisabled}
                      loading={updatePreferencesMutation.isPending}
                      onChange={(checked) =>
                        void handleToggleStartupSetting(
                          "startSilently",
                          checked,
                          checked ? "已开启静默启动" : "已关闭静默启动",
                        )
                      }
                      size="small"
                    />
                  </SettingsRow>
                </SettingsGroup>

                <SettingsGroup title="终端与编辑器">
                  <SettingsRow label="Windows Terminal 位置" wide>
                    <PathValue value={windowsTerminalPath || "未配置"} />
                    <Button
                      disabled={startupSwitchDisabled}
                      icon={<FolderOpenOutlined />}
                      loading={updatePreferencesMutation.isPending}
                      onClick={() => void handlePickWindowsTerminalDirectory()}
                    >
                      选择目录
                    </Button>
                  </SettingsRow>
                  <SettingsRow
                    description={
                      <Tooltip title="影响工作台启动命令和恢复任务中的 terminal_command；PowerShell 7 不可用时回退到 cmd.exe。">
                        <span>启动命令使用的 Shell</span>
                      </Tooltip>
                    }
                    label="终端命令 Shell"
                  >
                    <Select<TerminalCommandShell>
                      aria-label="终端命令 Shell"
                      className="!w-[140px]"
                      disabled={preferenceActionsDisabled}
                      onChange={(value) => void handleSaveTerminalCommandShell(value)}
                      options={TERMINAL_COMMAND_SHELL_OPTIONS}
                      value={terminalCommandShell}
                    />
                  </SettingsRow>
                  <SettingsRow description={`工作台“新增命令”可直接套用，已保存 ${terminalTemplates.length} 个`} label="终端命令模板">
                    <Button
                      disabled={!desktopRuntimeAvailable || preferencesQuery.isError}
                      icon={<SettingOutlined />}
                      onClick={openTemplateManager}
                    >
                      管理模板
                    </Button>
                  </SettingsRow>
                  <SettingsRow description={`VS Code / JetBrains / 自定义编辑器，已配置 ${configuredEditorCount} 个`} label="编辑器">
                    <Button
                      disabled={!desktopRuntimeAvailable || preferencesQuery.isError}
                      icon={<SettingOutlined />}
                      onClick={openEditorManager}
                    >
                      管理编辑器
                    </Button>
                  </SettingsRow>
                </SettingsGroup>

                <SettingsGroup title="Codex">
                  <SettingsRow description={codexHomeDescription} label="配置目录" wide>
                    <PathValue value={codexHomeDisplayValue} />
                    <Button
                      disabled={!canOpenCodexHomeDirectory}
                      icon={<FolderOpenOutlined />}
                      onClick={() => void handleOpenCodexHomeDirectory()}
                    >
                      打开
                    </Button>
                  </SettingsRow>
                  <SettingsRow description="留空使用系统默认，作用于会话历史和独立历史窗口" label="对话字体" wide>
                    <Input
                      allowClear
                      aria-label="Codex 对话字体"
                      className="min-w-0 flex-1"
                      disabled={preferenceActionsDisabled}
                      onBlur={() => void handleSaveCodexHistoryFontFamily()}
                      onChange={(event) => setCodexHistoryFontFamilyDraft(event.target.value)}
                      onPressEnter={(event) => event.currentTarget.blur()}
                      placeholder="系统默认"
                      value={codexHistoryFontFamilyDraft}
                    />
                    <Button
                      disabled={preferenceActionsDisabled || !codexHistoryFontFamily}
                      onClick={() => {
                        setCodexHistoryFontFamilyDraft("");
                        void handleSaveCodexHistoryFontFamily("");
                      }}
                    >
                      恢复默认
                    </Button>
                  </SettingsRow>
                  <SettingsRow
                    description={`默认 12px，范围 ${CODEX_HISTORY_FONT_SIZE_MIN}–${CODEX_HISTORY_FONT_SIZE_MAX}px`}
                    label="对话字号"
                  >
                    <InputNumber
                      addonAfter="px"
                      aria-label="Codex 对话字号"
                      className="!w-[120px]"
                      disabled={preferenceActionsDisabled}
                      max={CODEX_HISTORY_FONT_SIZE_MAX}
                      min={CODEX_HISTORY_FONT_SIZE_MIN}
                      onChange={(value) => void handleSaveCodexHistoryFontSize(value)}
                      precision={0}
                      value={codexHistoryFontSize}
                    />
                  </SettingsRow>
                  <SettingsRow
                    description={`自动轮询全部授权额度，默认 60 秒，范围 ${CODEX_AUTH_QUOTA_REFRESH_INTERVAL_MIN}–${CODEX_AUTH_QUOTA_REFRESH_INTERVAL_MAX} 秒`}
                    label="额度刷新间隔"
                  >
                    <InputNumber
                      addonAfter="秒"
                      aria-label="额度刷新间隔"
                      className="!w-[120px]"
                      disabled={preferenceActionsDisabled}
                      max={CODEX_AUTH_QUOTA_REFRESH_INTERVAL_MAX}
                      min={CODEX_AUTH_QUOTA_REFRESH_INTERVAL_MIN}
                      onChange={(value) => void handleSaveCodexAuthQuotaRefreshInterval(value)}
                      precision={0}
                      value={codexAuthQuotaRefreshIntervalSeconds}
                    />
                  </SettingsRow>
                  <SettingsRow description="只影响额度查询；系统代理读取 Windows 当前设置" label="额度查询代理">
                    <Select<CodexAuthProxyMode>
                      aria-label="额度查询代理"
                      className="!w-[140px]"
                      disabled={preferenceActionsDisabled}
                      onChange={(value) => void handleSaveCodexAuthProxyMode(value)}
                      options={CODEX_AUTH_PROXY_OPTIONS}
                      value={codexAuthProxyMode}
                    />
                  </SettingsRow>
                  <SettingsRow label="手动代理地址" wide>
                    <Input
                      allowClear
                      aria-label="手动代理地址"
                      className="min-w-0 flex-1"
                      disabled={preferenceActionsDisabled}
                      maxLength={CODEX_AUTH_MANUAL_PROXY_URL_MAX_LENGTH}
                      onBlur={() => void handleSaveCodexAuthManualProxyUrl()}
                      onChange={(event) => setCodexAuthManualProxyUrlDraft(event.currentTarget.value)}
                      onPressEnter={(event) => void handleSaveCodexAuthManualProxyUrl(event.currentTarget.value)}
                      placeholder="http://127.0.0.1:7890 或 socks5://127.0.0.1:7890"
                      value={codexAuthManualProxyUrlDraft}
                    />
                    <Button
                      disabled={
                        preferenceActionsDisabled ||
                        codexAuthManualProxyUrlDraft.trim() === codexAuthManualProxyUrl
                      }
                      onClick={() => void handleSaveCodexAuthManualProxyUrl()}
                    >
                      保存
                    </Button>
                  </SettingsRow>
                </SettingsGroup>
              </div>
            )}
          </>
        ) : (
          <>
            {hotkeyInlineError || hotkeyRecorderError ? (
              <Alert
                closable
                description={hotkeyRecorderError ?? hotkeyInlineError}
                message="热键设置失败"
                onClose={() => {
                  setHotkeyInlineError(null);
                  setHotkeyRecorderError(null);
                  if (hotkeyRecorderStatus === "error") {
                    setHotkeyRecorderStatus("idle");
                  }
                }}
                showIcon
                type="error"
              />
            ) : null}

            {hotkeyHealthQuery.data?.status === "degraded" ? (
              <Alert
                action={
                  <Button loading={retryHotkeyMutation.isPending} onClick={() => retryHotkeyMutation.mutate()} size="small">
                    重试注册
                  </Button>
                }
                description={
                  hotkeyHealthQuery.data.lastError?.message ??
                  "当前持久化快捷键未能注册，可能与其他程序或系统热键冲突。"
                }
                message="全局热键当前不可用"
                showIcon
                type="warning"
              />
            ) : hotkeyHealthQuery.isError ? (
              <Alert
                action={
                  <Button loading={hotkeyHealthQuery.isFetching} onClick={() => void hotkeyHealthQuery.refetch()} size="small">
                    重新检测
                  </Button>
                }
                description={getErrorSummary(hotkeyHealthQuery.error).message}
                message="无法读取全局热键状态"
                showIcon
                type="error"
              />
            ) : null}

            {desktopRuntimeAvailable && settingsPageLoading ? (
              <div className="flex min-h-[240px] items-center justify-center">
                <Spin size="small" />
              </div>
            ) : (
              <div className="flex flex-col gap-5">
                <SettingsGroup title="截图">
                  <SettingsRow
                    description={
                      hotkeyRecorderStatus === "recording"
                        ? hotkeyRecorderPreview
                          ? `录制中：${hotkeyRecorderPreview}（松开按键后保存）`
                          : "录制中：请按下新的组合键，Esc 取消"
                        : (
                          <Tooltip title="必须包含一个非修饰键，支持左右修饰键组合；保存后立即生效。">
                            <span>
                              默认 {DEFAULT_SCREENSHOT_CAPTURE_HOTKEY}
                              {hotkeyHealthQuery.data?.status === "ready" ? (
                                <span className="ml-2 text-success">
                                  已注册 {hotkeyHealthQuery.data.registeredBindings.length} 个全局热键
                                </span>
                              ) : null}
                            </span>
                          </Tooltip>
                        )
                    }
                    label="截图热键"
                  >
                    <kbd className="rounded border border-line bg-surface-2 px-2 py-0.5 font-mono text-[12px] text-fg" title={screenshotCaptureHotkey}>
                      {screenshotCaptureHotkey}
                    </kbd>
                    <Button
                      disabled={hotkeyActionsDisabled || hotkeyRecorderStatus === "saving"}
                      loading={hotkeyRecorderStatus === "saving"}
                      onClick={
                        hotkeyRecorderStatus === "recording"
                          ? cancelScreenshotHotkeyRecording
                          : beginScreenshotHotkeyRecording
                      }
                      type={hotkeyRecorderStatus === "recording" ? "primary" : "default"}
                    >
                      {hotkeyRecorderStatus === "recording" ? "取消录制" : "录制"}
                    </Button>
                    <Button
                      disabled={
                        hotkeyActionsDisabled ||
                        hotkeyRecorderStatus === "saving" ||
                        screenshotCaptureHotkey === DEFAULT_SCREENSHOT_CAPTURE_HOTKEY
                      }
                      onClick={() => void resetScreenshotHotkeyToDefault()}
                    >
                      恢复默认
                    </Button>
                  </SettingsRow>
                  {screenshotHotkeyUsesCtrlAlt ? (
                    <div className="px-3 py-2">
                      <Alert
                        description={`Windows 下这类组合容易与输入法或系统热键冲突，建议改用默认 ${DEFAULT_SCREENSHOT_CAPTURE_HOTKEY} 或不含 Alt 的组合。`}
                        message="当前截图热键使用了 Ctrl+Alt 组合"
                        showIcon
                        type="warning"
                      />
                    </div>
                  ) : null}
                </SettingsGroup>

                <PromptQuickPasteSettings
                  disabled={hotkeyActionsDisabled}
                  loadingPrompts={promptsQuery.isLoading || promptsQuery.isFetching}
                  onSave={handleSavePromptQuickPasteSlots}
                  promptLoadError={promptsQuery.isError ? getErrorSummary(promptsQuery.error).message : null}
                  prompts={promptsQuery.data ?? []}
                  registeredCount={registeredPromptQuickPasteCount}
                  slots={promptQuickPasteSlots}
                />
              </div>
            )}
          </>
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
              <Typography.Text className="text-[12px] font-semibold text-fg">
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
              <div className="flex min-h-[220px] items-center justify-center rounded-[12px] border border-dashed border-line bg-surface-2">
                <Empty
                  description="还没有终端模板，先点右上角“新建”。"
                  image={Empty.PRESENTED_IMAGE_SIMPLE}
                />
              </div>
            )}
          </div>

          <div className="flex min-h-[420px] flex-col rounded-[12px] border border-line bg-surface-2 p-4">
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
              <Typography.Text className="text-[12px] font-medium text-fg">
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
                <Typography.Text className="block text-[11px] text-danger">
                  {templateForm.formState.errors.name.message}
                </Typography.Text>
              ) : null}
            </div>

            <div className="mt-4 space-y-1">
              <Typography.Text className="text-[12px] font-medium text-fg">
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
                <Typography.Text className="block text-[11px] text-danger">
                  {templateForm.formState.errors.commandLine.message}
                </Typography.Text>
              ) : null}
              <Typography.Text className="block text-[11px] text-fg-3">
                模板会同步出现在首页“新增命令”的模板下拉里。
              </Typography.Text>
            </div>

            <div className="mt-auto flex items-center justify-end gap-2 border-t border-line pt-4">
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

      <Modal
        cancelText="关闭"
        footer={null}
        onCancel={closeEditorManager}
        open={isEditorManagerOpen}
        title="编辑器管理"
        width={760}
      >
        <div className="space-y-4 pt-1">
          {editorInlineError ? (
            <Alert
              closable
              message="自动检测失败"
              onClose={() => setEditorInlineError(null)}
              showIcon
              type="error"
              description={editorInlineError}
            />
          ) : null}

          <div className="rounded-[12px] border border-line bg-surface-2 p-4">
            <div className="flex items-center justify-between gap-3">
              <div className="min-w-0">
                <Typography.Text className="block text-[12px] font-medium text-fg">
                  PATH 自动检测
                </Typography.Text>
                <Typography.Text className="mt-1 block text-[11px] text-fg-2">
                  点击后会从系统变量 PATH 探测 VS Code / JetBrains 并自动写回当前设置。
                </Typography.Text>
                {editorDetectionResult ? (
                  <Typography.Text className="mt-1 block text-[11px] text-fg-3">
                    最近检测时间：{editorDetectionResult.checkedAt}
                  </Typography.Text>
                ) : null}
              </div>

              <Button
                className="!h-[32px] !px-3 !text-[12px]"
                disabled={
                  !desktopRuntimeAvailable ||
                  preferencesQuery.isError ||
                  detectEditorsMutation.isPending ||
                  updatePreferencesMutation.isPending
                }
                icon={<SearchOutlined />}
                loading={detectEditorsMutation.isPending || updatePreferencesMutation.isPending}
                onClick={() => void handleDetectEditorsFromPath()}
                size="small"
                type="primary"
              >
                从系统变量自动检测
              </Button>
            </div>
          </div>

          <EditorDetectionCard
            configuredPath={vscodePath}
            detection={editorDetectionResult?.vscode}
            disabled={!desktopRuntimeAvailable || preferencesQuery.isError}
            draftPath={editorDraftPaths.vscode}
            loading={updatePreferencesMutation.isPending}
            label="VS Code"
            onDeletePath={() => void handleClearEditorPath("vscode")}
            onDraftPathChange={(value) => setEditorDraftPath("vscode", value)}
            onPickPath={() => void handlePickEditorExecutable("vscode")}
            onSavePath={() => void handleSaveEditorPath("vscode")}
          />
          <EditorDetectionCard
            configuredPath={jetbrainsPath}
            detection={editorDetectionResult?.jetbrains}
            disabled={!desktopRuntimeAvailable || preferencesQuery.isError}
            draftPath={editorDraftPaths.jetbrains}
            loading={updatePreferencesMutation.isPending}
            label="JetBrains IDE"
            onDeletePath={() => void handleClearEditorPath("jetbrains")}
            onDraftPathChange={(value) => setEditorDraftPath("jetbrains", value)}
            onPickPath={() => void handlePickEditorExecutable("jetbrains")}
            onSavePath={() => void handleSaveEditorPath("jetbrains")}
          />

          <div className="rounded-[12px] border border-line bg-surface-2 p-4">
            <div className="flex items-center justify-between gap-3">
              <div className="min-w-0">
                <Typography.Text className="block text-[12px] font-medium text-fg">
                  自定义编辑器列表
                </Typography.Text>
                <Typography.Text className="mt-1 block text-[11px] text-fg-2">
                  支持手动新增任意编辑器命令（例如：asstudio64），并可增删改后统一保存。
                </Typography.Text>
              </div>
              <Button
                className="!h-[30px] !px-3 !text-[12px]"
                disabled={!desktopRuntimeAvailable || preferencesQuery.isError}
                icon={<PlusOutlined />}
                onClick={handleAddCustomEditor}
                size="small"
                type="default"
              >
                新增编辑器
              </Button>
            </div>

            <div className="mt-3 space-y-2">
              {customEditorsDraft.length ? (
                customEditorsDraft.map((editor) => (
                  <div
                    className="rounded-[10px] border border-line bg-surface p-3"
                    key={editor.id}
                  >
                    <div className="grid grid-cols-[minmax(0,220px)_minmax(0,1fr)_auto] items-center gap-2">
                      <Input
                        disabled={
                          !desktopRuntimeAvailable ||
                          preferencesQuery.isError ||
                          updatePreferencesMutation.isPending
                        }
                        onChange={(event) =>
                          handleUpdateCustomEditor(editor.id, "name", event.target.value)
                        }
                        placeholder="编辑器名称，例如 Android Studio"
                        value={editor.name}
                      />
                      <Input
                        disabled={
                          !desktopRuntimeAvailable ||
                          preferencesQuery.isError ||
                          updatePreferencesMutation.isPending
                        }
                        onChange={(event) =>
                          handleUpdateCustomEditor(editor.id, "command", event.target.value)
                        }
                        placeholder="命令或绝对路径，例如 asstudio64 或 C:\\IDE\\asstudio64.exe"
                        value={editor.command}
                      />
                      <div className="flex items-center gap-2">
                        <Button
                          className="!h-[30px] !px-3 !text-[12px]"
                          disabled={
                            !desktopRuntimeAvailable ||
                            preferencesQuery.isError ||
                            updatePreferencesMutation.isPending
                          }
                          icon={<FolderOpenOutlined />}
                          onClick={() => void handlePickCustomEditorCommand(editor.id)}
                          size="small"
                          type="default"
                        >
                          选文件
                        </Button>
                        <Popconfirm
                          description="删除后仅在保存列表后生效。"
                          okButtonProps={{ danger: true }}
                          okText="删除"
                          onConfirm={() => handleRemoveCustomEditor(editor.id)}
                          title="确认删除这条自定义编辑器吗？"
                        >
                          <Button
                            danger
                            icon={<DeleteOutlined />}
                            size="small"
                            type="default"
                          >
                            删除
                          </Button>
                        </Popconfirm>
                      </div>
                    </div>
                  </div>
                ))
              ) : (
                <div className="rounded-[10px] border border-dashed border-line-strong bg-surface px-3 py-5 text-center">
                  <Typography.Text className="text-[12px] text-fg-3">
                    暂无自定义编辑器，点击右上角“新增编辑器”开始添加。
                  </Typography.Text>
                </div>
              )}
            </div>

            <div className="mt-3 flex justify-end border-t border-line pt-3">
              <Button
                className="!h-[30px] !px-3 !text-[12px]"
                disabled={!desktopRuntimeAvailable || preferencesQuery.isError}
                loading={updatePreferencesMutation.isPending}
                onClick={() => void handleSaveCustomEditors()}
                size="small"
                type="primary"
              >
                保存自定义列表
              </Button>
            </div>
          </div>

          <div className="flex justify-end border-t border-line pt-3">
            <Button onClick={closeEditorManager} size="small" type="default">
              关闭
            </Button>
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
          ? "border-primary bg-selected"
          : "border-line bg-surface-2 hover:border-line hover:bg-hover",
        dragging && "border-primary bg-selected opacity-95",
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
          className="mt-0.5 flex h-6 w-6 cursor-grab touch-none items-center justify-center rounded-[6px] border border-line-strong bg-surface text-fg-2 active:cursor-grabbing"
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
            <Typography.Text className="block text-[12px] font-medium text-fg">
              {template.name}
            </Typography.Text>
            <Typography.Text className="text-[10px] text-fg-3">
              #{template.sortOrder + 1}
            </Typography.Text>
          </div>
          <Typography.Text className="mt-1 block truncate font-mono text-[11px] text-fg-2">
            {template.commandLine}
          </Typography.Text>
        </button>
      </div>
    </Reorder.Item>
  );
}

type EditorDetectionCardProps = {
  configuredPath: string;
  detection?: AdapterAvailability;
  disabled: boolean;
  draftPath: string;
  label: string;
  loading: boolean;
  onDeletePath: () => void;
  onDraftPathChange: (value: string) => void;
  onPickPath: () => void;
  onSavePath: () => void;
};

function EditorDetectionCard({
  configuredPath,
  detection,
  disabled,
  draftPath,
  label,
  loading,
  onDeletePath,
  onDraftPathChange,
  onPickPath,
  onSavePath,
}: EditorDetectionCardProps) {
  const statusText = detection
    ? detection.available
      ? "已检测到"
      : detection.status === "invalid"
        ? "路径无效"
        : "未检测到"
    : "未检测";
  const statusClassName = detection
    ? detection.available
      ? "border-success/40 bg-success-soft text-success"
      : "border-danger/40 bg-danger-soft text-danger"
    : "border-line-strong bg-surface-2 text-fg-2";
  const detectedPath = detection?.executablePath?.trim() ?? "";

  return (
    <div className="rounded-[12px] border border-line bg-surface-2 p-4">
      <div className="flex items-center justify-between gap-3">
        <Typography.Text className="text-[12px] font-medium text-fg">
          {label}
        </Typography.Text>
        <span
          className={cn(
            "inline-flex rounded-[999px] border px-2 py-0.5 text-[10px] font-medium",
            statusClassName,
          )}
        >
          {statusText}
        </span>
      </div>

      <div className="mt-3 space-y-2">
        <div>
          <Typography.Text className="mb-1 block text-[11px] text-fg-2">
            当前配置路径
          </Typography.Text>
          <div
            className="flex h-[32px] items-center overflow-hidden rounded-[8px] border border-line-strong bg-surface px-3 font-mono text-[12px] text-fg-2"
            title={configuredPath || "未配置"}
          >
            <span className="truncate">{configuredPath || "未配置"}</span>
          </div>
        </div>

        <div>
          <Typography.Text className="mb-1 block text-[11px] text-fg-2">
            最近检测路径
          </Typography.Text>
          <div
            className="flex h-[32px] items-center overflow-hidden rounded-[8px] border border-line-strong bg-surface px-3 font-mono text-[12px] text-fg-2"
            title={detectedPath || "未检测到可执行路径"}
          >
            <span className="truncate">{detectedPath || "未检测到可执行路径"}</span>
          </div>
        </div>
      </div>

      <div className="mt-3 space-y-2 border-t border-line pt-3">
        <Typography.Text className="block text-[11px] text-fg-2">
          手动配置路径
        </Typography.Text>
        <Input
          disabled={disabled || loading}
          onChange={(event) => onDraftPathChange(event.target.value)}
          placeholder="输入可执行文件绝对路径，例如 C:\\Tools\\code.cmd"
          value={draftPath}
        />
        <div className="flex flex-wrap justify-end gap-2">
          <Button
            className="!h-[30px] !px-3 !text-[12px]"
            disabled={disabled || loading}
            icon={<FolderOpenOutlined />}
            onClick={onPickPath}
            size="small"
            type="default"
          >
            选择文件
          </Button>
          <Button
            className="!h-[30px] !px-3 !text-[12px]"
            disabled={disabled || loading}
            loading={loading}
            onClick={onSavePath}
            size="small"
            type="primary"
          >
            保存路径
          </Button>
          <Popconfirm
            description="删除后会清空该编辑器的手动路径配置。"
            okButtonProps={{ danger: true, loading }}
            okText="删除"
            onConfirm={onDeletePath}
            title="确认删除这条路径配置吗？"
          >
            <Button
              danger
              disabled={disabled || loading}
              icon={<DeleteOutlined />}
              size="small"
              type="default"
            >
              删除配置
            </Button>
          </Popconfirm>
        </div>
      </div>

      {detection ? (
        <Typography.Text className="mt-2 block text-[11px] text-fg-2">
          {`来源：${detection.source} · ${detection.message}`}
        </Typography.Text>
      ) : null}
    </div>
  );
}

function resolveDetectedEditorPath(availability: AdapterAvailability): string | null {
  if (!availability.available) {
    return null;
  }

  const executablePath = availability.executablePath?.trim();
  return executablePath ? executablePath : null;
}

function formatHotkeyErrorSummary(summary: ReturnType<typeof getErrorSummary>) {
  const shortcut = summary.details?.shortcut?.trim();
  const reason = summary.details?.reason?.trim();
  const normalizedReason = reason?.toLowerCase() ?? "";

  if (normalizedReason.includes("already registered")) {
    return shortcut
      ? `热键已被占用，请更换组合键：${shortcut}`
      : "热键已被占用，请更换组合键。";
  }

  if (shortcut && reason) {
    return `${summary.message}：${shortcut}（${reason}）`;
  }

  if (shortcut) {
    return `${summary.message}：${shortcut}`;
  }

  if (reason && reason !== summary.message) {
    return `${summary.message}：${reason}`;
  }

  return summary.message;
}

function resolveCodexHistoryPreferences(
  preferences: AppPreferences,
): CodexHistoryViewPreferences {
  return {
    ...defaultAppPreferences.codexHistory,
    ...(preferences.codexHistory ?? {}),
    messageFontFamily:
      preferences.codexHistory?.messageFontFamily?.trim() ??
      defaultAppPreferences.codexHistory.messageFontFamily,
    messageFontSize: normalizeCodexHistoryFontSize(
      preferences.codexHistory?.messageFontSize,
    ),
  };
}

function normalizeCodexHistoryFontSize(value?: number | null) {
  if (!Number.isFinite(value)) {
    return defaultAppPreferences.codexHistory.messageFontSize;
  }
  const rounded = Math.round(value as number);
  if (
    rounded < CODEX_HISTORY_FONT_SIZE_MIN ||
    rounded > CODEX_HISTORY_FONT_SIZE_MAX
  ) {
    return defaultAppPreferences.codexHistory.messageFontSize;
  }
  return rounded;
}

function resolveCodexAuthPreferences(preferences: AppPreferences): CodexAuthPreferences {
  const proxy = resolveCodexAuthProxyPreferences(preferences.codexAuth?.proxy);
  return {
    ...defaultAppPreferences.codexAuth,
    ...(preferences.codexAuth ?? {}),
    proxy,
    quotaRefreshIntervalSeconds: normalizeCodexAuthQuotaRefreshInterval(
      preferences.codexAuth?.quotaRefreshIntervalSeconds,
    ),
  };
}

function resolveCodexAuthProxyPreferences(
  preferences?: Partial<CodexAuthProxyPreferences> | null,
): CodexAuthProxyPreferences {
  const mode = normalizeCodexAuthProxyMode(preferences?.mode);
  const manualProxyUrl = (preferences?.manualProxyUrl ?? "").trim();
  return {
    ...defaultAppPreferences.codexAuth.proxy,
    mode,
    manualProxyUrl,
  };
}

function normalizeCodexAuthProxyMode(
  value?: string | null,
): CodexAuthProxyMode {
  if (value === "manual" || value === "disabled" || value === "system") {
    return value;
  }
  return defaultAppPreferences.codexAuth.proxy.mode;
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

function formatCodexAuthProxyMode(mode: CodexAuthProxyMode) {
  switch (mode) {
    case "system":
      return "系统代理";
    case "manual":
      return "手动代理";
    case "disabled":
      return "不使用代理";
  }
}

function isSupportedCodexAuthProxyUrl(value: string) {
  const lowerValue = value.trim().toLowerCase();
  return (
    lowerValue.startsWith("http://") ||
    lowerValue.startsWith("https://") ||
    lowerValue.startsWith("socks5://") ||
    lowerValue.startsWith("socks5h://")
  );
}

function containsControlCharacter(value: string) {
  return /[\u0000-\u001f\u007f]/.test(value);
}

function normalizeHotkeyModifierToken(token: string) {
  const normalized = token.trim().toLowerCase();
  switch (normalized) {
    case "lctrl":
    case "rctrl":
      return "ctrl";
    case "lalt":
    case "ralt":
      return "alt";
    case "lshift":
    case "rshift":
      return "shift";
    case "lwin":
    case "rwin":
      return "super";
    default:
      return normalized;
  }
}

function isHotkeyModifierToken(token: string) {
  return ["ctrl", "alt", "shift", "super", "win"].includes(
    normalizeHotkeyModifierToken(token),
  );
}

function isRiskyCtrlAltHotkey(shortcut: string) {
  const tokens = shortcut
    .split("+")
    .map((token) => token.trim())
    .filter(Boolean);

  const normalized = new Set(tokens.map((token) => normalizeHotkeyModifierToken(token)));
  return (
    normalized.has("ctrl") &&
    normalized.has("alt") &&
    tokens.some((token) => !isHotkeyModifierToken(token))
  );
}

function createCustomEditorId() {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }

  return `editor-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
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
