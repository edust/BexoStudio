export type AppError = {
  code: string;
  message: string;
  details?: Record<string, string>;
  retryable?: boolean;
};

export type CommandResponse<T> =
  | {
      ok: true;
      data: T;
      runId?: string;
    }
  | {
      ok: false;
      error: AppError;
    };

export type ProjectRecord = {
  id: string;
  workspaceId: string;
  name: string;
  path: string;
  platform: string;
  terminalType: string;
  ideType?: string | null;
  codexProfileId?: string | null;
  openTerminal: boolean;
  openIde: boolean;
  autoResumeCodex: boolean;
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
  launchTasks: LaunchTaskRecord[];
};

export type LaunchTaskRetryPolicy = {
  maxAttempts: number;
  backoffMs: number;
};

export type LaunchTaskRecord = {
  id: string;
  projectId: string;
  name: string;
  taskType: string;
  enabled: boolean;
  command: string;
  args: string[];
  workingDir: string;
  timeoutMs: number;
  continueOnFailure: boolean;
  retryPolicy: LaunchTaskRetryPolicy;
  sortOrder: number;
};

export type WorkspaceRecord = {
  id: string;
  name: string;
  description?: string | null;
  icon?: string | null;
  color?: string | null;
  sortOrder: number;
  isDefault: boolean;
  isArchived: boolean;
  createdAt: string;
  updatedAt: string;
  projects: ProjectRecord[];
};

export type CodexProfileRecord = {
  id: string;
  name: string;
  description?: string | null;
  codexHome: string;
  startupMode: string;
  resumeStrategy: string;
  defaultArgs: string[];
  isDefault: boolean;
  createdAt: string;
  updatedAt: string;
};

export type RestoreMode = "full" | "terminals_only" | "ide_only" | "codex_only";

export type SnapshotCodexProfilePayload = {
  id: string;
  name: string;
  codexHome: string;
  startupMode: string;
  resumeStrategy: string;
  defaultArgs: string[];
};

export type SnapshotProjectPayload = {
  id: string;
  name: string;
  path: string;
  platform: string;
  terminalType: string;
  ideType?: string | null;
  openTerminal: boolean;
  openIde: boolean;
  autoResumeCodex: boolean;
  sortOrder: number;
  codexProfile?: SnapshotCodexProfilePayload | null;
  launchTasks: SnapshotLaunchTaskPayload[];
};

export type SnapshotLaunchTaskPayload = {
  id: string;
  name: string;
  taskType: string;
  enabled: boolean;
  command: string;
  args: string[];
  workingDir: string;
  timeoutMs: number;
  continueOnFailure: boolean;
  retryPolicy: LaunchTaskRetryPolicy;
  sortOrder: number;
};

export type SnapshotWorkspacePayload = {
  id: string;
  name: string;
  description?: string | null;
  icon?: string | null;
  color?: string | null;
};

export type SnapshotPayload = {
  workspace: SnapshotWorkspacePayload;
  projects: SnapshotProjectPayload[];
  capturedAt: string;
};

export type SnapshotRecord = {
  id: string;
  workspaceId: string;
  workspaceName: string;
  name: string;
  description?: string | null;
  projectCount: number;
  payload: SnapshotPayload;
  lastRestoreAt?: string | null;
  lastRestoreStatus?: string | null;
  createdAt: string;
  updatedAt: string;
};

export type RestoreActionPlan = {
  id: string;
  kind: string;
  label: string;
  adapter: string;
  taskType?: string | null;
  launchTaskId?: string | null;
  continueOnFailure: boolean;
  status: string;
  reason?: string | null;
  startedAt?: string | null;
  finishedAt?: string | null;
  durationMs?: number | null;
  executablePath?: string | null;
  executableSource?: string | null;
  cancelRequestedAt?: string | null;
  diagnosticCode?: string | null;
};

export type RestoreProjectPlan = {
  projectId: string;
  projectName: string;
  path: string;
  status: string;
  reason?: string | null;
  actions: RestoreActionPlan[];
};

export type RestorePreviewStats = {
  totalProjects: number;
  plannedProjects: number;
  runningProjects: number;
  completedProjects: number;
  cancelledProjects: number;
  failedProjects: number;
  blockedProjects: number;
  skippedProjects: number;
  totalActions: number;
  plannedActions: number;
  runningActions: number;
  completedActions: number;
  cancelledActions: number;
  failedActions: number;
  blockedActions: number;
  skippedActions: number;
};

export type RestorePreview = {
  snapshot: SnapshotRecord;
  mode: RestoreMode;
  stats: RestorePreviewStats;
  projects: RestoreProjectPlan[];
};

export type RestoreRunSummary = {
  id: string;
  workspaceId: string;
  workspaceName: string;
  snapshotId?: string | null;
  snapshotName?: string | null;
  runMode: string;
  status: string;
  startedAt: string;
  finishedAt?: string | null;
  errorSummary?: string | null;
  plannedTaskCount: number;
  runningTaskCount: number;
  completedTaskCount: number;
  cancelledTaskCount: number;
  failedTaskCount: number;
  blockedTaskCount: number;
  skippedTaskCount: number;
};

export type CancelRestoreRunResult = {
  cancelled: boolean;
  status: "cancel_requested" | "already_finished" | "not_found";
  terminatedProcessCount: number;
};

export type CancelRestoreActionResult = {
  cancelled: boolean;
  status: "cancel_requested" | "already_finished" | "not_found";
  terminatedProcessCount: number;
  runId: string;
  projectTaskId: string;
  actionId: string;
};

export type RestoreRunProjectRecord = {
  id: string;
  restoreRunId: string;
  projectId?: string | null;
  projectName: string;
  path: string;
  status: string;
  attemptCount: number;
  startedAt?: string | null;
  finishedAt?: string | null;
  errorMessage?: string | null;
  actions: RestoreActionPlan[];
};

export type RestoreRunDetail = {
  run: RestoreRunSummary;
  snapshot: SnapshotRecord;
  stats: RestorePreviewStats;
  tasks: RestoreRunProjectRecord[];
};

export type AdapterAvailability = {
  key: string;
  label: string;
  available: boolean;
  status: string;
  executablePath?: string | null;
  source: string;
  message: string;
};

export type RestoreCapabilities = {
  checkedAt: string;
  terminal: AdapterAvailability;
  vscode: AdapterAvailability;
  jetbrains: AdapterAvailability;
  codex: AdapterAvailability;
};

export type RecentRestoreTarget = {
  id: string;
  workspaceId: string;
  workspaceName: string;
  snapshotId: string;
  snapshotName: string;
  projectCount: number;
  snapshotUpdatedAt: string;
  lastRestoreAt?: string | null;
  lastRestoreStatus?: string | null;
};

export type TerminalCommandTemplateRecord = {
  id: string;
  name: string;
  commandLine: string;
  sortOrder: number;
};

export type TerminalPreferences = {
  windowsTerminalPath?: string | null;
  codexCliPath?: string | null;
  commandTemplates: TerminalCommandTemplateRecord[];
};

export type IdePreferences = {
  vscodePath?: string | null;
  jetbrainsPath?: string | null;
};

export type WorkspaceEditorKey = "vscode" | "jetbrains";

export type WorkspacePreferences = {
  selectedWorkspaceIds: string[];
};

export type CodexHomeDirectoryInfo = {
  path?: string | null;
  source: "env" | "default" | "unavailable";
  exists: boolean;
};

export type TrayPreferences = {
  closeToTray: boolean;
  showRecentWorkspaces: boolean;
};

export type DiagnosticsPreferences = {
  showAdapterSources: boolean;
  showExecutablePaths: boolean;
};

export type AppPreferences = {
  terminal: TerminalPreferences;
  ide: IdePreferences;
  workspace: WorkspacePreferences;
  tray: TrayPreferences;
  diagnostics: DiagnosticsPreferences;
};

export type OpenLogDirectoryResult = {
  path: string;
};

export type OpenWorkspaceTerminalResult = {
  workspaceId: string;
  workspacePath: string;
};

export type OpenWorkspaceInEditorResult = {
  workspaceId: string;
  workspacePath: string;
  editorKey: WorkspaceEditorKey;
  editorLabel: string;
};

export type RunWorkspaceTerminalCommandResult = {
  workspaceId: string;
  launchTaskId: string;
  workspacePath: string;
  commandLine: string;
};

export type RunWorkspaceTerminalCommandsResult = {
  workspaceId: string;
  workspacePath: string;
  launchedTaskIds: string[];
  launchedCount: number;
  windowTarget: string;
  staggerMs: number;
};

export type RestoreRunEvent = {
  eventType: string;
  runId: string;
  workspaceId: string;
  snapshotId?: string | null;
  projectId?: string | null;
  projectTaskId?: string | null;
  launchTaskId?: string | null;
  status?: string | null;
  message?: string | null;
  occurredAt: string;
  run?: RestoreRunSummary | null;
  project?: RestoreRunProjectRecord | null;
  action?: RestoreActionPlan | null;
  stats?: RestorePreviewStats | null;
};

export type DeleteResult = {
  id: string;
};

export type UpsertWorkspacePayload = {
  id?: string;
  name: string;
  description?: string;
  icon?: string;
  color?: string;
  sortOrder?: number;
  isDefault?: boolean;
  isArchived?: boolean;
};

export type UpsertProjectPayload = {
  id?: string;
  workspaceId: string;
  name: string;
  path: string;
  platform: string;
  terminalType: string;
  ideType?: string;
  codexProfileId?: string;
  openTerminal: boolean;
  openIde: boolean;
  autoResumeCodex: boolean;
  sortOrder?: number;
};

export type UpsertLaunchTaskPayload = {
  id?: string;
  projectId: string;
  name: string;
  taskType: string;
  enabled?: boolean;
  command: string;
  args: string[];
  workingDir?: string;
  timeoutMs?: number;
  continueOnFailure?: boolean;
  retryPolicy?: LaunchTaskRetryPolicy;
  sortOrder?: number;
};

export type UpsertCodexProfilePayload = {
  id?: string;
  name: string;
  description?: string;
  codexHome: string;
  startupMode: string;
  resumeStrategy: string;
  defaultArgs: string[];
  isDefault?: boolean;
};

export type CreateSnapshotPayload = {
  workspaceId: string;
  name: string;
  description?: string;
};

export type UpdateSnapshotPayload = {
  id: string;
  name: string;
  description?: string;
};

export type RestorePreviewPayload = {
  snapshotId: string;
  mode: RestoreMode;
};

export type StartRestoreDryRunPayload = {
  snapshotId: string;
  mode: RestoreMode;
};

export type StartRestoreRunPayload = {
  snapshotId: string;
  mode: RestoreMode;
};
