use serde::{Deserialize, Serialize};

use super::SnapshotLaunchTaskPayload;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRecord {
    pub id: String,
    pub workspace_id: String,
    pub workspace_name: String,
    pub name: String,
    pub description: Option<String>,
    pub project_count: i64,
    pub payload: SnapshotPayload,
    pub last_restore_at: Option<String>,
    pub last_restore_status: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotPayload {
    pub workspace: SnapshotWorkspacePayload,
    pub projects: Vec<SnapshotProjectPayload>,
    pub captured_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotWorkspacePayload {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotProjectPayload {
    pub id: String,
    pub name: String,
    pub path: String,
    pub platform: String,
    pub terminal_type: String,
    pub ide_type: Option<String>,
    pub open_terminal: bool,
    pub open_ide: bool,
    pub auto_resume_codex: bool,
    pub sort_order: i64,
    pub codex_profile: Option<SnapshotCodexProfilePayload>,
    #[serde(default)]
    pub launch_tasks: Vec<SnapshotLaunchTaskPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotCodexProfilePayload {
    pub id: String,
    pub name: String,
    pub codex_home: String,
    pub startup_mode: String,
    pub resume_strategy: String,
    pub default_args: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSnapshotInput {
    pub workspace_id: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSnapshotInput {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}
