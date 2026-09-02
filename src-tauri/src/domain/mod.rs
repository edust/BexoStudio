mod adapter;
mod codex_auth;
mod codex_history;
mod codex_profile;
mod hotkey;
mod launch_task;
mod native_interaction;
mod oss;
mod preferences;
mod project;
mod prompt;
mod prompt_transfer;
mod resource_browser;
mod restore_event;
mod restore_run;
mod restore_run_task;
mod screenshot;
mod snapshot;
mod validation;
mod workspace;
mod workspace_transfer;

use serde::Serialize;

pub use adapter::{
    AdapterAvailability, EditorPathDetectionResult, OpenLogDirectoryResult,
    OpenWorkspaceInEditorResult, OpenWorkspaceTerminalResult, RestoreCapabilities,
    RunWorkspaceTerminalCommandResult, RunWorkspaceTerminalCommandsResult, StartRestoreRunInput,
};
pub use codex_auth::{
    ensure_absolute_path, validate_codex_auth_json, validate_codex_config_toml, CodexAuthJson,
    CodexAuthProfileDetail, CodexAuthProfileRecord, CodexAuthProfileSummary,
    CodexAuthQuotaRefreshBatchResult, CodexAuthQuotaResult, CodexAuthQuotaTier,
    CodexAuthSwitchResult, UpsertCodexAuthProfileInput,
};
pub use codex_history::{
    CodexHistoryGlobalSessionsPage, CodexHistoryMessage, CodexHistoryMessagesInput,
    CodexHistoryMessagesPage, CodexHistorySession, CodexHistorySessionsResponse,
    ListCodexHistorySessionsInput, ListCodexHistorySessionsPageInput, OpenCodexHistoryWindowResult,
};
pub use codex_profile::{CodexProfileRecord, UpsertCodexProfileInput};
pub use hotkey::{
    HotkeyAction, HotkeyHealth, HotkeyHealthStatus, HotkeyRegisteredBindingView,
    HotkeyTriggerEvent, PromptQuickPasteResultEvent, PromptQuickPasteResultStatus,
    HOTKEY_TRIGGER_EVENT_NAME, PROMPT_QUICK_PASTE_RESULT_EVENT_NAME,
};
pub use launch_task::{
    validate_launch_task_args, validate_launch_task_command, validate_launch_task_id,
    validate_launch_task_retry_policy, validate_launch_task_timeout, validate_launch_task_type,
    validate_launch_task_working_dir, LaunchTaskRecord, LaunchTaskRetryPolicy,
    ReorderLaunchTasksInput, SnapshotLaunchTaskPayload, UpsertLaunchTaskInput,
};
pub use native_interaction::{
    NATIVE_INTERACTION_SHAPE_ANNOTATION_COMMITTED_EVENT_NAME,
    NATIVE_INTERACTION_SHAPE_ANNOTATION_UPDATED_EVENT_NAME,
    NATIVE_INTERACTION_STATE_UPDATED_EVENT_NAME,
};
#[allow(unused_imports)]
pub use oss::{
    credential_ref_for_account, mask_access_key_id, normalize_access_key_id,
    normalize_access_key_id_hint, normalize_access_key_secret, normalize_bucket,
    normalize_credential_ref, normalize_display_name, normalize_download_url_expires,
    normalize_endpoint, normalize_object_key, normalize_oss_account_input,
    normalize_oss_folder_name, normalize_oss_metadata_input, normalize_oss_target_input,
    normalize_page_size, normalize_prefix, normalize_region, validate_account_id,
    validate_operation_id, validate_target_id, CopyOssObjectInput, CopyOssObjectResult,
    CreateOssFolderInput, CreateOssFolderResult, DeleteOssObjectsInput, DeleteOssObjectsResult,
    GetOssObjectDownloadUrlInput, GetOssObjectDownloadUrlResult, HeadOssObjectInput,
    ListOssObjectsInput, OssAccountRecord, OssAccountSummary, OssCredential, OssObjectEntry,
    OssObjectMetadata, OssObjectPage, OssOperationInput, OssProbeResult, OssTargetRecord,
    OssTransferStart, OssTransferTaskRecord, OssTransferTaskView, StartOssDownloadInput,
    StartOssUploadInput, TestOssTargetInput, UpsertOssAccountInput, UpsertOssAccountMetadataInput,
    UpsertOssTargetInput, OSS_AUTH_MODE_ACCESS_KEY, OSS_DEFAULT_DOWNLOAD_URL_EXPIRES_SECONDS,
    OSS_DEFAULT_PAGE_SIZE, OSS_MAX_DOWNLOAD_URL_EXPIRES_SECONDS, OSS_MAX_FOLDER_NAME_CHARS,
    OSS_MAX_OBJECT_KEY_BYTES, OSS_MAX_PAGE_SIZE, OSS_MAX_PREFIX_BYTES, OSS_MAX_TRANSFER_RETRIES,
    OSS_MIN_DOWNLOAD_URL_EXPIRES_SECONDS, OSS_TRANSFER_PROGRESS_EVENT_NAME,
    OSS_TRANSFER_STATE_EVENT_NAME,
};
#[allow(unused_imports)]
pub use preferences::{
    AppPreferences, AppPreferencesPatch, CodexAuthPreferences, CodexAuthProxyPreferences,
    CodexHistoryViewPreferences, CodexHomeDirectoryInfo, CustomEditorPreference,
    DiagnosticsPreferences, HotkeyPreferences, IdePreferences, PromptQuickPasteHotkeySlot,
    StartupPreferences, TerminalCommandShell, TerminalCommandTemplate, TerminalPreferences,
    TrayPreferences, WorkspacePreferences, WorkspacePreferencesPatch,
    DEFAULT_CODEX_AUTH_PROXY_MODE, DEFAULT_CODEX_AUTH_QUOTA_REFRESH_INTERVAL_SECONDS,
    DEFAULT_CODEX_HISTORY_MESSAGE_FONT_SIZE, DEFAULT_PROMPT_QUICK_PASTE_HOTKEYS,
    DEFAULT_SCREENSHOT_CAPTURE_HOTKEY, DEFAULT_TERMINAL_COMMAND_SHELL,
    EARLIER_DEFAULT_SCREENSHOT_CAPTURE_HOTKEY, LEGACY_SCREENSHOT_CAPTURE_HOTKEY,
    PREVIOUS_DEFAULT_SCREENSHOT_CAPTURE_HOTKEY, PROMPT_QUICK_PASTE_SLOT_COUNT,
};
pub use project::{ProjectRecord, UpsertProjectInput};
pub use prompt::{
    validate_prompt_content, validate_prompt_id, validate_prompt_title, PromptRecord,
    ReorderPromptsInput, UpsertPromptInput, MAX_PROMPTS,
};
#[allow(unused_imports)]
pub use prompt_transfer::{
    build_prompt_import_preview, classify_prompt_import_entries, prompt_import_action_allowed,
    validate_prompt_import_selections, validate_prompt_transfer_document,
    ApplyPromptListImportInput, ExportPromptListInput, ExportPromptListResult,
    PreviewPromptListImportInput, PromptImportAction, PromptImportApplyResult,
    PromptImportClassification, PromptImportItemPreview, PromptImportItemStatus,
    PromptImportPreview, PromptImportPreviewSummary, PromptImportSelection, PromptTransferDocument,
    PromptTransferPrompt, PROMPT_TRANSFER_FORMAT, PROMPT_TRANSFER_MAX_FILE_BYTES,
    PROMPT_TRANSFER_SCHEMA_VERSION,
};
pub use resource_browser::{
    WorkspaceResourceEntry, WorkspaceResourceGitStatusEntry, WorkspaceResourceGitStatusResponse,
};
pub use restore_event::{RestoreRunEvent, RESTORE_RUN_EVENT_NAME};
pub use restore_run::{
    CancelRestoreActionResult, CancelRestoreRunResult, RecentRestoreTarget, RestorePreview,
    RestorePreviewInput, RestorePreviewStats, RestoreRunDetail, RestoreRunSummary,
    StartRestoreDryRunInput,
};
pub use restore_run_task::{
    RestoreActionPlan, RestoreProjectPlan, RestoreRunProjectRecord, RestoreRunTaskRecord,
};
pub use screenshot::{
    CancelScreenshotSessionResult, CopyScreenshotSelectionResult, SaveScreenshotSelectionResult,
    ScreenshotEscapePressedEvent, ScreenshotImageStatus, ScreenshotMonitorView,
    ScreenshotPreviewTransport, ScreenshotRenderedImageInput, ScreenshotSelectionInput,
    ScreenshotSelectionRenderMode, ScreenshotSelectionRenderTile, ScreenshotSelectionRenderView,
    ScreenshotSessionUpdatedEvent, ScreenshotSessionView, StartScreenshotSessionResult,
    SCREENSHOT_ESCAPE_PRESSED_EVENT_NAME, SCREENSHOT_OVERLAY_WINDOW_LABEL,
    SCREENSHOT_SESSION_UPDATED_EVENT_NAME,
};
pub use snapshot::{
    CreateSnapshotInput, SnapshotCodexProfilePayload, SnapshotPayload, SnapshotProjectPayload,
    SnapshotRecord, SnapshotWorkspacePayload, UpdateSnapshotInput,
};
pub use validation::{
    ensure_absolute_directory, parse_color_or_none, parse_json_string_list, parse_restore_mode,
    require_non_empty, validate_optional_uuid, validate_ordered_uuid_list, MAX_REORDER_ITEMS,
};
pub use workspace::{
    normalize_workspace_description, validate_workspace_id, DeleteResult, ReorderWorkspacesInput,
    UpdateWorkspaceDescriptionInput, UpsertWorkspaceInput, WorkspaceRecord,
};
pub use workspace_transfer::{
    validate_workspace_transfer_document, ApplyWorkspaceListImportInput, ExportWorkspaceListInput,
    ExportWorkspaceListResult, PreviewWorkspaceListImportInput, WorkspaceImportAction,
    WorkspaceImportApplyResult, WorkspaceImportItemPreview, WorkspaceImportItemStatus,
    WorkspaceImportPreview, WorkspaceImportPreviewSummary, WorkspaceImportProjectPreview,
    WorkspaceImportProjectStatus, WorkspaceImportRelocation, WorkspaceImportSelection,
    WorkspaceTransferDocument, WorkspaceTransferLaunchTask, WorkspaceTransferProject,
    WorkspaceTransferWorkspace, WORKSPACE_TRANSFER_FORMAT, WORKSPACE_TRANSFER_MAX_FILE_BYTES,
    WORKSPACE_TRANSFER_SCHEMA_VERSION,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteSection {
    pub key: &'static str,
    pub label: &'static str,
    pub summary: &'static str,
}

pub fn primary_sections() -> [RouteSection; 5] {
    [
        RouteSection {
            key: "home",
            label: "Home",
            summary: "恢复入口、最近运行与桌面状态。",
        },
        RouteSection {
            key: "history",
            label: "Session / History",
            summary: "全局只读查看 Codex 历史会话。",
        },
        RouteSection {
            key: "codex_auth",
            label: "Codex Auth",
            summary: "管理本机 Codex 授权配置。",
        },
        RouteSection {
            key: "prompts",
            label: "Prompts",
            summary: "保存、排序并快速复制常用 Prompt。",
        },
        RouteSection {
            key: "settings",
            label: "Settings",
            summary: "桌面运行策略与系统设置。",
        },
    ]
}
