use std::path::Path;

use chrono::Utc;
use rusqlite::{params, Connection, Error as SqlError, ErrorCode, OptionalExtension, Transaction};
use uuid::Uuid;

use crate::{
    domain::{
        parse_color_or_none, require_non_empty, validate_optional_uuid, DeleteResult,
        ProjectRecord, UpsertWorkspaceInput, WorkspaceRecord,
    },
    error::{AppError, AppResult},
    persistence::list_all_launch_tasks,
};

fn map_workspace_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorkspaceRecord> {
    Ok(WorkspaceRecord {
        id: row.get("id")?,
        name: row.get("name")?,
        description: row.get("description")?,
        icon: row.get("icon")?,
        color: row.get("color")?,
        sort_order: row.get("sort_order")?,
        is_default: row.get::<_, i64>("is_default")? != 0,
        is_archived: row.get::<_, i64>("is_archived")? != 0,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        projects: Vec::new(),
    })
}

fn map_project_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectRecord> {
    Ok(ProjectRecord {
        id: row.get("id")?,
        workspace_id: row.get("workspace_id")?,
        name: row.get("name")?,
        path: row.get("path")?,
        platform: row.get("platform")?,
        terminal_type: row.get("terminal_type")?,
        ide_type: row.get("ide_type")?,
        codex_profile_id: row.get("codex_profile_id")?,
        open_terminal: row.get::<_, i64>("open_terminal")? != 0,
        open_ide: row.get::<_, i64>("open_ide")? != 0,
        auto_resume_codex: row.get::<_, i64>("auto_resume_codex")? != 0,
        sort_order: row.get("sort_order")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        launch_tasks: Vec::new(),
    })
}

pub fn list_workspaces(connection: &Connection) -> AppResult<Vec<WorkspaceRecord>> {
    let mut workspaces_statement = connection
        .prepare(
            "SELECT id, name, description, icon, color, sort_order, is_default, is_archived, created_at, updated_at
             FROM workspaces
             ORDER BY sort_order ASC, is_default DESC, updated_at DESC",
        )
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to prepare workspace query")
                .with_detail("reason", error.to_string())
        })?;

    let workspace_iter = workspaces_statement
        .query_map([], map_workspace_from_row)
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to query workspaces")
                .with_detail("reason", error.to_string())
        })?;

    let mut workspaces = Vec::new();
    for workspace in workspace_iter {
        workspaces.push(workspace.map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to map workspace row")
                .with_detail("reason", error.to_string())
        })?);
    }

    let mut projects_statement = connection
        .prepare(
            "SELECT id, workspace_id, name, path, platform, terminal_type, ide_type, codex_profile_id,
                    open_terminal, open_ide, auto_resume_codex, sort_order, created_at, updated_at
             FROM projects
             ORDER BY sort_order ASC, updated_at DESC",
        )
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to prepare project query")
                .with_detail("reason", error.to_string())
        })?;

    let project_iter = projects_statement
        .query_map([], map_project_from_row)
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to query projects")
                .with_detail("reason", error.to_string())
        })?;

    for project in project_iter {
        let project = project.map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to map project row")
                .with_detail("reason", error.to_string())
        })?;
        if let Some(workspace) = workspaces
            .iter_mut()
            .find(|item| item.id == project.workspace_id)
        {
            workspace.projects.push(project);
        }
    }

    let launch_tasks = list_all_launch_tasks(connection)?;
    for launch_task in launch_tasks {
        for workspace in &mut workspaces {
            if let Some(project) = workspace
                .projects
                .iter_mut()
                .find(|project| project.id == launch_task.project_id)
            {
                project.launch_tasks.push(launch_task.clone());
                break;
            }
        }
    }

    Ok(workspaces)
}

pub fn get_workspace_primary_project_path(
    connection: &Connection,
    workspace_id: String,
) -> AppResult<String> {
    let workspace_id = validate_optional_uuid("workspaceId", Some(workspace_id))?
        .ok_or_else(|| AppError::validation("workspaceId is required"))?;

    let workspace_row = connection
        .query_row(
            "SELECT p.path
             FROM workspaces w
             LEFT JOIN projects p ON p.workspace_id = w.id
             WHERE w.id = ?1
             ORDER BY
               CASE
                 WHEN p.path IS NULL OR TRIM(p.path) = '' THEN 1
                 ELSE 0
               END ASC,
               p.sort_order ASC,
               p.updated_at DESC
             LIMIT 1",
            [workspace_id.as_str()],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to query workspace project path")
                .with_detail("workspaceId", workspace_id.clone())
                .with_detail("reason", error.to_string())
        })?;

    let Some(path) = workspace_row else {
        return Err(
            AppError::new("WORKSPACE_NOT_FOUND", "workspace was not found")
                .with_detail("workspaceId", workspace_id),
        );
    };

    let trimmed_path = path
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::new(
                "INVALID_WORKSPACE_PATH",
                "workspace does not contain a registered project path",
            )
            .with_detail("workspaceId", workspace_id.clone())
        })?;

    Ok(trimmed_path)
}

pub fn upsert_workspace(
    connection: &mut Connection,
    input: UpsertWorkspaceInput,
) -> AppResult<WorkspaceRecord> {
    let id = validate_optional_uuid("id", input.id)?.unwrap_or_else(|| Uuid::new_v4().to_string());
    let name = require_non_empty("name", &input.name, 80)?;
    let description = input
        .description
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let icon = input
        .icon
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let color = parse_color_or_none(input.color)?;
    let sort_order = input.sort_order.unwrap_or(0);
    let is_default = input.is_default.unwrap_or(false);
    let is_archived = input.is_archived.unwrap_or(false);
    let timestamp = Utc::now().to_rfc3339();

    let transaction = connection.transaction().map_err(|error| {
        AppError::new("DB_WRITE_FAILED", "failed to open workspace transaction")
            .with_detail("reason", error.to_string())
    })?;

    let existing_created_at: Option<String> = transaction
        .query_row(
            "SELECT created_at FROM workspaces WHERE id = ?1",
            [id.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to query existing workspace")
                .with_detail("reason", error.to_string())
        })?;

    if is_default {
        transaction
            .execute(
                "UPDATE workspaces SET is_default = 0 WHERE id <> ?1",
                [id.as_str()],
            )
            .map_err(|error| {
                AppError::new("DB_WRITE_FAILED", "failed to reset default workspace")
                    .with_detail("reason", error.to_string())
            })?;
    }

    let created_at = existing_created_at.unwrap_or_else(|| timestamp.clone());
    let result = transaction.execute(
        "INSERT INTO workspaces
         (id, name, description, icon, color, sort_order, is_default, is_archived, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(id) DO UPDATE SET
           name = excluded.name,
           description = excluded.description,
           icon = excluded.icon,
           color = excluded.color,
           sort_order = excluded.sort_order,
           is_default = excluded.is_default,
           is_archived = excluded.is_archived,
           updated_at = excluded.updated_at",
        params![
            id,
            name,
            description,
            icon,
            color,
            sort_order,
            if is_default { 1 } else { 0 },
            if is_archived { 1 } else { 0 },
            created_at,
            timestamp
        ],
    );

    if let Err(error) = result {
        return Err(map_workspace_write_error(error));
    }

    transaction.commit().map_err(|error| {
        AppError::new("DB_WRITE_FAILED", "failed to commit workspace transaction")
            .with_detail("reason", error.to_string())
    })?;

    connection
        .query_row(
            "SELECT id, name, description, icon, color, sort_order, is_default, is_archived, created_at, updated_at
             FROM workspaces WHERE id = ?1",
            [id],
            map_workspace_from_row,
        )
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to load saved workspace")
                .with_detail("reason", error.to_string())
        })
}

pub fn delete_workspace(connection: &mut Connection, id: String) -> AppResult<DeleteResult> {
    let id = validate_optional_uuid("id", Some(id))?
        .ok_or_else(|| AppError::validation("id is required"))?;

    let project_count: i64 = connection
        .query_row(
            "SELECT COUNT(1) FROM projects WHERE workspace_id = ?1",
            [id.as_str()],
            |row| row.get(0),
        )
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to count workspace projects")
                .with_detail("reason", error.to_string())
        })?;

    if project_count > 0 {
        return Err(AppError::new(
            "WORKSPACE_DELETE_BLOCKED",
            "workspace has projects and cannot be deleted yet",
        )
        .with_detail("workspaceId", id.clone())
        .with_detail("projectCount", project_count.to_string()));
    }

    let affected_rows = connection
        .execute("DELETE FROM workspaces WHERE id = ?1", [id.as_str()])
        .map_err(|error| {
            AppError::new("DB_WRITE_FAILED", "failed to delete workspace")
                .with_detail("reason", error.to_string())
        })?;

    if affected_rows == 0 {
        return Err(
            AppError::new("WORKSPACE_NOT_FOUND", "workspace was not found")
                .with_detail("workspaceId", id.clone()),
        );
    }

    Ok(DeleteResult { id })
}

pub fn register_workspace_folder(
    connection: &mut Connection,
    path: String,
) -> AppResult<WorkspaceRecord> {
    let directory_path = crate::domain::ensure_absolute_directory(&path, "INVALID_WORKSPACE_PATH")?;
    let workspace_label = derive_workspace_label(&directory_path);
    let project_label = workspace_label.clone();
    let timestamp = Utc::now().to_rfc3339();
    let workspace_id = Uuid::new_v4().to_string();
    let project_id = Uuid::new_v4().to_string();

    let transaction = connection.transaction().map_err(|error| {
        AppError::new(
            "DB_WRITE_FAILED",
            "failed to open register workspace transaction",
        )
        .with_detail("reason", error.to_string())
    })?;

    let existing_workspace_id: Option<String> = transaction
        .query_row(
            "SELECT workspace_id FROM projects WHERE LOWER(path) = LOWER(?1) LIMIT 1",
            [directory_path.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| {
            AppError::new(
                "DB_READ_FAILED",
                "failed to query existing workspace registration by path",
            )
            .with_detail("reason", error.to_string())
        })?;

    if let Some(existing_workspace_id) = existing_workspace_id {
        return Err(AppError::new(
            "WORKSPACE_PATH_ALREADY_REGISTERED",
            "folder is already registered as a workspace",
        )
        .with_detail("workspaceId", existing_workspace_id)
        .with_detail("path", directory_path));
    }

    let workspace_name = allocate_workspace_name(&transaction, &workspace_label)?;
    let workspace_count: i64 = transaction
        .query_row("SELECT COUNT(1) FROM workspaces", [], |row| row.get(0))
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to count existing workspaces")
                .with_detail("reason", error.to_string())
        })?;

    transaction
        .execute(
            "INSERT INTO workspaces
             (id, name, description, icon, color, sort_order, is_default, is_archived, created_at, updated_at)
             VALUES (?1, ?2, NULL, NULL, NULL, ?3, ?4, 0, ?5, ?6)",
            params![
                workspace_id,
                workspace_name,
                workspace_count,
                if workspace_count == 0 { 1 } else { 0 },
                timestamp,
                timestamp
            ],
        )
        .map_err(map_workspace_write_error)?;

    transaction
        .execute(
            "INSERT INTO projects
             (id, workspace_id, name, path, platform, terminal_type, ide_type, codex_profile_id,
              open_terminal, open_ide, auto_resume_codex, sort_order, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'windows', 'windows_terminal', NULL, NULL, 0, 0, 0, 0, ?5, ?6)",
            params![project_id, workspace_id, project_label, directory_path, timestamp, timestamp],
        )
        .map_err(|error| {
            AppError::new("DB_WRITE_FAILED", "failed to save default workspace project")
                .with_detail("reason", error.to_string())
        })?;

    transaction.commit().map_err(|error| {
        AppError::new(
            "DB_WRITE_FAILED",
            "failed to commit register workspace transaction",
        )
        .with_detail("reason", error.to_string())
    })?;

    list_workspaces(connection)?
        .into_iter()
        .find(|workspace| workspace.id == workspace_id)
        .ok_or_else(|| {
            AppError::new(
                "DB_READ_FAILED",
                "failed to load registered workspace after commit",
            )
            .with_detail("workspaceId", workspace_id)
        })
}

pub fn remove_workspace_registration(
    connection: &mut Connection,
    id: String,
) -> AppResult<DeleteResult> {
    let id = validate_optional_uuid("id", Some(id))?
        .ok_or_else(|| AppError::validation("id is required"))?;

    let transaction = connection.transaction().map_err(|error| {
        AppError::new(
            "DB_WRITE_FAILED",
            "failed to open remove workspace transaction",
        )
        .with_detail("reason", error.to_string())
    })?;

    let was_default: Option<bool> = transaction
        .query_row(
            "SELECT is_default FROM workspaces WHERE id = ?1",
            [id.as_str()],
            |row| Ok(row.get::<_, i64>(0)? != 0),
        )
        .optional()
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to query workspace before removal")
                .with_detail("reason", error.to_string())
        })?;

    if was_default.is_none() {
        return Err(
            AppError::new("WORKSPACE_NOT_FOUND", "workspace was not found")
                .with_detail("workspaceId", id),
        );
    }

    transaction
        .execute(
            "DELETE FROM projects WHERE workspace_id = ?1",
            [id.as_str()],
        )
        .map_err(|error| {
            AppError::new("DB_WRITE_FAILED", "failed to delete workspace projects")
                .with_detail("reason", error.to_string())
        })?;

    let affected_rows = transaction
        .execute("DELETE FROM workspaces WHERE id = ?1", [id.as_str()])
        .map_err(|error| {
            AppError::new("DB_WRITE_FAILED", "failed to remove workspace registration")
                .with_detail("reason", error.to_string())
        })?;

    if affected_rows == 0 {
        return Err(
            AppError::new("WORKSPACE_NOT_FOUND", "workspace was not found")
                .with_detail("workspaceId", id),
        );
    }

    if was_default == Some(true) {
        if let Some(next_workspace_id) = transaction
            .query_row(
                "SELECT id FROM workspaces ORDER BY updated_at DESC LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| {
                AppError::new(
                    "DB_READ_FAILED",
                    "failed to query next default workspace candidate",
                )
                .with_detail("reason", error.to_string())
            })?
        {
            transaction
                .execute(
                    "UPDATE workspaces SET is_default = 1 WHERE id = ?1",
                    [next_workspace_id.as_str()],
                )
                .map_err(|error| {
                    AppError::new(
                        "DB_WRITE_FAILED",
                        "failed to promote replacement default workspace",
                    )
                    .with_detail("reason", error.to_string())
                })?;
        }
    }

    transaction.commit().map_err(|error| {
        AppError::new(
            "DB_WRITE_FAILED",
            "failed to commit remove workspace transaction",
        )
        .with_detail("reason", error.to_string())
    })?;

    Ok(DeleteResult { id })
}

fn map_workspace_write_error(error: SqlError) -> AppError {
    match error {
        SqlError::SqliteFailure(result, _) if result.code == ErrorCode::ConstraintViolation => {
            AppError::new("DUPLICATE_NAME", "workspace name already exists")
        }
        other => AppError::new("DB_WRITE_FAILED", "failed to save workspace")
            .with_detail("reason", other.to_string()),
    }
}

fn derive_workspace_label(path: &str) -> String {
    let trimmed = path.trim_end_matches(['\\', '/']);
    let candidate = Path::new(trimmed)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| trimmed.to_string());

    if candidate.trim().is_empty() {
        "Workspace".to_string()
    } else {
        candidate
    }
}

fn allocate_workspace_name(transaction: &Transaction<'_>, base_name: &str) -> AppResult<String> {
    let mut statement = transaction
        .prepare("SELECT name FROM workspaces")
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to prepare workspace name query")
                .with_detail("reason", error.to_string())
        })?;

    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to query workspace names")
                .with_detail("reason", error.to_string())
        })?;

    let mut existing_names = Vec::new();
    for row in rows {
        existing_names.push(row.map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to map workspace name row")
                .with_detail("reason", error.to_string())
        })?);
    }

    if !contains_workspace_name(&existing_names, base_name) {
        return Ok(base_name.to_string());
    }

    for index in 2..=999 {
        let candidate = format!("{base_name} ({index})");
        if !contains_workspace_name(&existing_names, &candidate) {
            return Ok(candidate);
        }
    }

    Err(AppError::new(
        "WORKSPACE_NAME_EXHAUSTED",
        "failed to allocate a unique workspace name",
    )
    .with_detail("baseName", base_name.to_string()))
}

fn contains_workspace_name(existing_names: &[String], candidate: &str) -> bool {
    existing_names
        .iter()
        .any(|name| name.eq_ignore_ascii_case(candidate))
}
