import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  AppPreferences,
  AppPreferencesPatch,
  AppError,
  CancelScreenshotSessionResult,
  CancelRestoreActionResult,
  CancelRestoreRunResult,
  CodexHomeDirectoryInfo,
  CodexHistoryGlobalSessionsPage,
  CodexHistoryMessagesPage,
  CodexHistoryMessagesPayload,
  CodexHistorySessionsResponse,
  ListCodexHistorySessionsPagePayload,
  CodexAuthProfileDetail,
  CodexAuthProfileSummary,
  CodexAuthQuotaRefreshBatchResult,
  CodexAuthSwitchResult,
  CopyScreenshotSelectionResult,
  CreateSnapshotPayload,
  CodexProfileRecord,
  CommandResponse,
  DeleteResult,
  EditorPathDetectionResult,
  HotkeyTriggerEvent,
  HotkeyHealth,
  LaunchTaskRecord,
  NativeInteractionExclusionRect,
  NativeInteractionEditableShape,
  NativeInteractionMode,
  NativeInteractionShapeAnnotationCommittedEvent,
  NativeInteractionShapeAnnotationUpdatedEvent,
  NativeInteractionSelectionRect,
  NativeInteractionStateView,
  NativeInteractionStateUpdatedEvent,
  OpenLogDirectoryResult,
  OpenCodexHistoryWindowResult,
  OpenWorkspaceInEditorResult,
  OpenWorkspaceTerminalResult,
  PromptQuickPasteResultEvent,
  ApplyPromptListImportPayload,
  ExportPromptListPayload,
  ExportPromptListResult,
  PreviewPromptListImportPayload,
  PromptImportApplyResult,
  PromptImportPreview,
  PromptRecord,
  SaveScreenshotSelectionResult,
  RunWorkspaceTerminalCommandResult,
  RunWorkspaceTerminalCommandsResult,
  RecentRestoreTarget,
  ReorderLaunchTasksPayload,
  ReorderPromptsPayload,
  ReorderWorkspacesPayload,
  RestoreCapabilities,
  RestoreRunEvent,
  RestorePreview,
  RestorePreviewPayload,
  RestoreRunDetail,
  RestoreRunSummary,
  ScreenshotRenderedImageInput,
  ScreenshotEscapePressedEvent,
  ScreenshotSelectionInput,
  ScreenshotSelectionRenderView,
  ScreenshotSessionUpdatedEvent,
  ScreenshotSessionView,
  SnapshotRecord,
  StartScreenshotSessionResult,
  StartRestoreDryRunPayload,
  StartRestoreRunPayload,
  UpdateSnapshotPayload,
  UpsertCodexProfilePayload,
  UpsertCodexAuthProfilePayload,
  UpsertLaunchTaskPayload,
  UpsertPromptPayload,
  UpsertProjectPayload,
  UpsertWorkspacePayload,
  UpdateWorkspaceDescriptionPayload,
  WorkspaceRecord,
  ApplyWorkspaceListImportPayload,
  ExportWorkspaceListPayload,
  ExportWorkspaceListResult,
  PreviewWorkspaceListImportPayload,
  WorkspaceImportApplyResult,
  WorkspaceImportPreview,
  WorkspaceResourceEntry,
  WorkspaceResourceGitStatusResponse,
  CopyOssObjectPayload,
  CopyOssObjectResult,
  CreateOssFolderPayload,
  CreateOssFolderResult,
  DeleteOssObjectsPayload,
  DeleteOssObjectsResult,
  GetOssObjectDownloadUrlPayload,
  GetOssObjectDownloadUrlResult,
  HeadOssObjectPayload,
  ListOssObjectsPayload,
  OssAccountSummary,
  OssObjectMetadata,
  OssObjectPage,
  OssOperationPayload,
  OssProbeResult,
  OssTargetRecord,
  OssTransferTaskView,
  OssTransferStart,
  StartOssDownloadPayload,
  StartOssUploadPayload,
  TestOssTargetPayload,
  UpsertOssAccountPayload,
  UpsertOssTargetPayload,
} from "@/types/backend";

export class CommandClientError extends Error {
  public readonly code: string;
  public readonly details?: Record<string, string>;
  public readonly retryable?: boolean;

  public constructor(error: AppError) {
    super(error.message);
    this.name = "CommandClientError";
    this.code = error.code;
    this.details = error.details;
    this.retryable = error.retryable;
  }
}

function desktopRuntimeRequired(): never {
  throw new CommandClientError({
    code: "DESKTOP_RUNTIME_REQUIRED",
    message: "当前页面需要在 Tauri 桌面 runtime 内运行。",
  });
}

function normalizeUnknownError(error: unknown): CommandClientError {
  if (error instanceof CommandClientError) {
    return error;
  }
  if (error instanceof Error) {
    return new CommandClientError({
      code: "COMMAND_FAILED",
      message: error.message,
    });
  }
  return new CommandClientError({
    code: "COMMAND_FAILED",
    message: "命令执行失败",
  });
}

async function invokeCommand<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    desktopRuntimeRequired();
  }

  try {
    const response = await invoke<CommandResponse<T>>(command, args);
    if (response.ok) {
      return response.data;
    }
    throw new CommandClientError(response.error);
  } catch (error) {
    throw normalizeUnknownError(error);
  }
}

export function hasDesktopRuntime() {
  return isTauri();
}

export function getErrorSummary(error: unknown) {
  const resolved = normalizeUnknownError(error);
  return {
    code: resolved.code,
    message: resolved.message,
    details: resolved.details,
  };
}

export function listWorkspaces() {
  return invokeCommand<WorkspaceRecord[]>("list_workspaces");
}

export function exportWorkspaceList(input: ExportWorkspaceListPayload) {
  return invokeCommand<ExportWorkspaceListResult>("export_workspace_list", { input });
}

export function previewWorkspaceListImport(input: PreviewWorkspaceListImportPayload) {
  return invokeCommand<WorkspaceImportPreview>("preview_workspace_list_import", { input });
}

export function applyWorkspaceListImport(input: ApplyWorkspaceListImportPayload) {
  return invokeCommand<WorkspaceImportApplyResult>("apply_workspace_list_import", { input });
}

export function getAppPreferences() {
  return invokeCommand<AppPreferences>("get_app_preferences");
}

export function updateAppPreferences(input: AppPreferencesPatch) {
  return invokeCommand<AppPreferences>("update_app_preferences", { input });
}

export function getHotkeyHealth() {
  return invokeCommand<HotkeyHealth>("get_hotkey_health");
}

export function retryHotkeyRegistration() {
  return invokeCommand<HotkeyHealth>("retry_hotkey_registration");
}

export function getCodexHomeDirectory() {
  return invokeCommand<CodexHomeDirectoryInfo>("get_codex_home_directory");
}

export function detectEditorsFromPath() {
  return invokeCommand<EditorPathDetectionResult>("detect_editors_from_path");
}

export function upsertWorkspace(input: UpsertWorkspacePayload) {
  return invokeCommand<WorkspaceRecord>("upsert_workspace", { input });
}

export function reorderWorkspaces(input: ReorderWorkspacesPayload) {
  return invokeCommand<WorkspaceRecord[]>("reorder_workspaces", { input });
}

export function deleteWorkspace(id: string) {
  return invokeCommand<DeleteResult>("delete_workspace", { id });
}

export function registerWorkspaceFolder(path: string, description?: string | null) {
  return invokeCommand<WorkspaceRecord>("register_workspace_folder", {
    path,
    description: description ?? null,
  });
}

export function updateWorkspaceDescription(input: UpdateWorkspaceDescriptionPayload) {
  return invokeCommand<WorkspaceRecord>("update_workspace_description", { input });
}

export function removeWorkspaceRegistration(id: string) {
  return invokeCommand<DeleteResult>("remove_workspace_registration", { id });
}

export function openWorkspaceTerminal(workspaceId: string) {
  return invokeCommand<OpenWorkspaceTerminalResult>("open_workspace_terminal", { workspaceId });
}

export function openWorkspaceTerminalAtPath(workspaceId: string, targetPath?: string | null) {
  return invokeCommand<OpenWorkspaceTerminalResult>("open_workspace_terminal_at_path", {
    workspaceId,
    targetPath: targetPath?.trim() ? targetPath.trim() : null,
  });
}

export function openWorkspaceInEditor(workspaceId: string, editorKey: string) {
  return invokeCommand<OpenWorkspaceInEditorResult>("open_workspace_in_editor", {
    workspaceId,
    editorKey,
  });
}

export function runWorkspaceTerminalCommand(workspaceId: string, launchTaskId: string) {
  return invokeCommand<RunWorkspaceTerminalCommandResult>("run_workspace_terminal_command", {
    workspaceId,
    launchTaskId,
  });
}

export function runWorkspaceTerminalCommands(workspaceId: string) {
  return invokeCommand<RunWorkspaceTerminalCommandsResult>("run_workspace_terminal_commands", {
    workspaceId,
  });
}

export function listWorkspaceResourceChildren(workspaceId: string, targetPath?: string | null) {
  return invokeCommand<WorkspaceResourceEntry[]>("list_workspace_resource_children", {
    workspaceId,
    targetPath: targetPath?.trim() ? targetPath.trim() : null,
  });
}

export function allowWorkspaceResourceScope(workspaceId: string) {
  return invokeCommand<string>("allow_workspace_resource_scope", { workspaceId });
}

export function getWorkspaceResourceGitStatuses(workspaceId: string) {
  return invokeCommand<WorkspaceResourceGitStatusResponse>("get_workspace_resource_git_statuses", {
    workspaceId,
  });
}

export function upsertProject(input: UpsertProjectPayload) {
  return invokeCommand<WorkspaceRecord["projects"][number]>("upsert_project", { input });
}

export function listLaunchTasks(projectId: string) {
  return invokeCommand<LaunchTaskRecord[]>("list_launch_tasks", { projectId });
}

export function upsertLaunchTask(input: UpsertLaunchTaskPayload) {
  return invokeCommand<LaunchTaskRecord>("upsert_launch_task", { input });
}

export function reorderLaunchTasks(input: ReorderLaunchTasksPayload) {
  return invokeCommand<LaunchTaskRecord[]>("reorder_launch_tasks", { input });
}

export function deleteLaunchTask(id: string) {
  return invokeCommand<DeleteResult>("delete_launch_task", { id });
}

export function listCodexProfiles() {
  return invokeCommand<CodexProfileRecord[]>("list_codex_profiles");
}

export function upsertCodexProfile(input: UpsertCodexProfilePayload) {
  return invokeCommand<CodexProfileRecord>("upsert_codex_profile", { input });
}

export function listCodexAuthProfiles() {
  return invokeCommand<CodexAuthProfileSummary[]>("list_codex_auth_profiles");
}

export function getCodexAuthProfileDetail(id: string) {
  return invokeCommand<CodexAuthProfileDetail>("get_codex_auth_profile_detail", { id });
}

export function importCurrentCodexAuthProfile() {
  return invokeCommand<CodexAuthProfileSummary>("import_current_codex_auth_profile");
}

export function upsertCodexAuthProfile(input: UpsertCodexAuthProfilePayload) {
  return invokeCommand<CodexAuthProfileSummary>("upsert_codex_auth_profile", { input });
}

export function deleteCodexAuthProfile(id: string) {
  return invokeCommand<DeleteResult>("delete_codex_auth_profile", { id });
}

export function switchCodexAuthProfile(id: string) {
  return invokeCommand<CodexAuthSwitchResult>("switch_codex_auth_profile", { id });
}

export function queryCodexAuthQuota(id: string) {
  return invokeCommand<CodexAuthProfileSummary>("query_codex_auth_quota", { id });
}

export function refreshAllCodexAuthQuotas() {
  return invokeCommand<CodexAuthQuotaRefreshBatchResult>("refresh_all_codex_auth_quotas");
}

export function listPrompts() {
  return invokeCommand<PromptRecord[]>("list_prompts");
}

export function upsertPrompt(input: UpsertPromptPayload) {
  return invokeCommand<PromptRecord>("upsert_prompt", { input });
}

export function deletePrompt(id: string) {
  return invokeCommand<DeleteResult>("delete_prompt", { id });
}

export function reorderPrompts(input: ReorderPromptsPayload) {
  return invokeCommand<PromptRecord[]>("reorder_prompts", { input });
}

export function exportPromptList(input: ExportPromptListPayload) {
  return invokeCommand<ExportPromptListResult>("export_prompt_list", { input });
}

export function previewPromptListImport(input: PreviewPromptListImportPayload) {
  return invokeCommand<PromptImportPreview>("preview_prompt_list_import", { input });
}

export function applyPromptListImport(input: ApplyPromptListImportPayload) {
  return invokeCommand<PromptImportApplyResult>("apply_prompt_list_import", { input });
}

export function openCodexHistoryWindow(workspaceId: string) {
  return invokeCommand<OpenCodexHistoryWindowResult>("open_codex_history_window", { workspaceId });
}

export function listCodexHistorySessions(workspaceId: string) {
  return invokeCommand<CodexHistorySessionsResponse>("list_codex_history_sessions", {
    input: { workspaceId },
  });
}

export function listAllCodexHistorySessions(input: ListCodexHistorySessionsPagePayload) {
  return invokeCommand<CodexHistoryGlobalSessionsPage>("list_all_codex_history_sessions", {
    input,
  });
}

export function getCodexHistoryMessages(input: CodexHistoryMessagesPayload) {
  return invokeCommand<CodexHistoryMessagesPage>("get_codex_history_messages", { input });
}

export function listSnapshots(workspaceId?: string) {
  return invokeCommand<SnapshotRecord[]>("list_snapshots", { workspaceId });
}

export function createSnapshot(input: CreateSnapshotPayload) {
  return invokeCommand<SnapshotRecord>("create_snapshot", { input });
}

export function updateSnapshot(input: UpdateSnapshotPayload) {
  return invokeCommand<SnapshotRecord>("update_snapshot", { input });
}

export function listOssAccounts() {
  return invokeCommand<OssAccountSummary[]>("list_oss_accounts");
}

export function upsertOssAccount(input: UpsertOssAccountPayload) {
  return invokeCommand<OssAccountSummary>("upsert_oss_account", { input });
}

export function deleteOssAccount(id: string) {
  return invokeCommand<DeleteResult>("delete_oss_account", { id });
}

export function listOssTargets(accountId: string) {
  return invokeCommand<OssTargetRecord[]>("list_oss_targets", { accountId });
}

export function upsertOssTarget(input: UpsertOssTargetPayload) {
  return invokeCommand<OssTargetRecord>("upsert_oss_target", { input });
}

export function deleteOssTarget(id: string) {
  return invokeCommand<DeleteResult>("delete_oss_target", { id });
}

export function listOssObjects(input: ListOssObjectsPayload) {
  return invokeCommand<OssObjectPage>("list_oss_objects", { input });
}

export function createOssFolder(input: CreateOssFolderPayload) {
  return invokeCommand<CreateOssFolderResult>("create_oss_folder", { input });
}

export function headOssObject(input: HeadOssObjectPayload) {
  return invokeCommand<OssObjectMetadata>("head_oss_object", { input });
}

export function getOssObjectDownloadUrl(input: GetOssObjectDownloadUrlPayload) {
  return invokeCommand<GetOssObjectDownloadUrlResult>("get_oss_object_download_url", { input });
}

export function testOssTarget(input: TestOssTargetPayload) {
  return invokeCommand<OssProbeResult>("test_oss_target", { input });
}

export function listOssTransferTasks() {
  return invokeCommand<OssTransferTaskView[]>("list_oss_transfer_tasks");
}

export function startOssUpload(input: StartOssUploadPayload) {
  return invokeCommand<OssTransferStart>("start_oss_upload", { input });
}

export function startOssDownload(input: StartOssDownloadPayload) {
  return invokeCommand<OssTransferStart>("start_oss_download", { input });
}

export function resumeOssTransfer(input: OssOperationPayload) {
  return invokeCommand<OssTransferStart>("resume_oss_transfer", { input });
}

export function cancelOssTransfer(input: OssOperationPayload) {
  return invokeCommand<OssTransferTaskView>("cancel_oss_transfer", { input });
}

export function confirmOssTransfer(input: OssOperationPayload) {
  return invokeCommand<OssTransferTaskView>("confirm_oss_transfer", { input });
}

export function deleteOssObjects(input: DeleteOssObjectsPayload) {
  return invokeCommand<DeleteOssObjectsResult>("delete_oss_objects", { input });
}

export function copyOssObject(input: CopyOssObjectPayload) {
  return invokeCommand<CopyOssObjectResult>("copy_oss_object", { input });
}

export function startScreenshotSession() {
  return invokeCommand<StartScreenshotSessionResult>("start_screenshot_session");
}

export function getScreenshotSession() {
  return invokeCommand<ScreenshotSessionView>("get_screenshot_session");
}

export function getNativeInteractionState() {
  return invokeCommand<NativeInteractionStateView>("get_native_interaction_state");
}

export function updateNativeInteractionRuntime(
  sessionId: string,
  visible: boolean,
  exclusionRects: NativeInteractionExclusionRect[],
  mode: NativeInteractionMode,
  selection?: NativeInteractionSelectionRect | null,
  activeShape?: NativeInteractionEditableShape | null,
  shapeCandidates?: NativeInteractionEditableShape[],
  annotationColor?: string | null,
  annotationStrokeWidth?: number | null,
) {
  return invokeCommand<NativeInteractionStateView>("update_native_interaction_runtime", {
    input: {
      sessionId,
      visible,
      exclusionRects,
      mode,
      selection: selection ?? null,
      activeShape: activeShape ?? null,
      shapeCandidates: shapeCandidates ?? [],
      annotationColor: annotationColor?.trim() ? annotationColor.trim() : null,
      annotationStrokeWidth:
        typeof annotationStrokeWidth === "number" && Number.isFinite(annotationStrokeWidth)
          ? annotationStrokeWidth
          : null,
    },
  });
}

export function updateNativeInteractionExclusionRects(
  sessionId: string,
  exclusionRects: NativeInteractionExclusionRect[],
) {
  return invokeCommand<boolean>("update_native_interaction_exclusion_rects", {
    input: {
      sessionId,
      exclusionRects,
    },
  });
}

export async function getScreenshotPreviewRgba(sessionId: string) {
  if (!isTauri()) {
    desktopRuntimeRequired();
  }

  try {
    return await invoke<Uint8Array | ArrayBuffer | number[]>("get_screenshot_preview_rgba", {
      sessionId,
    });
  } catch (error) {
    throw normalizeUnknownError(error);
  }
}

export function getScreenshotSelectionRender(
  sessionId: string,
  selection: ScreenshotSelectionInput,
) {
  return invokeCommand<ScreenshotSelectionRenderView>("get_screenshot_selection_render", {
    sessionId,
    selection,
  });
}

export function copyScreenshotSelection(
  sessionId: string,
  selection: ScreenshotSelectionInput,
  renderedImage?: ScreenshotRenderedImageInput | null,
) {
  return invokeCommand<CopyScreenshotSelectionResult>("copy_screenshot_selection", {
    sessionId,
    selection,
    renderedImage: renderedImage ?? null,
  });
}

export function saveScreenshotSelection(
  sessionId: string,
  selection: ScreenshotSelectionInput,
  filePath?: string | null,
  renderedImage?: ScreenshotRenderedImageInput | null,
) {
  return invokeCommand<SaveScreenshotSelectionResult>("save_screenshot_selection", {
    sessionId,
    selection,
    filePath: filePath?.trim() ? filePath.trim() : null,
    renderedImage: renderedImage ?? null,
  });
}

export function cancelScreenshotSession(sessionId: string) {
  return invokeCommand<CancelScreenshotSessionResult>("cancel_screenshot_session", { sessionId });
}

export function previewRestore(input: RestorePreviewPayload) {
  return invokeCommand<RestorePreview>("preview_restore", { input });
}

export function startRestoreDryRun(input: StartRestoreDryRunPayload) {
  return invokeCommand<RestoreRunDetail>("start_restore_dry_run", { input });
}

export function getRestoreCapabilities() {
  return invokeCommand<RestoreCapabilities>("get_restore_capabilities");
}

export function listRecentRestoreTargets() {
  return invokeCommand<RecentRestoreTarget[]>("list_recent_restore_targets");
}

export function restoreRecentTarget(id: string, mode?: string) {
  return invokeCommand<RestoreRunDetail>("restore_recent_target", { id, mode });
}

export function startRestoreRun(input: StartRestoreRunPayload) {
  return invokeCommand<RestoreRunDetail>("start_restore_run", { input });
}

export function cancelRestoreRun(runId: string) {
  return invokeCommand<CancelRestoreRunResult>("cancel_restore_run", { runId });
}

export function cancelRestoreAction(runId: string, projectTaskId: string, actionId: string) {
  return invokeCommand<CancelRestoreActionResult>("cancel_restore_action", {
    runId,
    projectTaskId,
    actionId,
  });
}

export function listRestoreRuns() {
  return invokeCommand<RestoreRunSummary[]>("list_restore_runs");
}

export function getRestoreRunDetail(id: string) {
  return invokeCommand<RestoreRunDetail>("get_restore_run_detail", { id });
}

export function openLogDirectory() {
  return invokeCommand<OpenLogDirectoryResult>("open_log_directory");
}

export async function listenToRestoreRunEvents(
  handler: (event: RestoreRunEvent) => void,
): Promise<UnlistenFn> {
  if (!isTauri()) {
    desktopRuntimeRequired();
  }

  return listen<RestoreRunEvent>("restore://run-event", (event) => {
    handler(event.payload);
  });
}

export async function listenToHotkeyTriggerEvents(
  handler: (event: HotkeyTriggerEvent) => void,
): Promise<UnlistenFn> {
  if (!isTauri()) {
    desktopRuntimeRequired();
  }

  return listen<HotkeyTriggerEvent>("hotkey://trigger", (event) => {
    handler(event.payload);
  });
}

export async function listenToScreenshotSessionUpdatedEvents(
  handler: (event: ScreenshotSessionUpdatedEvent) => void,
): Promise<UnlistenFn> {
  if (!isTauri()) {
    desktopRuntimeRequired();
  }

  return listen<ScreenshotSessionUpdatedEvent>("screenshot://session-updated", (event) => {
    handler(event.payload);
  });
}

export async function listenToPromptQuickPasteResultEvents(
  handler: (event: PromptQuickPasteResultEvent) => void,
): Promise<UnlistenFn> {
  if (!isTauri()) {
    desktopRuntimeRequired();
  }

  return listen<PromptQuickPasteResultEvent>(
    "hotkey://prompt-quick-paste-result",
    (event) => {
      handler(event.payload);
    },
  );
}

export async function listenToScreenshotEscapePressedEvents(
  handler: (event: ScreenshotEscapePressedEvent) => void,
): Promise<UnlistenFn> {
  if (!isTauri()) {
    desktopRuntimeRequired();
  }

  return listen<ScreenshotEscapePressedEvent>("screenshot://escape-pressed", (event) => {
    handler(event.payload);
  });
}

export async function listenToNativeInteractionStateUpdatedEvents(
  handler: (event: NativeInteractionStateUpdatedEvent) => void,
): Promise<UnlistenFn> {
  if (!isTauri()) {
    desktopRuntimeRequired();
  }

  return listen<NativeInteractionStateUpdatedEvent>("native_interaction://state-updated", (event) => {
    handler(event.payload);
  });
}

export async function listenToNativeInteractionShapeAnnotationCommittedEvents(
  handler: (event: NativeInteractionShapeAnnotationCommittedEvent) => void,
): Promise<UnlistenFn> {
  if (!isTauri()) {
    desktopRuntimeRequired();
  }

  return listen<NativeInteractionShapeAnnotationCommittedEvent>(
    "native_interaction://shape-annotation-committed",
    (event) => {
      handler(event.payload);
    },
  );
}

export async function listenToNativeInteractionShapeAnnotationUpdatedEvents(
  handler: (event: NativeInteractionShapeAnnotationUpdatedEvent) => void,
): Promise<UnlistenFn> {
  if (!isTauri()) {
    desktopRuntimeRequired();
  }

  return listen<NativeInteractionShapeAnnotationUpdatedEvent>(
    "native_interaction://shape-annotation-updated",
    (event) => {
      handler(event.payload);
    },
  );
}

export async function listenToOssTransferProgressEvents(
  handler: (task: OssTransferTaskView) => void,
): Promise<UnlistenFn> {
  if (!isTauri()) {
    desktopRuntimeRequired();
  }

  return listen<OssTransferTaskView>("oss://transfer-progress", (event) => {
    handler(event.payload);
  });
}

export async function listenToOssTransferStateEvents(
  handler: (task: OssTransferTaskView) => void,
): Promise<UnlistenFn> {
  if (!isTauri()) {
    desktopRuntimeRequired();
  }

  return listen<OssTransferTaskView>("oss://transfer-state-changed", (event) => {
    handler(event.payload);
  });
}
