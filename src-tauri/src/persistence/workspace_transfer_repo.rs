use std::collections::{HashMap, HashSet};

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::{
    domain::{
        ensure_absolute_directory, validate_launch_task_args, validate_launch_task_command,
        validate_launch_task_retry_policy, validate_launch_task_timeout, validate_launch_task_type,
        validate_launch_task_working_dir, WorkspaceImportAction, WorkspaceImportApplyResult,
        WorkspaceTransferLaunchTask, WorkspaceTransferProject, WorkspaceTransferWorkspace,
    },
    error::{AppError, AppResult},
};

use super::list_workspaces;

#[derive(Debug, Clone)]
pub struct WorkspaceImportPlanEntry {
    pub workspace_index: usize,
    pub action: WorkspaceImportAction,
    pub workspace: WorkspaceTransferWorkspace,
    pub existing_workspace_id: Option<String>,
    pub existing_project_ids: Vec<Option<String>>,
}

#[derive(Debug, Clone, Default)]
pub struct WorkspaceImportPersistenceOutcome {
    pub result: WorkspaceImportApplyResult,
    pub pinned_workspace_ids_to_add: Vec<String>,
    pub pinned_workspace_ids_to_remove: Vec<String>,
}

pub fn collect_workspace_transfer_workspaces(
    connection: &Connection,
    pinned_workspace_ids: &HashSet<String>,
) -> AppResult<Vec<WorkspaceTransferWorkspace>> {
    let profile_names = list_codex_profile_names_by_id(connection)?;
    let workspaces = list_workspaces(connection)?;

    Ok(workspaces
        .into_iter()
        .map(|workspace| WorkspaceTransferWorkspace {
            name: workspace.name,
            description: workspace.description,
            icon: workspace.icon,
            color: workspace.color,
            sort_order: workspace.sort_order,
            is_default: workspace.is_default,
            is_archived: workspace.is_archived,
            pinned: pinned_workspace_ids.contains(&workspace.id),
            projects: workspace
                .projects
                .into_iter()
                .map(|project| WorkspaceTransferProject {
                    name: project.name,
                    path: project.path,
                    platform: project.platform,
                    terminal_type: project.terminal_type,
                    ide_type: project.ide_type,
                    codex_profile_name: project
                        .codex_profile_id
                        .as_ref()
                        .and_then(|profile_id| profile_names.get(profile_id).cloned()),
                    open_terminal: project.open_terminal,
                    open_ide: project.open_ide,
                    auto_resume_codex: project.auto_resume_codex,
                    sort_order: project.sort_order,
                    launch_tasks: project
                        .launch_tasks
                        .into_iter()
                        .map(|task| WorkspaceTransferLaunchTask {
                            name: task.name,
                            task_type: task.task_type,
                            enabled: task.enabled,
                            command: task.command,
                            args: task.args,
                            working_dir: task.working_dir,
                            timeout_ms: task.timeout_ms,
                            continue_on_failure: task.continue_on_failure,
                            retry_policy: task.retry_policy,
                            sort_order: task.sort_order,
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect())
}

pub fn apply_workspace_import_plan(
    connection: &mut Connection,
    entries: Vec<WorkspaceImportPlanEntry>,
) -> AppResult<WorkspaceImportPersistenceOutcome> {
    let mut outcome = WorkspaceImportPersistenceOutcome::default();
    let profile_names = list_codex_profile_names(connection)?;
    let mut workspace_names = list_workspace_names(connection)?;
    let mut next_workspace_sort_order = next_workspace_sort_order(connection)?;
    let has_existing_default = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM workspaces WHERE is_default = 1 LIMIT 1)",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| {
            AppError::new("WORKSPACE_TRANSFER_DB_FAILED", "读取当前默认工作区失败")
                .with_detail("reason", error.to_string())
        })?
        != 0;
    let mut preferred_default_workspace_id = None;
    let mut fallback_default_workspace_id = None;
    let mut warned_profiles = HashSet::new();

    for entry in entries {
        match entry.action {
            WorkspaceImportAction::Skip => {
                outcome.result.skipped_workspace_count += 1;
                continue;
            }
            WorkspaceImportAction::Create => {
                if entry.existing_workspace_id.is_some()
                    || entry.existing_project_ids.iter().any(Option::is_some)
                {
                    return Err(selection_error(
                        entry.workspace_index,
                        "新建动作不能引用现有工作区或项目",
                    ));
                }
                if entry.workspace.projects.is_empty() {
                    return Err(selection_error(
                        entry.workspace_index,
                        "不能导入不包含项目目录的工作区",
                    ));
                }

                let workspace_id = Uuid::new_v4().to_string();
                let workspace_name = unique_workspace_name(&entry.workspace.name, &workspace_names);
                workspace_names.insert(workspace_name.to_lowercase());
                let timestamp = Utc::now().to_rfc3339();
                connection
                    .execute(
                        "INSERT INTO workspaces
                         (id, name, description, icon, color, sort_order, is_default, is_archived, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, ?8, ?8)",
                        params![
                            workspace_id,
                            workspace_name,
                            entry.workspace.description,
                            entry.workspace.icon,
                            entry.workspace.color,
                            next_workspace_sort_order,
                            if entry.workspace.is_archived { 1 } else { 0 },
                            timestamp,
                        ],
                    )
                    .map_err(|error| import_db_error(
                        "创建导入工作区失败",
                        entry.workspace_index,
                        error,
                    ))?;
                next_workspace_sort_order = next_workspace_sort_order.saturating_add(1);

                for (project_index, project) in entry.workspace.projects.iter().enumerate() {
                    let counts = insert_imported_project(
                        connection,
                        &workspace_id,
                        None,
                        project,
                        &profile_names,
                        entry.workspace_index,
                        project_index,
                        &mut outcome.result.warnings,
                        &mut warned_profiles,
                    )?;
                    outcome.result.imported_project_count += 1;
                    outcome.result.imported_launch_task_count += counts;
                }

                if entry.workspace.pinned {
                    outcome
                        .pinned_workspace_ids_to_add
                        .push(workspace_id.clone());
                }
                fallback_default_workspace_id.get_or_insert_with(|| workspace_id.clone());
                if entry.workspace.is_default && preferred_default_workspace_id.is_none() {
                    preferred_default_workspace_id = Some(workspace_id.clone());
                }
                outcome.result.created_workspace_count += 1;
            }
            WorkspaceImportAction::Update => {
                let workspace_id = entry.existing_workspace_id.clone().ok_or_else(|| {
                    selection_error(entry.workspace_index, "更新动作必须对应一个现有工作区")
                })?;
                if entry.workspace.projects.len() != entry.existing_project_ids.len() {
                    return Err(selection_error(
                        entry.workspace_index,
                        "项目预检结果与导入文件不一致",
                    ));
                }
                let timestamp = Utc::now().to_rfc3339();
                let affected = connection
                    .execute(
                        "UPDATE workspaces
                         SET description = ?2, icon = ?3, color = ?4, is_archived = ?5, updated_at = ?6
                         WHERE id = ?1",
                        params![
                            workspace_id,
                            entry.workspace.description,
                            entry.workspace.icon,
                            entry.workspace.color,
                            if entry.workspace.is_archived { 1 } else { 0 },
                            timestamp,
                        ],
                    )
                    .map_err(|error| import_db_error(
                        "更新现有工作区失败",
                        entry.workspace_index,
                        error,
                    ))?;
                if affected != 1 {
                    return Err(selection_error(
                        entry.workspace_index,
                        "预检时匹配的工作区已不存在，请重新预检",
                    ));
                }

                for (project_index, (project, existing_project_id)) in entry
                    .workspace
                    .projects
                    .iter()
                    .zip(entry.existing_project_ids.iter())
                    .enumerate()
                {
                    let counts = insert_imported_project(
                        connection,
                        &workspace_id,
                        existing_project_id.as_deref(),
                        project,
                        &profile_names,
                        entry.workspace_index,
                        project_index,
                        &mut outcome.result.warnings,
                        &mut warned_profiles,
                    )?;
                    outcome.result.imported_project_count += 1;
                    outcome.result.imported_launch_task_count += counts;
                }

                if entry.workspace.pinned {
                    outcome
                        .pinned_workspace_ids_to_add
                        .push(workspace_id.clone());
                } else {
                    outcome
                        .pinned_workspace_ids_to_remove
                        .push(workspace_id.clone());
                }
                fallback_default_workspace_id.get_or_insert_with(|| workspace_id.clone());
                if entry.workspace.is_default && preferred_default_workspace_id.is_none() {
                    preferred_default_workspace_id = Some(workspace_id.clone());
                }
                outcome.result.updated_workspace_count += 1;
            }
        }
    }

    if !has_existing_default {
        if let Some(default_workspace_id) =
            preferred_default_workspace_id.or(fallback_default_workspace_id)
        {
            connection
                .execute(
                    "UPDATE workspaces SET is_default = CASE WHEN id = ?1 THEN 1 ELSE 0 END",
                    [default_workspace_id.as_str()],
                )
                .map_err(|error| {
                    AppError::new("WORKSPACE_TRANSFER_DB_FAILED", "设置导入后的默认工作区失败")
                        .with_detail("reason", error.to_string())
                })?;
        }
    }

    Ok(outcome)
}

#[allow(clippy::too_many_arguments)]
fn insert_imported_project(
    connection: &mut Connection,
    workspace_id: &str,
    existing_project_id: Option<&str>,
    project: &WorkspaceTransferProject,
    profile_names: &[(String, String)],
    workspace_index: usize,
    project_index: usize,
    warnings: &mut Vec<String>,
    warned_profiles: &mut HashSet<String>,
) -> AppResult<usize> {
    let project_path = ensure_absolute_directory(&project.path, "WORKSPACE_TRANSFER_PATH_MISSING")
        .map_err(|error| {
            error
                .with_detail("workspaceIndex", workspace_index.to_string())
                .with_detail("projectIndex", project_index.to_string())
        })?;
    let profile_id = project.codex_profile_name.as_ref().and_then(|name| {
        resolve_codex_profile_id(profile_names, name).or_else(|| {
            let warning_key = name.to_lowercase();
            if warned_profiles.insert(warning_key) {
                warnings.push(format!(
                    "未找到 Codex Profile“{name}”，相关项目已解除 Profile 绑定。"
                ));
            }
            None
        })
    });
    let project_id = existing_project_id
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let timestamp = Utc::now().to_rfc3339();

    if let Some(existing_project_id) = existing_project_id {
        let belongs_to_workspace = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1 AND workspace_id = ?2)",
                params![existing_project_id, workspace_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| import_db_error("核对现有项目归属失败", workspace_index, error))?
            != 0;
        if !belongs_to_workspace {
            return Err(
                selection_error(workspace_index, "预检时匹配的项目已发生变化，请重新预检")
                    .with_detail("projectIndex", project_index.to_string()),
            );
        }

        connection
            .execute(
                "UPDATE projects
                 SET name = ?3, path = ?4, platform = ?5, terminal_type = ?6, ide_type = ?7,
                     codex_profile_id = ?8, open_terminal = ?9, open_ide = ?10,
                     auto_resume_codex = ?11, sort_order = ?12, updated_at = ?13
                 WHERE id = ?1 AND workspace_id = ?2",
                params![
                    project_id,
                    workspace_id,
                    project.name,
                    project_path,
                    project.platform,
                    project.terminal_type,
                    project.ide_type,
                    profile_id,
                    if project.open_terminal { 1 } else { 0 },
                    if project.open_ide { 1 } else { 0 },
                    if project.auto_resume_codex { 1 } else { 0 },
                    project.sort_order,
                    timestamp,
                ],
            )
            .map_err(|error| import_db_error("更新现有项目配置失败", workspace_index, error))?;
        connection
            .execute(
                "DELETE FROM launch_tasks WHERE project_id = ?1",
                [project_id.as_str()],
            )
            .map_err(|error| import_db_error("替换现有项目启动任务失败", workspace_index, error))?;
    } else {
        connection
            .execute(
                "INSERT INTO projects
                 (id, workspace_id, name, path, platform, terminal_type, ide_type, codex_profile_id,
                  open_terminal, open_ide, auto_resume_codex, sort_order, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13)",
                params![
                    project_id,
                    workspace_id,
                    project.name,
                    project_path,
                    project.platform,
                    project.terminal_type,
                    project.ide_type,
                    profile_id,
                    if project.open_terminal { 1 } else { 0 },
                    if project.open_ide { 1 } else { 0 },
                    if project.auto_resume_codex { 1 } else { 0 },
                    project.sort_order,
                    timestamp,
                ],
            )
            .map_err(|error| import_db_error("创建导入项目失败", workspace_index, error))?;
    }

    for (task_index, task) in project.launch_tasks.iter().enumerate() {
        insert_imported_launch_task(
            connection,
            &project_id,
            task,
            workspace_index,
            project_index,
            task_index,
        )?;
    }
    Ok(project.launch_tasks.len())
}

fn insert_imported_launch_task(
    connection: &mut Connection,
    project_id: &str,
    task: &WorkspaceTransferLaunchTask,
    workspace_index: usize,
    project_index: usize,
    task_index: usize,
) -> AppResult<()> {
    let task_type = validate_launch_task_type(&task.task_type)?;
    let command = validate_launch_task_command(&task_type, &task.command)?;
    let args = validate_launch_task_args(task.args.clone())?;
    let working_dir = validate_launch_task_working_dir(Some(task.working_dir.clone()))?;
    let timeout_ms = validate_launch_task_timeout(Some(task.timeout_ms))?;
    let retry_policy = validate_launch_task_retry_policy(Some(task.retry_policy.clone()))?;
    let args_json = serde_json::to_string(&args).map_err(|error| {
        AppError::new("WORKSPACE_TRANSFER_DB_FAILED", "序列化导入启动参数失败")
            .with_detail("reason", error.to_string())
    })?;
    let retry_policy_json = serde_json::to_string(&retry_policy).map_err(|error| {
        AppError::new("WORKSPACE_TRANSFER_DB_FAILED", "序列化导入重试策略失败")
            .with_detail("reason", error.to_string())
    })?;
    connection
        .execute(
            "INSERT INTO launch_tasks
             (id, project_id, name, task_type, enabled, command, args_json, working_dir,
              timeout_ms, continue_on_failure, retry_policy_json, sort_order)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                Uuid::new_v4().to_string(),
                project_id,
                task.name,
                task_type,
                if task.enabled { 1 } else { 0 },
                command,
                args_json,
                working_dir,
                timeout_ms,
                if task.continue_on_failure { 1 } else { 0 },
                retry_policy_json,
                task.sort_order,
            ],
        )
        .map_err(|error| {
            import_db_error("创建导入启动任务失败", workspace_index, error)
                .with_detail("projectIndex", project_index.to_string())
                .with_detail("taskIndex", task_index.to_string())
        })?;
    Ok(())
}

fn list_codex_profile_names_by_id(connection: &Connection) -> AppResult<HashMap<String, String>> {
    let mut statement = connection
        .prepare("SELECT id, name FROM codex_profiles ORDER BY name ASC")
        .map_err(|error| {
            AppError::new(
                "WORKSPACE_TRANSFER_DB_FAILED",
                "读取 Codex Profile 名称失败",
            )
            .with_detail("reason", error.to_string())
        })?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| {
            AppError::new(
                "WORKSPACE_TRANSFER_DB_FAILED",
                "查询 Codex Profile 名称失败",
            )
            .with_detail("reason", error.to_string())
        })?;
    let mut profiles = HashMap::new();
    for row in rows {
        let (id, name) = row.map_err(|error| {
            AppError::new(
                "WORKSPACE_TRANSFER_DB_FAILED",
                "解析 Codex Profile 名称失败",
            )
            .with_detail("reason", error.to_string())
        })?;
        profiles.insert(id, name);
    }
    Ok(profiles)
}

fn list_codex_profile_names(connection: &Connection) -> AppResult<Vec<(String, String)>> {
    let mut statement = connection
        .prepare("SELECT name, id FROM codex_profiles ORDER BY name COLLATE NOCASE ASC, id ASC")
        .map_err(|error| {
            AppError::new(
                "WORKSPACE_TRANSFER_DB_FAILED",
                "读取 Codex Profile 映射失败",
            )
            .with_detail("reason", error.to_string())
        })?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| {
            AppError::new(
                "WORKSPACE_TRANSFER_DB_FAILED",
                "查询 Codex Profile 映射失败",
            )
            .with_detail("reason", error.to_string())
        })?;
    let mut profiles = Vec::new();
    for row in rows {
        profiles.push(row.map_err(|error| {
            AppError::new(
                "WORKSPACE_TRANSFER_DB_FAILED",
                "解析 Codex Profile 映射失败",
            )
            .with_detail("reason", error.to_string())
        })?);
    }
    Ok(profiles)
}

fn resolve_codex_profile_id(profiles: &[(String, String)], name: &str) -> Option<String> {
    profiles
        .iter()
        .find(|(profile_name, _)| profile_name == name)
        .or_else(|| {
            profiles
                .iter()
                .find(|(profile_name, _)| profile_name.eq_ignore_ascii_case(name))
        })
        .map(|(_, id)| id.clone())
}

fn list_workspace_names(connection: &Connection) -> AppResult<HashSet<String>> {
    let mut statement = connection
        .prepare("SELECT name FROM workspaces")
        .map_err(|error| {
            AppError::new("WORKSPACE_TRANSFER_DB_FAILED", "读取现有工作区名称失败")
                .with_detail("reason", error.to_string())
        })?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| {
            AppError::new("WORKSPACE_TRANSFER_DB_FAILED", "查询现有工作区名称失败")
                .with_detail("reason", error.to_string())
        })?;
    let mut names = HashSet::new();
    for row in rows {
        names.insert(
            row.map_err(|error| {
                AppError::new("WORKSPACE_TRANSFER_DB_FAILED", "解析现有工作区名称失败")
                    .with_detail("reason", error.to_string())
            })?
            .to_lowercase(),
        );
    }
    Ok(names)
}

fn next_workspace_sort_order(connection: &Connection) -> AppResult<i64> {
    connection
        .query_row("SELECT MAX(sort_order) FROM workspaces", [], |row| {
            row.get::<_, Option<i64>>(0)
        })
        .optional()
        .map_err(|error| {
            AppError::new("WORKSPACE_TRANSFER_DB_FAILED", "读取工作区排序位置失败")
                .with_detail("reason", error.to_string())
        })
        .map(|value| value.flatten().unwrap_or(-1).saturating_add(1))
}

pub(crate) fn unique_workspace_name(base: &str, occupied_names: &HashSet<String>) -> String {
    if !occupied_names.contains(&base.to_lowercase()) {
        return base.to_string();
    }
    for suffix_index in 1..=10_000 {
        let suffix = format!(" (导入 {suffix_index})");
        let max_base_chars = 80usize.saturating_sub(suffix.chars().count());
        let truncated_base = base.chars().take(max_base_chars).collect::<String>();
        let candidate = format!("{truncated_base}{suffix}");
        if !occupied_names.contains(&candidate.to_lowercase()) {
            return candidate;
        }
    }
    format!("导入工作区-{}", Uuid::new_v4())
}

pub fn unique_workspace_name_for_transfer(base: &str, occupied_names: &HashSet<String>) -> String {
    unique_workspace_name(base, occupied_names)
}

fn selection_error(workspace_index: usize, message: &str) -> AppError {
    AppError::new("WORKSPACE_TRANSFER_SELECTION_INVALID", message)
        .with_detail("workspaceIndex", workspace_index.to_string())
}

fn import_db_error(message: &str, workspace_index: usize, error: rusqlite::Error) -> AppError {
    AppError::new("WORKSPACE_TRANSFER_DB_FAILED", message)
        .with_detail("workspaceIndex", workspace_index.to_string())
        .with_detail("reason", error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::LaunchTaskRetryPolicy;
    use rusqlite::Connection;
    use std::fs;

    fn test_connection() -> Connection {
        let connection = Connection::open_in_memory().expect("open sqlite memory database");
        connection
            .execute_batch(crate::persistence::schema::SCHEMA)
            .expect("initialize schema");
        connection
    }

    fn test_workspace(path: String) -> WorkspaceTransferWorkspace {
        WorkspaceTransferWorkspace {
            name: "Imported".to_string(),
            description: Some("用途".to_string()),
            icon: None,
            color: None,
            sort_order: 0,
            is_default: true,
            is_archived: false,
            pinned: true,
            projects: vec![WorkspaceTransferProject {
                name: "Imported".to_string(),
                path: path.clone(),
                platform: "windows".to_string(),
                terminal_type: "windows_terminal".to_string(),
                ide_type: Some("vscode".to_string()),
                codex_profile_name: None,
                open_terminal: true,
                open_ide: true,
                auto_resume_codex: false,
                sort_order: 0,
                launch_tasks: vec![WorkspaceTransferLaunchTask {
                    name: "dev".to_string(),
                    task_type: "terminal_command".to_string(),
                    enabled: true,
                    command: "npm run dev".to_string(),
                    args: Vec::new(),
                    working_dir: path,
                    timeout_ms: 30_000,
                    continue_on_failure: false,
                    retry_policy: LaunchTaskRetryPolicy::default(),
                    sort_order: 0,
                }],
            }],
        }
    }

    #[test]
    fn creates_fresh_ids_and_preserves_launch_task_relationships() {
        let mut connection = test_connection();
        let directory =
            std::env::temp_dir().join(format!("bexo-workspace-import-{}", Uuid::new_v4()));
        fs::create_dir_all(&directory).expect("create import directory");
        connection.execute_batch("BEGIN IMMEDIATE").unwrap();
        let outcome = apply_workspace_import_plan(
            &mut connection,
            vec![WorkspaceImportPlanEntry {
                workspace_index: 0,
                action: WorkspaceImportAction::Create,
                workspace: test_workspace(directory.display().to_string()),
                existing_workspace_id: None,
                existing_project_ids: vec![None],
            }],
        )
        .unwrap();
        connection.execute_batch("COMMIT").unwrap();

        assert_eq!(outcome.result.created_workspace_count, 1);
        assert_eq!(outcome.result.imported_project_count, 1);
        assert_eq!(outcome.result.imported_launch_task_count, 1);
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM launch_tasks", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(outcome.pinned_workspace_ids_to_add.len(), 1);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn caller_transaction_can_roll_back_a_partially_applied_batch() {
        let mut connection = test_connection();
        let directory =
            std::env::temp_dir().join(format!("bexo-workspace-import-rollback-{}", Uuid::new_v4()));
        fs::create_dir_all(&directory).expect("create import directory");
        connection.execute_batch("BEGIN IMMEDIATE").unwrap();
        let result = apply_workspace_import_plan(
            &mut connection,
            vec![
                WorkspaceImportPlanEntry {
                    workspace_index: 0,
                    action: WorkspaceImportAction::Create,
                    workspace: test_workspace(directory.display().to_string()),
                    existing_workspace_id: None,
                    existing_project_ids: vec![None],
                },
                WorkspaceImportPlanEntry {
                    workspace_index: 1,
                    action: WorkspaceImportAction::Create,
                    workspace: test_workspace(directory.join("missing").display().to_string()),
                    existing_workspace_id: None,
                    existing_project_ids: vec![None],
                },
            ],
        );
        assert!(result.is_err());
        connection.execute_batch("ROLLBACK").unwrap();
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM workspaces", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn update_replaces_matched_configuration_and_preserves_unmatched_projects() {
        let mut connection = test_connection();
        let directory =
            std::env::temp_dir().join(format!("bexo-workspace-update-{}", Uuid::new_v4()));
        let unmatched_directory = directory.join("unmatched");
        fs::create_dir_all(&unmatched_directory).expect("create unmatched project directory");

        connection.execute_batch("BEGIN IMMEDIATE").unwrap();
        let created = apply_workspace_import_plan(
            &mut connection,
            vec![WorkspaceImportPlanEntry {
                workspace_index: 0,
                action: WorkspaceImportAction::Create,
                workspace: test_workspace(directory.display().to_string()),
                existing_workspace_id: None,
                existing_project_ids: vec![None],
            }],
        )
        .unwrap();
        connection.execute_batch("COMMIT").unwrap();
        let workspace_id = created.pinned_workspace_ids_to_add[0].clone();
        let project_id = connection
            .query_row(
                "SELECT id FROM projects WHERE workspace_id = ?1 ORDER BY sort_order ASC LIMIT 1",
                [workspace_id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .unwrap();
        let timestamp = Utc::now().to_rfc3339();
        connection
            .execute(
                "INSERT INTO projects
                 (id, workspace_id, name, path, platform, terminal_type, ide_type, codex_profile_id,
                  open_terminal, open_ide, auto_resume_codex, sort_order, created_at, updated_at)
                 VALUES (?1, ?2, 'Unmatched', ?3, 'windows', 'windows_terminal', NULL, NULL,
                         1, 0, 0, 1, ?4, ?4)",
                params![
                    Uuid::new_v4().to_string(),
                    workspace_id,
                    unmatched_directory.display().to_string(),
                    timestamp
                ],
            )
            .unwrap();

        let mut updated_workspace = test_workspace(directory.display().to_string());
        updated_workspace.description = Some("更新后的用途".to_string());
        updated_workspace.pinned = false;
        updated_workspace.projects[0].launch_tasks[0].command = "pnpm run dev".to_string();
        connection.execute_batch("BEGIN IMMEDIATE").unwrap();
        let updated = apply_workspace_import_plan(
            &mut connection,
            vec![WorkspaceImportPlanEntry {
                workspace_index: 0,
                action: WorkspaceImportAction::Update,
                workspace: updated_workspace,
                existing_workspace_id: Some(workspace_id.clone()),
                existing_project_ids: vec![Some(project_id.clone())],
            }],
        )
        .unwrap();
        connection.execute_batch("COMMIT").unwrap();

        assert_eq!(updated.result.updated_workspace_count, 1);
        assert_eq!(
            updated.pinned_workspace_ids_to_remove,
            vec![workspace_id.clone()]
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM projects WHERE workspace_id = ?1",
                    [workspace_id.as_str()],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            2
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT command FROM launch_tasks WHERE project_id = ?1",
                    [project_id.as_str()],
                    |row| row.get::<_, String>(0),
                )
                .unwrap(),
            "pnpm run dev"
        );

        connection.execute_batch("BEGIN IMMEDIATE").unwrap();
        let skipped = apply_workspace_import_plan(
            &mut connection,
            vec![WorkspaceImportPlanEntry {
                workspace_index: 0,
                action: WorkspaceImportAction::Skip,
                workspace: test_workspace(directory.display().to_string()),
                existing_workspace_id: Some(workspace_id),
                existing_project_ids: vec![Some(project_id)],
            }],
        )
        .unwrap();
        connection.execute_batch("COMMIT").unwrap();
        assert_eq!(skipped.result.skipped_workspace_count, 1);
        let _ = fs::remove_dir_all(directory);
    }
}
