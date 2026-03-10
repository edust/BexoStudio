use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterAvailability {
    pub key: String,
    pub label: String,
    pub available: bool,
    pub status: String,
    pub executable_path: Option<String>,
    pub source: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreCapabilities {
    pub checked_at: String,
    pub terminal: AdapterAvailability,
    pub vscode: AdapterAvailability,
    pub jetbrains: AdapterAvailability,
    pub codex: AdapterAvailability,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorPathDetectionResult {
    pub checked_at: String,
    pub vscode: AdapterAvailability,
    pub jetbrains: AdapterAvailability,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRestoreRunInput {
    pub snapshot_id: String,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenLogDirectoryResult {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenWorkspaceTerminalResult {
    pub workspace_id: String,
    pub workspace_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenWorkspaceInEditorResult {
    pub workspace_id: String,
    pub workspace_path: String,
    pub editor_key: String,
    pub editor_label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunWorkspaceTerminalCommandResult {
    pub workspace_id: String,
    pub launch_task_id: String,
    pub workspace_path: String,
    pub command_line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunWorkspaceTerminalCommandsResult {
    pub workspace_id: String,
    pub workspace_path: String,
    pub launched_task_ids: Vec<String>,
    pub launched_count: usize,
    pub window_target: String,
    pub stagger_ms: i64,
}
