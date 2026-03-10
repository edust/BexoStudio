use serde::{Deserialize, Serialize};

use crate::domain::{RestoreProjectPlan, RestoreRunProjectRecord, SnapshotRecord};

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreRunRecord {
    pub id: String,
    pub workspace_id: String,
    pub snapshot_id: Option<String>,
    pub run_mode: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub error_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePreviewStats {
    pub total_projects: i64,
    pub planned_projects: i64,
    pub running_projects: i64,
    pub completed_projects: i64,
    pub cancelled_projects: i64,
    pub failed_projects: i64,
    pub blocked_projects: i64,
    pub skipped_projects: i64,
    pub total_actions: i64,
    pub planned_actions: i64,
    pub running_actions: i64,
    pub completed_actions: i64,
    pub cancelled_actions: i64,
    pub failed_actions: i64,
    pub blocked_actions: i64,
    pub skipped_actions: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePreview {
    pub snapshot: SnapshotRecord,
    pub mode: String,
    pub stats: RestorePreviewStats,
    pub projects: Vec<RestoreProjectPlan>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreRunSummary {
    pub id: String,
    pub workspace_id: String,
    pub workspace_name: String,
    pub snapshot_id: Option<String>,
    pub snapshot_name: Option<String>,
    pub run_mode: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub error_summary: Option<String>,
    pub planned_task_count: i64,
    pub running_task_count: i64,
    pub completed_task_count: i64,
    pub cancelled_task_count: i64,
    pub failed_task_count: i64,
    pub blocked_task_count: i64,
    pub skipped_task_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelRestoreRunResult {
    pub cancelled: bool,
    pub status: String,
    pub terminated_process_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelRestoreActionResult {
    pub cancelled: bool,
    pub status: String,
    pub terminated_process_count: i64,
    pub run_id: String,
    pub project_task_id: String,
    pub action_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentRestoreTarget {
    pub id: String,
    pub workspace_id: String,
    pub workspace_name: String,
    pub snapshot_id: String,
    pub snapshot_name: String,
    pub project_count: i64,
    pub snapshot_updated_at: String,
    pub last_restore_at: Option<String>,
    pub last_restore_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreRunDetail {
    pub run: RestoreRunSummary,
    pub snapshot: SnapshotRecord,
    pub stats: RestorePreviewStats,
    pub tasks: Vec<RestoreRunProjectRecord>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePreviewInput {
    pub snapshot_id: String,
    pub mode: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRestoreDryRunInput {
    pub snapshot_id: String,
    pub mode: String,
}
