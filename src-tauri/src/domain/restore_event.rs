use serde::{Deserialize, Serialize};

use super::{RestoreActionPlan, RestorePreviewStats, RestoreRunProjectRecord, RestoreRunSummary};

pub const RESTORE_RUN_EVENT_NAME: &str = "restore://run-event";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreRunEvent {
    pub event_type: String,
    pub run_id: String,
    pub workspace_id: String,
    pub snapshot_id: Option<String>,
    pub project_id: Option<String>,
    pub project_task_id: Option<String>,
    pub launch_task_id: Option<String>,
    pub status: Option<String>,
    pub message: Option<String>,
    pub occurred_at: String,
    pub run: Option<RestoreRunSummary>,
    pub project: Option<RestoreRunProjectRecord>,
    pub action: Option<RestoreActionPlan>,
    pub stats: Option<RestorePreviewStats>,
}
