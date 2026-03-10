use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexProfileRecord {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub codex_home: String,
    pub startup_mode: String,
    pub resume_strategy: String,
    pub default_args: Vec<String>,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertCodexProfileInput {
    pub id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub codex_home: String,
    pub startup_mode: String,
    pub resume_strategy: String,
    pub default_args: Vec<String>,
    pub is_default: Option<bool>,
}
