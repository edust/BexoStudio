use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::{
    domain::{
        ensure_absolute_directory, require_non_empty, validate_optional_uuid, ProjectRecord,
        UpsertProjectInput,
    },
    error::{AppError, AppResult},
};

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

pub fn upsert_project(
    connection: &mut Connection,
    input: UpsertProjectInput,
) -> AppResult<ProjectRecord> {
    let id = validate_optional_uuid("id", input.id)?.unwrap_or_else(|| Uuid::new_v4().to_string());
    let workspace_id = validate_optional_uuid("workspaceId", Some(input.workspace_id))?
        .ok_or_else(|| AppError::validation("workspaceId is required"))?;
    let name = require_non_empty("name", &input.name, 80)?;
    let path = ensure_absolute_directory(&input.path, "INVALID_PROJECT_PATH")?;
    let platform = require_non_empty("platform", &input.platform, 40)?;
    let terminal_type = require_non_empty("terminalType", &input.terminal_type, 40)?;
    let ide_type = input
        .ide_type
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let codex_profile_id = validate_optional_uuid("codexProfileId", input.codex_profile_id)?;
    let sort_order = input.sort_order.unwrap_or(0);
    let timestamp = Utc::now().to_rfc3339();

    let workspace_exists: bool = connection
        .query_row(
            "SELECT 1 FROM workspaces WHERE id = ?1 LIMIT 1",
            [workspace_id.as_str()],
            |_| Ok(true),
        )
        .optional()
        .map_err(|error| {
            AppError::new(
                "DB_READ_FAILED",
                "failed to query workspace before saving project",
            )
            .with_detail("reason", error.to_string())
        })?
        .unwrap_or(false);

    if !workspace_exists {
        return Err(
            AppError::new("WORKSPACE_NOT_FOUND", "workspace was not found")
                .with_detail("workspaceId", workspace_id),
        );
    }

    if let Some(profile_id) = codex_profile_id.as_ref() {
        let profile_exists: bool = connection
            .query_row(
                "SELECT 1 FROM codex_profiles WHERE id = ?1 LIMIT 1",
                [profile_id.as_str()],
                |_| Ok(true),
            )
            .optional()
            .map_err(|error| {
                AppError::new(
                    "DB_READ_FAILED",
                    "failed to query codex profile before saving project",
                )
                .with_detail("reason", error.to_string())
            })?
            .unwrap_or(false);

        if !profile_exists {
            return Err(
                AppError::new("CODEX_PROFILE_NOT_FOUND", "codex profile was not found")
                    .with_detail("codexProfileId", profile_id.clone()),
            );
        }
    }

    let transaction = connection.transaction().map_err(|error| {
        AppError::new("DB_WRITE_FAILED", "failed to open project transaction")
            .with_detail("reason", error.to_string())
    })?;

    let existing_created_at: Option<String> = transaction
        .query_row(
            "SELECT created_at FROM projects WHERE id = ?1",
            [id.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to query existing project")
                .with_detail("reason", error.to_string())
        })?;

    let created_at = existing_created_at.unwrap_or_else(|| timestamp.clone());
    transaction
        .execute(
            "INSERT INTO projects
             (id, workspace_id, name, path, platform, terminal_type, ide_type, codex_profile_id,
              open_terminal, open_ide, auto_resume_codex, sort_order, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(id) DO UPDATE SET
               workspace_id = excluded.workspace_id,
               name = excluded.name,
               path = excluded.path,
               platform = excluded.platform,
               terminal_type = excluded.terminal_type,
               ide_type = excluded.ide_type,
               codex_profile_id = excluded.codex_profile_id,
               open_terminal = excluded.open_terminal,
               open_ide = excluded.open_ide,
               auto_resume_codex = excluded.auto_resume_codex,
               sort_order = excluded.sort_order,
               updated_at = excluded.updated_at",
            params![
                id,
                workspace_id,
                name,
                path,
                platform,
                terminal_type,
                ide_type,
                codex_profile_id,
                if input.open_terminal { 1 } else { 0 },
                if input.open_ide { 1 } else { 0 },
                if input.auto_resume_codex { 1 } else { 0 },
                sort_order,
                created_at,
                timestamp,
            ],
        )
        .map_err(|error| {
            AppError::new("DB_WRITE_FAILED", "failed to save project")
                .with_detail("reason", error.to_string())
        })?;

    transaction.commit().map_err(|error| {
        AppError::new("DB_WRITE_FAILED", "failed to commit project transaction")
            .with_detail("reason", error.to_string())
    })?;

    connection
        .query_row(
            "SELECT id, workspace_id, name, path, platform, terminal_type, ide_type, codex_profile_id,
                    open_terminal, open_ide, auto_resume_codex, sort_order, created_at, updated_at
             FROM projects WHERE id = ?1",
            [id],
            map_project_from_row,
        )
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to load saved project")
                .with_detail("reason", error.to_string())
        })
}
