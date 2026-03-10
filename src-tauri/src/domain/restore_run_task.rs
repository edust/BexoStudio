use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreRunTaskRecord {
    pub id: String,
    pub restore_run_id: String,
    pub project_id: Option<String>,
    pub launch_task_id: Option<String>,
    pub status: String,
    pub attempt_count: i64,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreActionPlan {
    #[serde(default)]
    pub id: String,
    pub kind: String,
    pub label: String,
    pub adapter: String,
    #[serde(default)]
    pub task_type: Option<String>,
    #[serde(default)]
    pub launch_task_id: Option<String>,
    #[serde(default)]
    pub continue_on_failure: bool,
    pub status: String,
    pub reason: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub duration_ms: Option<i64>,
    pub executable_path: Option<String>,
    pub executable_source: Option<String>,
    #[serde(default)]
    pub cancel_requested_at: Option<String>,
    #[serde(default)]
    pub diagnostic_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreProjectPlan {
    pub project_id: String,
    pub project_name: String,
    pub path: String,
    pub status: String,
    pub reason: Option<String>,
    pub actions: Vec<RestoreActionPlan>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreRunProjectRecord {
    pub id: String,
    pub restore_run_id: String,
    pub project_id: Option<String>,
    pub project_name: String,
    pub path: String,
    pub status: String,
    pub attempt_count: i64,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub error_message: Option<String>,
    pub actions: Vec<RestoreActionPlan>,
}
