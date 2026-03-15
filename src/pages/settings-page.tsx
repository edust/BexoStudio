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
  Modal,
  Popconfirm,
  Spin,
  Switch,
  Typography,
} from "antd";
import { Reorder, useDragControls } from "motion/react";
import { Controller, useForm } from "react-hook-form";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useParams } from "react-router-dom";
import { toast } from "sonner";

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
import { reorderLayoutTransition } from "@/lib/reorder-motion";
import {
  appPreferencesQueryKey,
  codexHomeDirectoryQueryKey,
  getAppPreferences,
  getCodexHomeDirectory,
  updateAppPreferences,
} from "@/queries/preferences";
import type {
  AdapterAvailability,
  AppPreferences,
  CustomEditorRecord,
  EditorPathDetectionResult,
  TerminalCommandTemplateRecord,
} from "@/types/backend";

type EditorKey = "vscode" | "jetbrains";
type SettingsSection = "general" | "hotkeys";
type HotkeyRecorderStatus = "idle" | "recording" | "saving" | "error";
type ScreenshotToolHotkeyKey = keyof AppPreferences["hotkey"]["screenshotTools"];

const DEFAULT_SCREENSHOT_CAPTURE_HOTKEY = "Ctrl+Shift+X";
const DEFAULT_SCREENSHOT_TOOL_HOTKEYS: AppPreferences["hotkey"]["screenshotTools"] = {
  select: "1",
  line: "2",
  rect: "3",
  ellipse: "4",
  arrow: "5",
};
const SCREENSHOT_TOOL_HOTKEY_ITEMS: Array<{
  key: ScreenshotToolHotkeyKey;
  label: string;
  description: string;
}> = [
  { key: "select", label: "选区工具", description: "默认 1" },
  { key: "line", label: "线条工具", description: "默认 2" },
  { key: "rect", label: "矩形工具", description: "默认 3" },
  { key: "ellipse", label: "圆形工具", description: "默认 4" },
  { key: "arrow", label: "箭头工具", description: "默认 5" },
];
const HOTKEY_MODIFIER_TOKENS = new Set([
  "Ctrl",
  "Alt",
  "Shift",
  "Super",
  "LCtrl",
  "RCtrl",
  "LAlt",
  "RAlt",
  "LWin",
  "RWin",
  "LShift",
  "RShift",
]);

export default function SettingsPage() {
  const { section } = useParams<{ section?: string }>();
  const activeSection: SettingsSection = section === "hotkeys" ? "hotkeys" : "general";
  const [pathInlineError, setPathInlineError] = useState<string | null>(null);
  const [startupInlineError, setStartupInlineError] = useState<string | null>(null);
  const [codexHomeInlineError, setCodexHomeInlineError] = useState<string | null>(null);
  const [templateInlineError, setTemplateInlineError] = useState<string | null>(null);
  const [editorInlineError, setEditorInlineError] = useState<string | null>(null);
  const [hotkeyInlineError, setHotkeyInlineError] = useState<string | null>(null);
  const [hotkeyRecorderStatus, setHotkeyRecorderStatus] =
    useState<HotkeyRecorderStatus>("idle");
  const [hotkeyRecorderPreview, setHotkeyRecorderPreview] = useState("");
  const [hotkeyRecorderError, setHotkeyRecorderError] = useState<string | null>(null);
  const [screenshotToolHotkeyDrafts, setScreenshotToolHotkeyDrafts] =
    useState<AppPreferences["hotkey"]["screenshotTools"]>(DEFAULT_SCREENSHOT_TOOL_HOTKEYS);
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
  const detectEditorsMutation = useMutation({
    mutationFn: detectEditorsFromPath,
  });

  const resolvedPreferences = preferencesQuery.data ?? defaultAppPreferences;
  const launchAtLoginEnabled = resolvedPreferences.startup.launchAtLogin;
  const startSilentlyEnabled = resolvedPreferences.startup.startSilently;
  const windowsTerminalPath = resolvedPreferences.terminal.windowsTerminalPath?.trim() ?? "";
  const vscodePath = resolvedPreferences.ide.vscodePath?.trim() ?? "";
  const jetbrainsPath = resolvedPreferences.ide.jetbrainsPath?.trim() ?? "";
  const screenshotCaptureHotkey =
    resolvedPreferences.hotkey.screenshotCapture?.trim() || DEFAULT_SCREENSHOT_CAPTURE_HOTKEY;
  const screenshotToolHotkeys = useMemo(
    () => resolveScreenshotToolHotkeys(resolvedPreferences.hotkey.screenshotTools),
    [resolvedPreferences.hotkey.screenshotTools],
  );
  const screenshotHotkeyUsesCtrlAlt = isRiskyCtrlAltHotkey(screenshotCaptureHotkey);
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
      : preferencesQuery.isLoading;
  const hotkeyActionsDisabled =
    !desktopRuntimeAvailable || preferencesQuery.isError || updatePreferencesMutation.isPending;

  useEffect(() => {
    setOrderedTemplates(terminalTemplates);
    latestOrderedTemplatesRef.current = terminalTemplates;
  }, [terminalTemplates]);

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

  useEffect(() => {
    setScreenshotToolHotkeyDrafts((current) =>
      isSameScreenshotToolHotkeys(current, screenshotToolHotkeys)
        ? current
        : screenshotToolHotkeys,
    );
  }, [screenshotToolHotkeys]);

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

  async function persistPreferences(nextPreferences: AppPreferences) {
    return updatePreferencesMutation.mutateAsync(nextPreferences);
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

      await persistPreferences(nextPreferences);
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
      ...resolvedPreferences,
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

      await persistPreferences(nextPreferences);
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

      await persistPreferences(nextPreferences);
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
        ...currentPreferences,
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
          ...currentPreferences,
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
          ...currentPreferences,
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

  function setScreenshotToolHotkeyDraft(
    key: ScreenshotToolHotkeyKey,
    value: string,
  ) {
    setScreenshotToolHotkeyDrafts((current) => ({
      ...current,
      [key]: value,
    }));
  }

  async function saveScreenshotToolHotkeys() {
    if (
      !desktopRuntimeAvailable ||
      updatePreferencesMutation.isPending ||
      preferencesQuery.isError
    ) {
      return;
    }

    setHotkeyInlineError(null);
    setHotkeyRecorderError(null);

    const normalized = normalizeScreenshotToolHotkeys(screenshotToolHotkeyDrafts);
    const duplicate = findDuplicateScreenshotToolHotkey(normalized);
    if (duplicate) {
      const message = `截图工具热键冲突：${duplicate.label} 与 ${duplicate.conflictLabel} 不能重复。`;
      setHotkeyInlineError(message);
      toast.error(message);
      return;
    }

    if (
      screenshotCaptureHotkey &&
      isSameHotkeyShortcut(screenshotCaptureHotkey, normalized.select)
    ) {
      const message = "截图热键不能与“选区工具”热键重复，请调整后再保存。";
      setHotkeyInlineError(message);
      toast.error(message);
      return;
    }

    try {
      const currentPreferences =
        queryClient.getQueryData<AppPreferences>(appPreferencesQueryKey) ??
        resolvedPreferences;

      await persistPreferences({
        ...currentPreferences,
        hotkey: {
          ...currentPreferences.hotkey,
          screenshotTools: normalized,
        },
      });

      toast.success("截图工具热键已保存");
    } catch (error) {
      const summary = getErrorSummary(error);
      const message = formatHotkeyErrorSummary(summary);
      setHotkeyInlineError(message);
      toast.error(message);
    }
  }

  async function resetScreenshotToolHotkeysToDefault() {
    setScreenshotToolHotkeyDrafts(DEFAULT_SCREENSHOT_TOOL_HOTKEYS);
    if (
      !desktopRuntimeAvailable ||
      updatePreferencesMutation.isPending ||
      preferencesQuery.isError
    ) {
      return;
    }

    setHotkeyInlineError(null);
    setHotkeyRecorderError(null);

    try {
      const currentPreferences =
        queryClient.getQueryData<AppPreferences>(appPreferencesQueryKey) ??
        resolvedPreferences;
      await persistPreferences({
        ...currentPreferences,
        hotkey: {
          ...currentPreferences.hotkey,
          screenshotTools: DEFAULT_SCREENSHOT_TOOL_HOTKEYS,
        },
      });
      toast.success("截图工具热键已恢复默认");
    } catch (error) {
      const summary = getErrorSummary(error);
      const message = formatHotkeyErrorSummary(summary);
      setHotkeyInlineError(message);
      toast.error(message);
    }
  }

  useEffect(() => {
    if (hotkeyRecorderStatus !== "recording") {
      return;
    }

    const activeTokens = new Set<string>();
    const seenTokens = new Set<string>();
    let ignoredUnsupportedKey = false;

    const finishRecording = (combo: string) => {
      if (!combo) {
        return;
      }

      const tokens = combo.split("+").filter(Boolean);
      if (!tokens.some((token) => !isHotkeyModifierToken(token))) {
        setHotkeyRecorderError(
          `截图热键至少包含一个非修饰键，建议使用默认 ${DEFAULT_SCREENSHOT_CAPTURE_HOTKEY}。`,
        );
        setHotkeyRecorderStatus("error");
        return;
      }

      void applyRecordedScreenshotHotkey(combo);
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.repeat) {
        return;
      }

      if (event.code === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        cancelScreenshotHotkeyRecording();
        return;
      }

      const token = keyboardEventToHotkeyToken(event);
      if (!token) {
        ignoredUnsupportedKey = true;
        setHotkeyRecorderError(
          `不支持的按键：${event.code}（支持 A-Z、0-9、F1-F24、Space/Tab/Enter/Backspace/Delete 与左右修饰键）。`,
        );
        return;
      }

      event.preventDefault();
      event.stopPropagation();
      setHotkeyRecorderError(null);
      activeTokens.add(token);
      seenTokens.add(token);
      setHotkeyRecorderPreview(formatHotkeyTokens(activeTokens));

      if (!isHotkeyModifierToken(token)) {
        finishRecording(formatHotkeyTokens(activeTokens));
      }
    };

    const handleKeyUp = (event: KeyboardEvent) => {
      const token = keyboardEventToHotkeyToken(event);
      if (!token) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();

      activeTokens.delete(token);
      setHotkeyRecorderPreview(formatHotkeyTokens(activeTokens));

      if (activeTokens.size !== 0 || seenTokens.size === 0) {
        return;
      }

      if (ignoredUnsupportedKey) {
        setHotkeyRecorderError("包含不支持的按键，请重新录制。");
        setHotkeyRecorderStatus("error");
        return;
      }

      finishRecording(formatHotkeyTokens(seenTokens));
    };

    window.addEventListener("keydown", handleKeyDown, true);
    window.addEventListener("keyup", handleKeyUp, true);

    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
      window.removeEventListener("keyup", handleKeyUp, true);
    };
  }, [applyRecordedScreenshotHotkey, hotkeyRecorderStatus]);

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
          {activeSection === "hotkeys" ? "Hotkeys" : "General"}
        </Typography.Text>
      </div>

      <div className="flex min-h-0 flex-1 flex-col px-4 py-4">
        {!desktopRuntimeAvailable ? (
          <Alert
            className="mb-4"
            message="当前页面需要在 Tauri 桌面 runtime 内设置运行偏好。"
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

        {activeSection === "general" ? (
          <>
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

            {startupInlineError ? (
              <Alert
                className="mb-4"
                closable
                message="更新启动设置失败"
                onClose={() => setStartupInlineError(null)}
                showIcon
                type="error"
                description={startupInlineError}
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

            {desktopRuntimeAvailable && settingsPageLoading ? (
              <div className="flex min-h-0 flex-1 items-center justify-center">
                <Spin size="small" />
              </div>
            ) : (
              <div className="flex min-h-0 flex-1 flex-col gap-0">
                <div className="rounded-[0] border border-[#eef2f6] bg-white">
                  <div className="grid grid-cols-[180px_minmax(0,1fr)_auto] items-center gap-3 px-4 py-4">
                    <div className="min-w-0">
                      <Typography.Text className="block text-[12px] font-medium text-[#1f2937]">
                        随系统启动
                      </Typography.Text>
                    </div>

                    <div className="min-w-0">
                      <Typography.Text className="block text-[11px] text-[#667085]">
                        应用会在系统登录后自动启动。
                      </Typography.Text>
                    </div>

                    <Switch
                      checked={launchAtLoginEnabled}
                      disabled={
                        !desktopRuntimeAvailable ||
                        preferencesQuery.isFetching ||
                        preferencesQuery.isError
                      }
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
                  </div>
                </div>

                <div className="rounded-[0] border border-[#eef2f6] bg-white">
                  <div className="grid grid-cols-[180px_minmax(0,1fr)_auto] items-center gap-3 px-4 py-4">
                    <div className="min-w-0">
                      <Typography.Text className="block text-[12px] font-medium text-[#1f2937]">
                        静默启动
                      </Typography.Text>
                    </div>

                    <div className="min-w-0">
                      <Typography.Text className="block text-[11px] text-[#667085]">
                        仅在自启动场景生效：应用启动后保持托盘常驻，不主动弹出主窗口。
                      </Typography.Text>
                    </div>

                    <Switch
                      checked={startSilentlyEnabled}
                      disabled={
                        !desktopRuntimeAvailable ||
                        preferencesQuery.isFetching ||
                        preferencesQuery.isError
                      }
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
                  </div>
                </div>

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

                <div className="rounded-[0] border border-[#eef2f6] bg-white">
                  <div className="flex items-center justify-between gap-4 px-4 py-4">
                    <div className="min-w-0">
                      <Typography.Text className="block text-[12px] font-medium text-[#1f2937]">
                        编辑器管理
                      </Typography.Text>
                      <Typography.Text className="mt-1 block text-[11px] text-[#667085]">
                        管理 VS Code / JetBrains 路径。支持从系统变量 PATH 自动检测。当前已配置{" "}
                        {configuredEditorCount} 个编辑器。
                      </Typography.Text>
                    </div>
                    <Button
                      className="!h-[32px] !px-3 !text-[12px]"
                      disabled={!desktopRuntimeAvailable || preferencesQuery.isError}
                      icon={<SettingOutlined />}
                      onClick={openEditorManager}
                      size="small"
                    >
                      管理编辑器
                    </Button>
                  </div>
                </div>
              </div>
            )}
          </>
        ) : (
          <>
            {hotkeyInlineError || hotkeyRecorderError ? (
              <Alert
                className="mb-4"
                closable
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
                description={hotkeyRecorderError ?? hotkeyInlineError}
              />
            ) : null}

            {desktopRuntimeAvailable && settingsPageLoading ? (
              <div className="flex min-h-0 flex-1 items-center justify-center">
                <Spin size="small" />
              </div>
            ) : (
              <div className="flex min-h-0 flex-1 flex-col gap-0">
                <div className="rounded-[0] border border-[#eef2f6] bg-white">
                  <div className="grid grid-cols-[180px_minmax(0,1fr)_auto] items-center gap-3 px-4 py-4">
                    <div className="min-w-0">
                      <Typography.Text className="block text-[12px] font-medium text-[#1f2937]">
                        截图热键
                      </Typography.Text>
                    </div>

                    <div className="min-w-0">
                      <div
                        className="flex h-[32px] items-center overflow-hidden rounded-[8px] border border-[#d8e1eb] bg-[#f8fafc] px-3 font-mono text-[12px] text-[#475467]"
                        title={screenshotCaptureHotkey}
                      >
                        <span className="truncate">{screenshotCaptureHotkey}</span>
                      </div>
                      <Typography.Text className="mt-1 block text-[11px] text-[#667085]">
                        {hotkeyRecorderStatus === "recording"
                          ? hotkeyRecorderPreview
                            ? `录制中：${hotkeyRecorderPreview}（松开按键后保存）`
                            : "录制中：请按下新的组合键，按 Esc 取消。"
                          : `默认 ${DEFAULT_SCREENSHOT_CAPTURE_HOTKEY}，保存后立即生效。截图热键必须包含一个非修饰键，支持左右修饰键组合。`}
                      </Typography.Text>
                    </div>

                    <div className="flex flex-wrap justify-end gap-2">
                      <Button
                        className="!h-[32px] !px-3 !text-[12px]"
                        disabled={
                          hotkeyActionsDisabled ||
                          hotkeyRecorderStatus === "saving"
                        }
                        loading={hotkeyRecorderStatus === "saving"}
                        onClick={
                          hotkeyRecorderStatus === "recording"
                            ? cancelScreenshotHotkeyRecording
                            : beginScreenshotHotkeyRecording
                        }
                        size="small"
                        type={hotkeyRecorderStatus === "recording" ? "primary" : "default"}
                      >
                        {hotkeyRecorderStatus === "recording" ? "取消录制" : "录制热键"}
                      </Button>
                      <Button
                        className="!h-[32px] !px-3 !text-[12px]"
                        disabled={
                          hotkeyActionsDisabled ||
                          hotkeyRecorderStatus === "saving" ||
                          screenshotCaptureHotkey === DEFAULT_SCREENSHOT_CAPTURE_HOTKEY
                        }
                        onClick={() => void resetScreenshotHotkeyToDefault()}
                        size="small"
                      >
                        恢复默认
                      </Button>
                    </div>
                  </div>

                  {screenshotHotkeyUsesCtrlAlt ? (
                    <div className="border-t border-[#eef2f6] px-4 py-3">
                      <Alert
                        showIcon
                        type="warning"
                        message="当前截图热键使用了 Ctrl+Alt 组合"
                        description={`Windows 下这类组合更容易与输入法或系统热键环境冲突，建议改用默认 ${DEFAULT_SCREENSHOT_CAPTURE_HOTKEY} 或其他不含 Alt 的组合。`}
                      />
                    </div>
                  ) : null}
                </div>

                <div className="rounded-[0] border border-[#eef2f6] bg-white">
                  <div className="border-b border-[#eef2f6] px-4 py-3">
                    <Typography.Text className="block text-[12px] font-medium text-[#1f2937]">
                      截图工具热键（截图界面内）
                    </Typography.Text>
                    <Typography.Text className="mt-1 block text-[11px] text-[#667085]">
                      支持单键或组合键（例如 2、Ctrl+Shift+2）。以下热键只在截图 overlay 内生效。
                    </Typography.Text>
                  </div>

                  <div className="px-4 py-3">
                    <div className="grid grid-cols-[repeat(2,minmax(0,1fr))] gap-3">
                      {SCREENSHOT_TOOL_HOTKEY_ITEMS.map((item) => (
                        <div className="rounded-[8px] border border-[#e6edf5] bg-[#f8fafc] p-3" key={item.key}>
                          <Typography.Text className="block text-[11px] font-medium text-[#344054]">
                            {item.label}
                          </Typography.Text>
                          <Typography.Text className="mt-0.5 block text-[10px] text-[#667085]">
                            {item.description}
                          </Typography.Text>
                          <Input
                            className="mt-2"
                            disabled={hotkeyActionsDisabled}
                            onChange={(event) =>
                              setScreenshotToolHotkeyDraft(item.key, event.target.value)
                            }
                            placeholder={item.description}
                            value={screenshotToolHotkeyDrafts[item.key]}
                          />
                        </div>
                      ))}
                    </div>

                    <div className="mt-3 flex flex-wrap justify-end gap-2 border-t border-[#eef2f6] pt-3">
                      <Button
                        className="!h-[32px] !px-3 !text-[12px]"
                        disabled={
                          hotkeyActionsDisabled ||
                          updatePreferencesMutation.isPending ||
                          isSameScreenshotToolHotkeys(
                            screenshotToolHotkeyDrafts,
                            DEFAULT_SCREENSHOT_TOOL_HOTKEYS,
                          )
                        }
                        onClick={() => void resetScreenshotToolHotkeysToDefault()}
                        size="small"
                      >
                        恢复默认
                      </Button>
                      <Button
                        className="!h-[32px] !px-3 !text-[12px]"
                        disabled={hotkeyActionsDisabled}
                        loading={updatePreferencesMutation.isPending}
                        onClick={() => void saveScreenshotToolHotkeys()}
                        size="small"
                        type="primary"
                      >
                        保存工具热键
                      </Button>
                    </div>
                  </div>
                </div>

                <div className="rounded-[0] border border-[#eef2f6] bg-white px-4 py-4">
                  <Typography.Text className="block text-[12px] font-medium text-[#1f2937]">
                    语音输入热键（预留）
                  </Typography.Text>
                  <Typography.Text className="mt-1 block text-[11px] text-[#667085]">
                    当前阶段已在后端预留 `voiceInputToggle / voiceInputHold` 接口字段，后续合并
                    voiceType 语音输入能力时可直接接入。
                  </Typography.Text>
                </div>
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

          <div className="rounded-[12px] border border-[#eef2f6] bg-[#fbfcfe] p-4">
            <div className="flex items-center justify-between gap-3">
              <div className="min-w-0">
                <Typography.Text className="block text-[12px] font-medium text-[#1f2937]">
                  PATH 自动检测
                </Typography.Text>
                <Typography.Text className="mt-1 block text-[11px] text-[#667085]">
                  点击后会从系统变量 PATH 探测 VS Code / JetBrains 并自动写回当前设置。
                </Typography.Text>
                {editorDetectionResult ? (
                  <Typography.Text className="mt-1 block text-[11px] text-[#98a2b3]">
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

          <div className="rounded-[12px] border border-[#eef2f6] bg-[#fbfcfe] p-4">
            <div className="flex items-center justify-between gap-3">
              <div className="min-w-0">
                <Typography.Text className="block text-[12px] font-medium text-[#1f2937]">
                  自定义编辑器列表
                </Typography.Text>
                <Typography.Text className="mt-1 block text-[11px] text-[#667085]">
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
                    className="rounded-[10px] border border-[#e6edf5] bg-white p-3"
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
                <div className="rounded-[10px] border border-dashed border-[#d8e1eb] bg-white px-3 py-5 text-center">
                  <Typography.Text className="text-[12px] text-[#98a2b3]">
                    暂无自定义编辑器，点击右上角“新增编辑器”开始添加。
                  </Typography.Text>
                </div>
              )}
            </div>

            <div className="mt-3 flex justify-end border-t border-[#e6edf5] pt-3">
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

          <div className="flex justify-end border-t border-[#e6edf5] pt-3">
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
      ? "border-[#b7ebc0] bg-[#effcf2] text-[#1f7a34]"
      : "border-[#f2d9d5] bg-[#fff6f5] text-[#b5473b]"
    : "border-[#d8e1eb] bg-[#f8fafc] text-[#667085]";
  const detectedPath = detection?.executablePath?.trim() ?? "";

  return (
    <div className="rounded-[12px] border border-[#eef2f6] bg-[#fbfcfe] p-4">
      <div className="flex items-center justify-between gap-3">
        <Typography.Text className="text-[12px] font-medium text-[#1f2937]">
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
          <Typography.Text className="mb-1 block text-[11px] text-[#667085]">
            当前配置路径
          </Typography.Text>
          <div
            className="flex h-[32px] items-center overflow-hidden rounded-[8px] border border-[#d8e1eb] bg-white px-3 font-mono text-[12px] text-[#475467]"
            title={configuredPath || "未配置"}
          >
            <span className="truncate">{configuredPath || "未配置"}</span>
          </div>
        </div>

        <div>
          <Typography.Text className="mb-1 block text-[11px] text-[#667085]">
            最近检测路径
          </Typography.Text>
          <div
            className="flex h-[32px] items-center overflow-hidden rounded-[8px] border border-[#d8e1eb] bg-white px-3 font-mono text-[12px] text-[#475467]"
            title={detectedPath || "未检测到可执行路径"}
          >
            <span className="truncate">{detectedPath || "未检测到可执行路径"}</span>
          </div>
        </div>
      </div>

      <div className="mt-3 space-y-2 border-t border-[#e6edf5] pt-3">
        <Typography.Text className="block text-[11px] text-[#667085]">
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
        <Typography.Text className="mt-2 block text-[11px] text-[#667085]">
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

function resolveScreenshotToolHotkeys(
  input?: Partial<AppPreferences["hotkey"]["screenshotTools"]> | null,
): AppPreferences["hotkey"]["screenshotTools"] {
  return normalizeScreenshotToolHotkeys({
    select: input?.select ?? DEFAULT_SCREENSHOT_TOOL_HOTKEYS.select,
    line: input?.line ?? DEFAULT_SCREENSHOT_TOOL_HOTKEYS.line,
    rect: input?.rect ?? DEFAULT_SCREENSHOT_TOOL_HOTKEYS.rect,
    ellipse: input?.ellipse ?? DEFAULT_SCREENSHOT_TOOL_HOTKEYS.ellipse,
    arrow: input?.arrow ?? DEFAULT_SCREENSHOT_TOOL_HOTKEYS.arrow,
  });
}

function normalizeScreenshotToolHotkeys(
  input: AppPreferences["hotkey"]["screenshotTools"],
): AppPreferences["hotkey"]["screenshotTools"] {
  return {
    select: normalizeScreenshotToolHotkeyValue(input.select, DEFAULT_SCREENSHOT_TOOL_HOTKEYS.select),
    line: normalizeScreenshotToolHotkeyValue(input.line, DEFAULT_SCREENSHOT_TOOL_HOTKEYS.line),
    rect: normalizeScreenshotToolHotkeyValue(input.rect, DEFAULT_SCREENSHOT_TOOL_HOTKEYS.rect),
    ellipse: normalizeScreenshotToolHotkeyValue(
      input.ellipse,
      DEFAULT_SCREENSHOT_TOOL_HOTKEYS.ellipse,
    ),
    arrow: normalizeScreenshotToolHotkeyValue(input.arrow, DEFAULT_SCREENSHOT_TOOL_HOTKEYS.arrow),
  };
}

function normalizeScreenshotToolHotkeyValue(value: string, fallback: string) {
  const trimmed = value.trim();
  return trimmed || fallback;
}

function isSameScreenshotToolHotkeys(
  left: AppPreferences["hotkey"]["screenshotTools"],
  right: AppPreferences["hotkey"]["screenshotTools"],
) {
  const normalizedLeft = normalizeScreenshotToolHotkeys(left);
  const normalizedRight = normalizeScreenshotToolHotkeys(right);
  return (
    normalizedLeft.select === normalizedRight.select &&
    normalizedLeft.line === normalizedRight.line &&
    normalizedLeft.rect === normalizedRight.rect &&
    normalizedLeft.ellipse === normalizedRight.ellipse &&
    normalizedLeft.arrow === normalizedRight.arrow
  );
}

function isSameHotkeyShortcut(left: string, right: string) {
  return left.trim().toLowerCase() === right.trim().toLowerCase();
}

function findDuplicateScreenshotToolHotkey(
  input: AppPreferences["hotkey"]["screenshotTools"],
): { label: string; conflictLabel: string } | null {
  const seen = new Map<string, string>();
  for (const item of SCREENSHOT_TOOL_HOTKEY_ITEMS) {
    const shortcut = input[item.key].trim().toLowerCase();
    const previous = seen.get(shortcut);
    if (previous) {
      return {
        label: previous,
        conflictLabel: item.label,
      };
    }
    seen.set(shortcut, item.label);
  }

  return null;
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

function keyboardEventToHotkeyToken(event: KeyboardEvent): string | null {
  switch (event.code) {
    case "ControlLeft":
      return "LCtrl";
    case "ControlRight":
      return "RCtrl";
    case "AltLeft":
      return "LAlt";
    case "AltRight":
      return "RAlt";
    case "ShiftLeft":
      return "LShift";
    case "ShiftRight":
      return "RShift";
    case "MetaLeft":
      return "LWin";
    case "MetaRight":
      return "RWin";
    case "Space":
      return "Space";
    case "Tab":
      return "Tab";
    case "Enter":
      return "Enter";
    case "Backspace":
      return "Backspace";
    case "Delete":
      return "Delete";
    default:
      break;
  }

  if (event.code.startsWith("Key") && event.code.length === 4) {
    return event.code.slice(3);
  }

  if (event.code.startsWith("Digit") && event.code.length === 6) {
    return event.code.slice(5);
  }

  const functionKeyMatch = event.code.match(/^F([1-9]|1[0-9]|2[0-4])$/);
  if (functionKeyMatch) {
    return functionKeyMatch[0];
  }

  return null;
}

function hotkeyTokenOrder(token: string) {
  const fixedOrder: Record<string, number> = {
    Ctrl: 10,
    LCtrl: 11,
    RCtrl: 12,
    Alt: 20,
    LAlt: 21,
    RAlt: 22,
    Shift: 30,
    LShift: 31,
    RShift: 32,
    Super: 40,
    LWin: 41,
    RWin: 42,
    Space: 100,
    Tab: 101,
    Enter: 102,
    Backspace: 103,
    Delete: 104,
  };

  const fixed = fixedOrder[token];
  if (typeof fixed === "number") {
    return fixed;
  }

  if (/^[A-Z]$/.test(token)) {
    return 100 + token.charCodeAt(0);
  }

  if (/^[0-9]$/.test(token)) {
    return 200 + token.charCodeAt(0);
  }

  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(token)) {
    const sequence = Number.parseInt(token.slice(1), 10);
    return 300 + sequence;
  }

  return 999;
}

function formatHotkeyTokens(tokens: Iterable<string>) {
  return Array.from(new Set(tokens))
    .sort((left, right) => hotkeyTokenOrder(left) - hotkeyTokenOrder(right))
    .join("+");
}

function isHotkeyModifierToken(token: string) {
  return HOTKEY_MODIFIER_TOKENS.has(token);
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
