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

export type WorkspaceImportAction = "create" | "skip" | "update";

export type WorkspaceImportProjectStatus = "ready" | "existing" | "missing" | "invalid";

export type WorkspaceImportItemStatus = "ready" | "existing" | "blocked";

export type WorkspaceImportRelocation = {
  workspaceIndex: number;
  projectIndex: number;
  path: string;
};

export type WorkspaceImportSelection = {
  workspaceIndex: number;
  action: WorkspaceImportAction;
};

export type WorkspaceImportProjectPreview = {
  projectIndex: number;
  name: string;
  sourcePath: string;
  resolvedPath: string;
  status: WorkspaceImportProjectStatus;
  existingWorkspaceId?: string | null;
  existingWorkspaceName?: string | null;
  message?: string | null;
};

export type WorkspaceImportItemPreview = {
  workspaceIndex: number;
  name: string;
  suggestedName: string;
  status: WorkspaceImportItemStatus;
  recommendedAction: WorkspaceImportAction;
  canCreate: boolean;
  canUpdate: boolean;
  existingWorkspaceId?: string | null;
  existingWorkspaceName?: string | null;
  projects: WorkspaceImportProjectPreview[];
  warnings: string[];
};

export type WorkspaceImportPreviewSummary = {
  totalWorkspaceCount: number;
  readyWorkspaceCount: number;
  existingWorkspaceCount: number;
  blockedWorkspaceCount: number;
  missingProjectCount: number;
  invalidProjectCount: number;
};

export type WorkspaceImportPreview = {
  sourcePath: string;
  fileHash: string;
  schemaVersion: number;
  summary: WorkspaceImportPreviewSummary;
  items: WorkspaceImportItemPreview[];
  warnings: string[];
};

export type ExportWorkspaceListPayload = {
  destinationPath: string;
};

export type ExportWorkspaceListResult = {
  destinationPath: string;
  workspaceCount: number;
  projectCount: number;
  launchTaskCount: number;
  warnings: string[];
};

export type PreviewWorkspaceListImportPayload = {
  sourcePath: string;
  relocations: WorkspaceImportRelocation[];
};

export type ApplyWorkspaceListImportPayload = PreviewWorkspaceListImportPayload & {
  expectedFileHash: string;
  selections: WorkspaceImportSelection[];
};

export type WorkspaceImportApplyResult = {
  createdWorkspaceCount: number;
  updatedWorkspaceCount: number;
  skippedWorkspaceCount: number;
  importedProjectCount: number;
  importedLaunchTaskCount: number;
  warnings: string[];
};

export type WorkspaceResourceEntry = {
  path: string;
  name: string;
  kind: "file" | "directory";
  isHidden: boolean;
};

export type WorkspaceResourceGitStatus =
  | "modified"
  | "renamed"
  | "untracked"
  | "ignored";

export type WorkspaceResourceGitStatusEntry = {
  path: string;
  status: WorkspaceResourceGitStatus;
  originalPath?: string | null;
};

export type WorkspaceResourceGitStatusResponse = {
  workspaceRootPath: string;
  gitAvailable: boolean;
  repositoryRootPath?: string | null;
  statuses: WorkspaceResourceGitStatusEntry[];
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

export type PromptRecord = {
  id: string;
  title: string;
  content: string;
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
};

export type PromptImportAction = "create" | "skip" | "update";

export type PromptImportItemStatus = "ready" | "existing" | "conflict" | "duplicate";

export type PromptImportSelection = {
  promptIndex: number;
  action: PromptImportAction;
};

export type PromptImportItemPreview = {
  promptIndex: number;
  id: string;
  title: string;
  contentPreview: string;
  status: PromptImportItemStatus;
  recommendedAction: PromptImportAction;
  canCreate: boolean;
  canUpdate: boolean;
  matchedPromptId?: string | null;
  matchedPromptTitle?: string | null;
  message: string;
};

export type PromptImportPreviewSummary = {
  totalPromptCount: number;
  readyPromptCount: number;
  existingPromptCount: number;
  conflictPromptCount: number;
  duplicatePromptCount: number;
};

export type PromptImportPreview = {
  sourcePath: string;
  fileHash: string;
  schemaVersion: number;
  summary: PromptImportPreviewSummary;
  items: PromptImportItemPreview[];
};

export type ExportPromptListPayload = {
  destinationPath: string;
};

export type ExportPromptListResult = {
  promptCount: number;
  fileSizeBytes: number;
  fileHash: string;
};

export type PreviewPromptListImportPayload = {
  sourcePath: string;
};

export type ApplyPromptListImportPayload = PreviewPromptListImportPayload & {
  expectedFileHash: string;
  selections: PromptImportSelection[];
};

export type PromptImportApplyResult = {
  createdPromptCount: number;
  updatedPromptCount: number;
  skippedPromptCount: number;
  prompts: PromptRecord[];
};

export type CodexAuthQuotaTier = {
  name: string;
  utilization: number;
  resetsAt?: string | null;
};

export type CodexAuthQuotaResult = {
  profileId: string;
  success: boolean;
  credentialStatus: string;
  credentialMessage?: string | null;
  tiers: CodexAuthQuotaTier[];
  error?: string | null;
  queriedAt: string;
};

export type CodexAuthProfileSummary = {
  id: string;
  name: string;
  description?: string | null;
  codexHome: string;
  isActive: boolean;
  lastQuota?: CodexAuthQuotaResult | null;
  lastQuotaCheckedAt?: string | null;
  createdAt: string;
  updatedAt: string;
};

export type CodexAuthProfileDetail = CodexAuthProfileSummary & {
  authJson: string;
  configToml: string;
};

export type CodexAuthSwitchResult = {
  profile: CodexAuthProfileSummary;
  authPath: string;
  configPath: string;
};

export type CodexAuthQuotaRefreshBatchResult = {
  total: number;
  refreshed: number;
  failed: number;
  startedAt: string;
  finishedAt: string;
  profiles: CodexAuthProfileSummary[];
};

export type OpenCodexHistoryWindowResult = {
  workspaceId: string;
  windowLabel: string;
};

export type CodexHistorySession = {
  providerId: string;
  sessionId: string;
  title?: string | null;
  summary?: string | null;
  projectDir?: string | null;
  createdAt?: string | null;
  lastActiveAt?: string | null;
  sourcePath: string;
  fileSizeBytes: number;
};

export type CodexHistorySessionsResponse = {
  workspaceId: string;
  workspacePath: string;
  codexRoots: string[];
  sessions: CodexHistorySession[];
};

export type ListCodexHistorySessionsPagePayload = {
  cursor?: string | null;
  limit?: number;
  workspaceId?: string | null;
  query?: string | null;
};

export type CodexHistoryGlobalSessionsPage = {
  codexRoots: string[];
  sessions: CodexHistorySession[];
  nextCursor?: string | null;
  hasMore: boolean;
};

export type CodexHistoryMessage = {
  role: string;
  content: string;
  timestamp?: string | null;
  itemType: string;
  truncated: boolean;
};

export type CodexHistoryMessagesPayload = {
  workspaceId?: string | null;
  sourcePath: string;
  cursor?: string | null;
  limit?: number;
};

export type CodexHistoryMessagesPage = {
  messages: CodexHistoryMessage[];
  olderCursor?: string | null;
  hasMore: boolean;
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

export type EditorPathDetectionResult = {
  checkedAt: string;
  vscode: AdapterAvailability;
  jetbrains: AdapterAvailability;
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

export type TerminalCommandShell = "powershell7" | "cmd";

export type TerminalPreferences = {
  windowsTerminalPath?: string | null;
  codexCliPath?: string | null;
  commandShell: TerminalCommandShell;
  commandTemplates: TerminalCommandTemplateRecord[];
};

export type CustomEditorRecord = {
  id: string;
  name: string;
  command: string;
};

export type IdePreferences = {
  vscodePath?: string | null;
  jetbrainsPath?: string | null;
  customEditors: CustomEditorRecord[];
};

export type WorkspaceEditorKey = string;

export type WorkspacePreferences = {
  selectedWorkspaceIds: string[];
  pinnedWorkspaceIds: string[];
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

export type CodexHistoryViewPreferences = {
  messageFontFamily: string;
  messageFontSize: number;
};

export type CodexAuthProxyMode = "system" | "manual" | "disabled";

export type CodexAuthProxyPreferences = {
  mode: CodexAuthProxyMode;
  manualProxyUrl: string;
};

export type CodexAuthPreferences = {
  quotaRefreshIntervalSeconds: number;
  proxy: CodexAuthProxyPreferences;
};

export type StartupPreferences = {
  launchAtLogin: boolean;
  startSilently: boolean;
};

export type HotkeyPreferences = {
  screenshotCapture: string;
  voiceInputToggle?: string | null;
  voiceInputHold?: string | null;
  promptQuickPasteSlots: PromptQuickPasteHotkeySlot[];
};

export type PromptQuickPasteHotkeySlot = {
  slot: number;
  enabled: boolean;
  promptId?: string | null;
  shortcut: string;
};

export type AppPreferences = {
  terminal: TerminalPreferences;
  ide: IdePreferences;
  workspace: WorkspacePreferences;
  startup: StartupPreferences;
  hotkey: HotkeyPreferences;
  tray: TrayPreferences;
  diagnostics: DiagnosticsPreferences;
  codexHistory: CodexHistoryViewPreferences;
  codexAuth: CodexAuthPreferences;
};

export type WorkspacePreferencesPatch = {
  selectedWorkspaceIds?: string[];
  pinnedWorkspaceIds?: string[];
};

export type AppPreferencesPatch = {
  terminal?: TerminalPreferences;
  ide?: IdePreferences;
  workspace?: WorkspacePreferencesPatch;
  startup?: StartupPreferences;
  hotkey?: HotkeyPreferences;
  tray?: TrayPreferences;
  diagnostics?: DiagnosticsPreferences;
  codexHistory?: CodexHistoryViewPreferences;
  codexAuth?: CodexAuthPreferences;
};

export type HotkeyHealthStatus = "uninitialized" | "ready" | "degraded";

export type HotkeyRegisteredBindingView = {
  action: HotkeyTriggerAction;
  shortcut: string;
};

export type HotkeyHealth = {
  status: HotkeyHealthStatus;
  initialized: boolean;
  registeredBindings: HotkeyRegisteredBindingView[];
  lastError?: AppError | null;
  updatedAt?: string | null;
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

export type HotkeyTriggerAction =
  | "screenshot_capture"
  | "voice_input_toggle"
  | "voice_input_hold"
  | "prompt_quick_paste_1"
  | "prompt_quick_paste_2"
  | "prompt_quick_paste_3"
  | "prompt_quick_paste_4"
  | "prompt_quick_paste_5";

export type HotkeyTriggerEvent = {
  action: HotkeyTriggerAction;
  shortcut: string;
  triggeredAt: string;
  source: string;
};

export type ScreenshotSelectionInput = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export type ScreenshotRenderedImageInput = {
  dataUrl: string;
};

export type PromptQuickPasteResultEvent = {
  slot: number;
  shortcut: string;
  status: "sent" | "failed";
  promptId?: string | null;
  promptTitle?: string | null;
  message: string;
  error?: AppError | null;
  occurredAt: string;
};

export type ScreenshotImageStatus = "loading" | "ready" | "failed";

export type ScreenshotPreviewTransport = "file" | "raw_rgba_fast";

export type ScreenshotSelectionRenderMode = "native" | "logical_fallback";

export type ScreenshotMonitorView = {
  displayId: number;
  displayX: number;
  displayY: number;
  relativeX: number;
  relativeY: number;
  displayWidth: number;
  displayHeight: number;
  captureWidth: number;
  captureHeight: number;
  scaleFactor: number;
};

export type ScreenshotSessionView = {
  sessionId: string;
  createdAt: string;
  displayX: number;
  displayY: number;
  displayWidth: number;
  displayHeight: number;
  scaleFactor: number;
  captureWidth: number;
  captureHeight: number;
  imageStatus: ScreenshotImageStatus;
  imageError?: string | null;
  nativePreviewActive: boolean;
  imageDataUrl: string;
  previewImagePath?: string | null;
  previewTransport: ScreenshotPreviewTransport;
  previewPixelWidth: number;
  previewPixelHeight: number;
  monitors: ScreenshotMonitorView[];
};

export type ScreenshotSelectionRenderTile = {
  displayId: number;
  scaleFactor: number;
  logicalX: number;
  logicalY: number;
  logicalWidth: number;
  logicalHeight: number;
  outputX: number;
  outputY: number;
  outputWidth: number;
  outputHeight: number;
};

export type ScreenshotSelectionRenderView = {
  sessionId: string;
  width: number;
  height: number;
  scaleFactor: number;
  renderMode: ScreenshotSelectionRenderMode;
  imageDataUrl: string;
  tiles: ScreenshotSelectionRenderTile[];
};

export type StartScreenshotSessionResult = {
  sessionId: string;
  windowLabel: string;
};

export type CopyScreenshotSelectionResult = {
  sessionId: string;
  width: number;
  height: number;
};

export type SaveScreenshotSelectionResult = {
  sessionId: string;
  filePath: string;
  width: number;
  height: number;
};

export type CancelScreenshotSessionResult = {
  sessionId: string;
  cancelled: boolean;
};

export type ScreenshotSessionUpdatedEvent = {
  sessionId: string;
  createdAt: string;
};

export type ScreenshotEscapePressedEvent = {
  sessionId: string;
  shortcut: string;
  triggeredAt: string;
};

export type NativeInteractionBackendKind = "windows_layered_selection_mvp";

export type NativeInteractionSelectionRect = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export type NativeInteractionSelectionPoint = {
  x: number;
  y: number;
};

export type NativeInteractionMode =
  | "selection"
  | "line_annotation"
  | "rect_annotation"
  | "ellipse_annotation"
  | "arrow_annotation";

export type NativeInteractionShapeAnnotationKind = "line" | "rect" | "ellipse" | "arrow";

export type NativeInteractionEditableShape = {
  id: string;
  kind: NativeInteractionShapeAnnotationKind;
  color: string;
  strokeWidth: number;
  start: NativeInteractionSelectionPoint;
  end: NativeInteractionSelectionPoint;
};

export type NativeInteractionExclusionRect = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export type NativeInteractionStateView = {
  backendKind?: NativeInteractionBackendKind | null;
  lifecycleState: string;
  hasActiveSession: boolean;
  selection?: NativeInteractionSelectionRect | null;
  activeShape?: NativeInteractionEditableShape | null;
  activeShapeDraft?: NativeInteractionEditableShape | null;
  hoveredHitRegion: string;
  dragMode?: string | null;
  selectionRevision: number;
  activeShapeRevision: number;
  interactionMode: NativeInteractionMode;
  rectDraft?: NativeInteractionSelectionRect | null;
};

export type NativeInteractionStateUpdatedEvent = {
  sessionId?: string | null;
  backendKind?: NativeInteractionBackendKind | null;
  lifecycleState: string;
  hasActiveSession: boolean;
  selection?: NativeInteractionSelectionRect | null;
  activeShape?: NativeInteractionEditableShape | null;
  activeShapeDraft?: NativeInteractionEditableShape | null;
  hoveredHitRegion: string;
  dragMode?: string | null;
  selectionRevision: number;
  activeShapeRevision: number;
  interactionMode: NativeInteractionMode;
  rectDraft?: NativeInteractionSelectionRect | null;
};

export type NativeInteractionShapeAnnotationCommittedEvent = {
  sessionId: string;
  kind: NativeInteractionShapeAnnotationKind;
  color: string;
  strokeWidth: number;
  start: NativeInteractionSelectionPoint;
  end: NativeInteractionSelectionPoint;
};

export type NativeInteractionShapeAnnotationUpdatedEvent = {
  sessionId: string;
  id: string;
  kind: NativeInteractionShapeAnnotationKind;
  color: string;
  strokeWidth: number;
  start: NativeInteractionSelectionPoint;
  end: NativeInteractionSelectionPoint;
};

export type DeleteResult = {
  id: string;
};

export type OssAccountSummary = {
  id: string;
  displayName: string;
  authMode: "access_key";
  accessKeyIdHint: string;
  lastProbeStatus: string;
  lastProbeError?: string | null;
  lastProbeAt?: string | null;
  isDisabled: boolean;
  createdAt: string;
  updatedAt: string;
};

export type OssTargetRecord = {
  id: string;
  accountId: string;
  displayName: string;
  bucket: string;
  region: string;
  endpoint: string;
  prefix: string;
  isDefault: boolean;
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
};

export type OssObjectEntry = {
  key: string;
  kind: "object" | "prefix" | string;
  size?: number | null;
  etag?: string | null;
  lastModified?: string | null;
  storageClass?: string | null;
};

export type OssObjectPage = {
  objects: OssObjectEntry[];
  commonPrefixes: string[];
  nextContinuationToken?: string | null;
  isTruncated: boolean;
};

export type OssProbeResult = {
  targetId: string;
  bucket: string;
  reachable: boolean;
  canListObjects: boolean;
  message: string;
  checkedAt: string;
};

export type OssObjectMetadata = {
  key: string;
  size?: number | null;
  etag?: string | null;
  contentType?: string | null;
  lastModified?: string | null;
  storageClass?: string | null;
  versionId?: string | null;
};

export type OssTransferStart = {
  operationId: string;
  taskId: string;
};

export type OssTransferTaskRecord = {
  id: string;
  accountId: string;
  targetId: string;
  operation: "upload" | "download" | string;
  objectKey: string;
  localPath: string;
  status: string;
  bytesCompleted: number;
  totalBytes: number;
  lastErrorCode?: string | null;
  lastErrorMessage?: string | null;
  createdAt: string;
  updatedAt: string;
};

export type OssTransferTaskView = OssTransferTaskRecord & {
  accountDisplayName: string;
  targetDisplayName: string;
  bucket: string;
  region: string;
};

export type UpsertOssAccountPayload = {
  id?: string;
  displayName: string;
  accessKeyId: string;
  accessKeySecret?: string;
};

export type UpsertOssTargetPayload = {
  id?: string;
  accountId: string;
  displayName: string;
  bucket: string;
  region: string;
  endpoint: string;
  prefix?: string;
  isDefault?: boolean;
  sortOrder?: number;
};

export type ListOssObjectsPayload = {
  targetId: string;
  prefix?: string;
  continuationToken?: string | null;
  pageSize?: number;
};

export type CreateOssFolderPayload = {
  targetId: string;
  parentPrefix: string;
  name: string;
};

export type CreateOssFolderResult = {
  targetId: string;
  objectKey: string;
};

export type HeadOssObjectPayload = {
  targetId: string;
  objectKey: string;
};

export type GetOssObjectDownloadUrlPayload = {
  targetId: string;
  objectKey: string;
  expiresInSeconds?: number;
};

export type GetOssObjectDownloadUrlResult = {
  targetId: string;
  objectKey: string;
  url: string;
  expiresAt: string;
};

export type TestOssTargetPayload = {
  targetId: string;
};

export type StartOssUploadPayload = {
  targetId: string;
  localPath: string;
  objectKey: string;
  overwrite: boolean;
};

export type StartOssDownloadPayload = {
  targetId: string;
  objectKey: string;
  localPath: string;
  overwrite: boolean;
};

export type OssOperationPayload = {
  operationId: string;
};

export type DeleteOssObjectsPayload = {
  targetId: string;
  objectKeys: string[];
};

export type CopyOssObjectPayload = {
  targetId: string;
  sourceKey: string;
  targetKey: string;
  overwrite: boolean;
};

export type DeleteOssObjectsResult = {
  deletedKeys: string[];
};

export type CopyOssObjectResult = {
  sourceKey: string;
  targetKey: string;
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

export type UpdateWorkspaceDescriptionPayload = {
  workspaceId: string;
  description: string;
};

export type ReorderWorkspacesPayload = {
  workspaceIds: string[];
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

export type ReorderLaunchTasksPayload = {
  projectId: string;
  launchTaskIds: string[];
};

export type UpsertPromptPayload = {
  id?: string;
  title: string;
  content: string;
};

export type ReorderPromptsPayload = {
  promptIds: string[];
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

export type UpsertCodexAuthProfilePayload = {
  id?: string;
  name: string;
  description?: string;
  codexHome: string;
  authJson: string;
  configToml: string;
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
