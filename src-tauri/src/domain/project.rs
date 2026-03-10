use serde::{Deserialize, Serialize};

use super::LaunchTaskRecord;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRecord {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub path: String,
    pub platform: String,
    pub terminal_type: String,
    pub ide_type: Option<String>,
    pub codex_profile_id: Option<String>,
    pub open_terminal: bool,
    pub open_ide: bool,
    pub auto_resume_codex: bool,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
    pub launch_tasks: Vec<LaunchTaskRecord>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertProjectInput {
    pub id: Option<String>,
    pub workspace_id: String,
    pub name: String,
    pub path: String,
    pub platform: String,
    pub terminal_type: String,
    pub ide_type: Option<String>,
    pub codex_profile_id: Option<String>,
    pub open_terminal: bool,
    pub open_ide: bool,
    pub auto_resume_codex: bool,
    pub sort_order: Option<i64>,
}
