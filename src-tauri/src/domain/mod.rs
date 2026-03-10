mod adapter;
mod codex_profile;
mod launch_task;
mod preferences;
mod project;
mod resource_browser;
mod restore_event;
mod restore_run;
mod restore_run_task;
mod snapshot;
mod validation;
mod workspace;

use serde::Serialize;

pub use adapter::{
    AdapterAvailability, EditorPathDetectionResult, OpenLogDirectoryResult,
    OpenWorkspaceInEditorResult, OpenWorkspaceTerminalResult, RestoreCapabilities,
    RunWorkspaceTerminalCommandResult, RunWorkspaceTerminalCommandsResult, StartRestoreRunInput,
};
pub use codex_profile::{CodexProfileRecord, UpsertCodexProfileInput};
pub use launch_task::{
    validate_launch_task_args, validate_launch_task_command, validate_launch_task_id,
    validate_launch_task_retry_policy, validate_launch_task_timeout, validate_launch_task_type,
    validate_launch_task_working_dir, LaunchTaskRecord, LaunchTaskRetryPolicy,
    SnapshotLaunchTaskPayload, UpsertLaunchTaskInput,
};
#[allow(unused_imports)]
pub use preferences::{
    AppPreferences, CodexHomeDirectoryInfo, CustomEditorPreference, DiagnosticsPreferences,
    IdePreferences, StartupPreferences, TerminalCommandTemplate, TerminalPreferences,
    TrayPreferences, WorkspacePreferences,
};
pub use project::{ProjectRecord, UpsertProjectInput};
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
pub use snapshot::{
    CreateSnapshotInput, SnapshotCodexProfilePayload, SnapshotPayload, SnapshotProjectPayload,
    SnapshotRecord, SnapshotWorkspacePayload, UpdateSnapshotInput,
};
pub use validation::{
    ensure_absolute_directory, parse_color_or_none, parse_json_string_list, parse_restore_mode,
    require_non_empty, validate_optional_uuid,
};
pub use workspace::{DeleteResult, UpsertWorkspaceInput, WorkspaceRecord};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteSection {
    pub key: &'static str,
    pub label: &'static str,
    pub summary: &'static str,
}

pub fn primary_sections() -> [RouteSection; 6] {
    [
        RouteSection {
            key: "home",
            label: "Home",
            summary: "恢复入口、最近运行与桌面状态。",
        },
        RouteSection {
            key: "workspaces",
            label: "Workspaces",
            summary: "工作区与项目编排总览。",
        },
        RouteSection {
            key: "snapshots",
            label: "Snapshots",
            summary: "快照与恢复预览入口。",
        },
        RouteSection {
            key: "profiles",
            label: "Profiles",
            summary: "Codex Profile 与工具偏好。",
        },
        RouteSection {
            key: "logs",
            label: "Logs",
            summary: "恢复结果与诊断记录。",
        },
        RouteSection {
            key: "settings",
            label: "Settings",
            summary: "桌面运行策略与系统设置。",
        },
    ]
}
