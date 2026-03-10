use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceResourceEntry {
    pub path: String,
    pub name: String,
    pub kind: String,
    pub is_hidden: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceResourceGitStatusEntry {
    pub path: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceResourceGitStatusResponse {
    pub workspace_root_path: String,
    pub git_available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository_root_path: Option<String>,
    pub statuses: Vec<WorkspaceResourceGitStatusEntry>,
}
