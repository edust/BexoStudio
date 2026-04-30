use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListCodexHistorySessionsInput {
    pub workspace_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexHistoryMessagesInput {
    pub workspace_id: Option<String>,
    pub source_path: String,
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodexHistoryWindowResult {
    pub workspace_id: String,
    pub window_label: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexHistorySessionsResponse {
    pub workspace_id: String,
    pub workspace_path: String,
    pub codex_roots: Vec<String>,
    pub sessions: Vec<CodexHistorySession>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexHistoryGlobalSessionsResponse {
    pub codex_roots: Vec<String>,
    pub sessions: Vec<CodexHistorySession>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexHistorySession {
    pub provider_id: String,
    pub session_id: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub project_dir: Option<String>,
    pub created_at: Option<String>,
    pub last_active_at: Option<String>,
    pub source_path: String,
    pub file_size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexHistoryMessagesPage {
    pub messages: Vec<CodexHistoryMessage>,
    pub older_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexHistoryMessage {
    pub role: String,
    pub content: String,
    pub timestamp: Option<String>,
    pub item_type: String,
    pub truncated: bool,
}
