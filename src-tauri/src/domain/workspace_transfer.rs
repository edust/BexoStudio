use std::path::Path;

use chrono::DateTime;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

use super::{
    normalize_workspace_description, parse_color_or_none, validate_launch_task_args,
    validate_launch_task_retry_policy, validate_launch_task_timeout, validate_launch_task_type,
    LaunchTaskRetryPolicy,
};

pub const WORKSPACE_TRANSFER_FORMAT: &str = "bexo-studio.workspace-list";
pub const WORKSPACE_TRANSFER_SCHEMA_VERSION: u32 = 1;
pub const WORKSPACE_TRANSFER_MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
pub const WORKSPACE_TRANSFER_MAX_WORKSPACES: usize = 500;
pub const WORKSPACE_TRANSFER_MAX_PROJECTS_PER_WORKSPACE: usize = 200;
pub const WORKSPACE_TRANSFER_MAX_TASKS_PER_PROJECT: usize = 1_000;
const WORKSPACE_TRANSFER_MAX_PATH_CHARS: usize = 4_096;
const WORKSPACE_TRANSFER_MAX_ICON_CHARS: usize = 160;
const WORKSPACE_TRANSFER_MAX_IDE_TYPE_CHARS: usize = 160;
const WORKSPACE_TRANSFER_MAX_APP_VERSION_CHARS: usize = 40;
const WORKSPACE_TRANSFER_MAX_SORT_ORDER: i64 = 1_000_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceTransferDocument {
    pub format: String,
    pub schema_version: u32,
    pub app_version: String,
    pub exported_at: String,
    pub workspaces: Vec<WorkspaceTransferWorkspace>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceTransferWorkspace {
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub sort_order: i64,
    pub is_default: bool,
    pub is_archived: bool,
    pub pinned: bool,
    pub projects: Vec<WorkspaceTransferProject>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceTransferProject {
    pub name: String,
    pub path: String,
    pub platform: String,
    pub terminal_type: String,
    pub ide_type: Option<String>,
    pub codex_profile_name: Option<String>,
    pub open_terminal: bool,
    pub open_ide: bool,
    pub auto_resume_codex: bool,
    pub sort_order: i64,
    pub launch_tasks: Vec<WorkspaceTransferLaunchTask>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceTransferLaunchTask {
    pub name: String,
    pub task_type: String,
    pub enabled: bool,
    pub command: String,
    pub args: Vec<String>,
    pub working_dir: String,
    pub timeout_ms: i64,
    pub continue_on_failure: bool,
    pub retry_policy: LaunchTaskRetryPolicy,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportWorkspaceListInput {
    pub destination_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportWorkspaceListResult {
    pub destination_path: String,
    pub workspace_count: usize,
    pub project_count: usize,
    pub launch_task_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewWorkspaceListImportInput {
    pub source_path: String,
    #[serde(default)]
    pub relocations: Vec<WorkspaceImportRelocation>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceImportRelocation {
    pub workspace_index: usize,
    pub project_index: usize,
    pub path: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceImportAction {
    Create,
    Skip,
    Update,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceImportSelection {
    pub workspace_index: usize,
    pub action: WorkspaceImportAction,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyWorkspaceListImportInput {
    pub source_path: String,
    pub expected_file_hash: String,
    pub selections: Vec<WorkspaceImportSelection>,
    #[serde(default)]
    pub relocations: Vec<WorkspaceImportRelocation>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceImportProjectStatus {
    Ready,
    Existing,
    Missing,
    Invalid,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceImportItemStatus {
    Ready,
    Existing,
    Blocked,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceImportProjectPreview {
    pub project_index: usize,
    pub name: String,
    pub source_path: String,
    pub resolved_path: String,
    pub status: WorkspaceImportProjectStatus,
    pub existing_workspace_id: Option<String>,
    pub existing_workspace_name: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceImportItemPreview {
    pub workspace_index: usize,
    pub name: String,
    pub suggested_name: String,
    pub status: WorkspaceImportItemStatus,
    pub recommended_action: WorkspaceImportAction,
    pub can_create: bool,
    pub can_update: bool,
    pub existing_workspace_id: Option<String>,
    pub existing_workspace_name: Option<String>,
    pub projects: Vec<WorkspaceImportProjectPreview>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceImportPreviewSummary {
    pub total_workspace_count: usize,
    pub ready_workspace_count: usize,
    pub existing_workspace_count: usize,
    pub blocked_workspace_count: usize,
    pub missing_project_count: usize,
    pub invalid_project_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceImportPreview {
    pub source_path: String,
    pub file_hash: String,
    pub schema_version: u32,
    pub summary: WorkspaceImportPreviewSummary,
    pub items: Vec<WorkspaceImportItemPreview>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceImportApplyResult {
    pub created_workspace_count: usize,
    pub updated_workspace_count: usize,
    pub skipped_workspace_count: usize,
    pub imported_project_count: usize,
    pub imported_launch_task_count: usize,
    pub warnings: Vec<String>,
}

pub fn validate_workspace_transfer_document(
    mut document: WorkspaceTransferDocument,
) -> AppResult<WorkspaceTransferDocument> {
    if document.format != WORKSPACE_TRANSFER_FORMAT {
        return Err(AppError::new(
            "WORKSPACE_TRANSFER_FORMAT_UNSUPPORTED",
            "这不是 Bexo Studio 工作区列表文件",
        )
        .with_detail("format", document.format));
    }
    if document.schema_version != WORKSPACE_TRANSFER_SCHEMA_VERSION {
        return Err(AppError::new(
            "WORKSPACE_TRANSFER_VERSION_UNSUPPORTED",
            "工作区列表文件版本不受当前 Bexo Studio 支持",
        )
        .with_detail("schemaVersion", document.schema_version.to_string())
        .with_detail(
            "supportedSchemaVersion",
            WORKSPACE_TRANSFER_SCHEMA_VERSION.to_string(),
        ));
    }

    document.app_version = normalize_required_text(
        "appVersion",
        &document.app_version,
        WORKSPACE_TRANSFER_MAX_APP_VERSION_CHARS,
    )?;
    DateTime::parse_from_rfc3339(document.exported_at.trim()).map_err(|error| {
        AppError::new(
            "WORKSPACE_TRANSFER_JSON_INVALID",
            "exportedAt 必须是合法的 RFC 3339 时间",
        )
        .with_detail("reason", error.to_string())
    })?;
    document.exported_at = document.exported_at.trim().to_string();

    if document.workspaces.len() > WORKSPACE_TRANSFER_MAX_WORKSPACES {
        return Err(limit_error(
            "workspaces",
            document.workspaces.len(),
            WORKSPACE_TRANSFER_MAX_WORKSPACES,
        ));
    }

    for (workspace_index, workspace) in document.workspaces.iter_mut().enumerate() {
        workspace.name = normalize_required_text("workspace.name", &workspace.name, 80)
            .map_err(|error| error.with_detail("workspaceIndex", workspace_index.to_string()))?;
        workspace.description = normalize_workspace_description(workspace.description.take())
            .map_err(|error| error.with_detail("workspaceIndex", workspace_index.to_string()))?;
        workspace.icon = normalize_optional_text(
            "workspace.icon",
            workspace.icon.take(),
            WORKSPACE_TRANSFER_MAX_ICON_CHARS,
        )
        .map_err(|error| error.with_detail("workspaceIndex", workspace_index.to_string()))?;
        workspace.color = parse_color_or_none(workspace.color.take())
            .map_err(|error| error.with_detail("workspaceIndex", workspace_index.to_string()))?;
        validate_sort_order("workspace.sortOrder", workspace.sort_order)
            .map_err(|error| error.with_detail("workspaceIndex", workspace_index.to_string()))?;

        if workspace.projects.len() > WORKSPACE_TRANSFER_MAX_PROJECTS_PER_WORKSPACE {
            return Err(limit_error(
                "workspace.projects",
                workspace.projects.len(),
                WORKSPACE_TRANSFER_MAX_PROJECTS_PER_WORKSPACE,
            )
            .with_detail("workspaceIndex", workspace_index.to_string()));
        }

        for (project_index, project) in workspace.projects.iter_mut().enumerate() {
            let with_indexes = |error: AppError| {
                error
                    .with_detail("workspaceIndex", workspace_index.to_string())
                    .with_detail("projectIndex", project_index.to_string())
            };
            project.name =
                normalize_required_text("project.name", &project.name, 80).map_err(with_indexes)?;
            project.path = normalize_absolute_path_text("project.path", &project.path)
                .map_err(with_indexes)?;
            project.platform = normalize_required_text("project.platform", &project.platform, 40)
                .map_err(with_indexes)?;
            project.terminal_type =
                normalize_required_text("project.terminalType", &project.terminal_type, 40)
                    .map_err(with_indexes)?;
            project.ide_type = normalize_optional_text(
                "project.ideType",
                project.ide_type.take(),
                WORKSPACE_TRANSFER_MAX_IDE_TYPE_CHARS,
            )
            .map_err(with_indexes)?;
            project.codex_profile_name = normalize_optional_text(
                "project.codexProfileName",
                project.codex_profile_name.take(),
                80,
            )
            .map_err(with_indexes)?;
            validate_sort_order("project.sortOrder", project.sort_order).map_err(with_indexes)?;

            if project.launch_tasks.len() > WORKSPACE_TRANSFER_MAX_TASKS_PER_PROJECT {
                return Err(limit_error(
                    "project.launchTasks",
                    project.launch_tasks.len(),
                    WORKSPACE_TRANSFER_MAX_TASKS_PER_PROJECT,
                )
                .with_detail("workspaceIndex", workspace_index.to_string())
                .with_detail("projectIndex", project_index.to_string()));
            }

            for (task_index, task) in project.launch_tasks.iter_mut().enumerate() {
                let with_task_indexes = |error: AppError| {
                    error
                        .with_detail("workspaceIndex", workspace_index.to_string())
                        .with_detail("projectIndex", project_index.to_string())
                        .with_detail("taskIndex", task_index.to_string())
                };
                task.name = normalize_required_text("launchTask.name", &task.name, 100)
                    .map_err(with_task_indexes)?;
                task.task_type =
                    validate_launch_task_type(&task.task_type).map_err(with_task_indexes)?;
                task.command = validate_transfer_task_command(&task.task_type, &task.command)
                    .map_err(with_task_indexes)?;
                task.args = validate_launch_task_args(std::mem::take(&mut task.args))
                    .map_err(with_task_indexes)?;
                task.working_dir =
                    validate_transfer_working_dir(&task.working_dir).map_err(with_task_indexes)?;
                task.timeout_ms = validate_launch_task_timeout(Some(task.timeout_ms))
                    .map_err(with_task_indexes)?;
                task.retry_policy =
                    validate_launch_task_retry_policy(Some(task.retry_policy.clone()))
                        .map_err(with_task_indexes)?;
                validate_sort_order("launchTask.sortOrder", task.sort_order)
                    .map_err(with_task_indexes)?;
                if task.task_type == "open_path" && !task.args.is_empty() {
                    return Err(with_task_indexes(AppError::new(
                        "WORKSPACE_TRANSFER_JSON_INVALID",
                        "open_path 启动任务不能包含 args",
                    )));
                }
            }
        }
    }

    Ok(document)
}

fn validate_transfer_task_command(task_type: &str, value: &str) -> AppResult<String> {
    let normalized = normalize_required_text("launchTask.command", value, 512)?;
    match task_type {
        "open_path" => normalize_absolute_path_text("launchTask.command", &normalized),
        "ide" if !matches!(normalized.as_str(), "vscode" | "jetbrains") => Err(AppError::new(
            "WORKSPACE_TRANSFER_JSON_INVALID",
            "ide 启动任务的 command 必须是 vscode 或 jetbrains",
        )
        .with_detail("command", normalized)),
        "codex"
            if !matches!(
                normalized.as_str(),
                "inherit_profile" | "terminal_only" | "run_codex" | "resume_last"
            ) =>
        {
            Err(AppError::new(
                "WORKSPACE_TRANSFER_JSON_INVALID",
                "codex 启动任务的 command 不受支持",
            )
            .with_detail("command", normalized))
        }
        _ => Ok(normalized),
    }
}

fn validate_transfer_working_dir(value: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    normalize_absolute_path_text("launchTask.workingDir", trimmed)
}

fn normalize_absolute_path_text(field: &str, value: &str) -> AppResult<String> {
    let normalized = normalize_required_text(field, value, WORKSPACE_TRANSFER_MAX_PATH_CHARS)?;
    if !Path::new(&normalized).is_absolute() {
        return Err(AppError::new(
            "WORKSPACE_TRANSFER_JSON_INVALID",
            format!("{field} 必须是绝对路径"),
        )
        .with_detail("field", field)
        .with_detail("path", normalized));
    }
    Ok(normalized)
}

fn normalize_optional_text(
    field: &str,
    value: Option<String>,
    max_chars: usize,
) -> AppResult<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    normalize_text(field, trimmed, max_chars).map(Some)
}

fn normalize_required_text(field: &str, value: &str, max_chars: usize) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::new(
            "WORKSPACE_TRANSFER_JSON_INVALID",
            format!("{field} 不能为空"),
        )
        .with_detail("field", field));
    }
    normalize_text(field, trimmed, max_chars)
}

fn normalize_text(field: &str, value: &str, max_chars: usize) -> AppResult<String> {
    if value.chars().any(char::is_control) {
        return Err(AppError::new(
            "WORKSPACE_TRANSFER_JSON_INVALID",
            format!("{field} 不能包含控制字符"),
        )
        .with_detail("field", field));
    }
    let character_count = value.chars().count();
    if character_count > max_chars {
        return Err(AppError::new(
            "WORKSPACE_TRANSFER_JSON_INVALID",
            format!("{field} 超过最大长度"),
        )
        .with_detail("field", field)
        .with_detail("count", character_count.to_string())
        .with_detail("limit", max_chars.to_string()));
    }
    Ok(value.to_string())
}

fn validate_sort_order(field: &str, value: i64) -> AppResult<()> {
    if !(-WORKSPACE_TRANSFER_MAX_SORT_ORDER..=WORKSPACE_TRANSFER_MAX_SORT_ORDER).contains(&value) {
        return Err(AppError::new(
            "WORKSPACE_TRANSFER_JSON_INVALID",
            format!("{field} 超出允许范围"),
        )
        .with_detail("field", field)
        .with_detail("value", value.to_string()));
    }
    Ok(())
}

fn limit_error(field: &str, count: usize, limit: usize) -> AppError {
    AppError::new(
        "WORKSPACE_TRANSFER_LIMIT_EXCEEDED",
        "工作区列表文件包含过多数据",
    )
    .with_detail("field", field)
    .with_detail("count", count.to_string())
    .with_detail("limit", limit.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_document() -> WorkspaceTransferDocument {
        WorkspaceTransferDocument {
            format: WORKSPACE_TRANSFER_FORMAT.to_string(),
            schema_version: WORKSPACE_TRANSFER_SCHEMA_VERSION,
            app_version: "0.1.16".to_string(),
            exported_at: "2026-09-02T00:00:00Z".to_string(),
            workspaces: vec![WorkspaceTransferWorkspace {
                name: "Project".to_string(),
                description: Some(" 编程项目 ".to_string()),
                icon: None,
                color: Some("#aabbcc".to_string()),
                sort_order: 0,
                is_default: true,
                is_archived: false,
                pinned: true,
                projects: vec![WorkspaceTransferProject {
                    name: "Project".to_string(),
                    path: if cfg!(windows) {
                        r"C:\work\project".to_string()
                    } else {
                        "/work/project".to_string()
                    },
                    platform: "windows".to_string(),
                    terminal_type: "windows_terminal".to_string(),
                    ide_type: Some("vscode".to_string()),
                    codex_profile_name: None,
                    open_terminal: true,
                    open_ide: false,
                    auto_resume_codex: false,
                    sort_order: 0,
                    launch_tasks: Vec::new(),
                }],
            }],
        }
    }

    #[test]
    fn validates_and_normalizes_a_supported_document() {
        let document = validate_workspace_transfer_document(valid_document()).unwrap();
        assert_eq!(
            document.workspaces[0].description.as_deref(),
            Some("编程项目")
        );
        assert_eq!(document.workspaces[0].color.as_deref(), Some("#AABBCC"));
    }

    #[test]
    fn rejects_unknown_format_and_future_schema() {
        let mut document = valid_document();
        document.format = "other".to_string();
        assert_eq!(
            validate_workspace_transfer_document(document)
                .unwrap_err()
                .code,
            "WORKSPACE_TRANSFER_FORMAT_UNSUPPORTED"
        );

        let mut document = valid_document();
        document.schema_version = WORKSPACE_TRANSFER_SCHEMA_VERSION + 1;
        assert_eq!(
            validate_workspace_transfer_document(document)
                .unwrap_err()
                .code,
            "WORKSPACE_TRANSFER_VERSION_UNSUPPORTED"
        );
    }

    #[test]
    fn rejects_control_characters_and_limits() {
        let mut document = valid_document();
        document.workspaces[0].name = "bad\u{0007}".to_string();
        assert_eq!(
            validate_workspace_transfer_document(document)
                .unwrap_err()
                .code,
            "WORKSPACE_TRANSFER_JSON_INVALID"
        );

        let mut document = valid_document();
        document.workspaces = (0..=WORKSPACE_TRANSFER_MAX_WORKSPACES)
            .map(|_| valid_document().workspaces.remove(0))
            .collect();
        assert_eq!(
            validate_workspace_transfer_document(document)
                .unwrap_err()
                .code,
            "WORKSPACE_TRANSFER_LIMIT_EXCEEDED"
        );
    }
}
