import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  AppPreferences,
  AppError,
  CancelScreenshotSessionResult,
  CancelRestoreActionResult,
  CancelRestoreRunResult,
  CodexHomeDirectoryInfo,
  CopyScreenshotSelectionResult,
  CreateSnapshotPayload,
  CodexProfileRecord,
  CommandResponse,
  DeleteResult,
  EditorPathDetectionResult,
  HotkeyTriggerEvent,
  LaunchTaskRecord,
  OpenLogDirectoryResult,
  OpenWorkspaceInEditorResult,
  OpenWorkspaceTerminalResult,
  SaveScreenshotSelectionResult,
  RunWorkspaceTerminalCommandResult,
  RunWorkspaceTerminalCommandsResult,
  RecentRestoreTarget,
  RestoreCapabilities,
  RestoreRunEvent,
  RestorePreview,
  RestorePreviewPayload,
  RestoreRunDetail,
  RestoreRunSummary,
  ScreenshotRenderedImageInput,
  ScreenshotSelectionInput,
  ScreenshotSessionUpdatedEvent,
  ScreenshotSessionView,
  SnapshotRecord,
  StartScreenshotSessionResult,
  StartRestoreDryRunPayload,
  StartRestoreRunPayload,
  UpdateSnapshotPayload,
  UpsertCodexProfilePayload,
  UpsertLaunchTaskPayload,
  UpsertProjectPayload,
  UpsertWorkspacePayload,
  WorkspaceRecord,
  WorkspaceResourceEntry,
  WorkspaceResourceGitStatusResponse,
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

export function getAppPreferences() {
  return invokeCommand<AppPreferences>("get_app_preferences");
}

export function updateAppPreferences(input: AppPreferences) {
  return invokeCommand<AppPreferences>("update_app_preferences", { input });
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

export function deleteWorkspace(id: string) {
  return invokeCommand<DeleteResult>("delete_workspace", { id });
}

export function registerWorkspaceFolder(path: string) {
  return invokeCommand<WorkspaceRecord>("register_workspace_folder", { path });
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

export function deleteLaunchTask(id: string) {
  return invokeCommand<DeleteResult>("delete_launch_task", { id });
}

export function listCodexProfiles() {
  return invokeCommand<CodexProfileRecord[]>("list_codex_profiles");
}

export function upsertCodexProfile(input: UpsertCodexProfilePayload) {
  return invokeCommand<CodexProfileRecord>("upsert_codex_profile", { input });
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

export function startScreenshotSession() {
  return invokeCommand<StartScreenshotSessionResult>("start_screenshot_session");
}

export function getScreenshotSession() {
  return invokeCommand<ScreenshotSessionView>("get_screenshot_session");
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
