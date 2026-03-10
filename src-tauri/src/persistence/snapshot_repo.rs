use chrono::Utc;
use rusqlite::{params, Connection, Error as SqlError, ErrorCode, OptionalExtension};
use uuid::Uuid;

use crate::{
    domain::{
        require_non_empty, validate_optional_uuid, CreateSnapshotInput, SnapshotPayload,
        SnapshotRecord, UpdateSnapshotInput,
    },
    error::{AppError, AppResult},
};

fn map_snapshot_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SnapshotRecord> {
    let payload_json: String = row.get("payload_json")?;
    let payload = serde_json::from_str::<SnapshotPayload>(&payload_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })?;

    Ok(SnapshotRecord {
        id: row.get("id")?,
        workspace_id: row.get("workspace_id")?,
        workspace_name: payload.workspace.name.clone(),
        name: row.get("name")?,
        description: row.get("description")?,
        project_count: payload.projects.len() as i64,
        payload,
        last_restore_at: row.get("last_restore_at")?,
        last_restore_status: row.get("last_restore_status")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn list_snapshots(
    connection: &Connection,
    workspace_id: Option<String>,
) -> AppResult<Vec<SnapshotRecord>> {
    let workspace_id = validate_optional_uuid("workspaceId", workspace_id)?;
    let query = "SELECT id, workspace_id, name, description, payload_json, last_restore_at, last_restore_status, created_at, updated_at
                 FROM snapshots
                 WHERE (?1 IS NULL OR workspace_id = ?1)
                 ORDER BY updated_at DESC";

    let mut statement = connection.prepare(query).map_err(|error| {
        AppError::new("DB_READ_FAILED", "failed to prepare snapshot query")
            .with_detail("reason", error.to_string())
    })?;

    let rows = statement
        .query_map([workspace_id.as_deref()], map_snapshot_from_row)
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to query snapshots")
                .with_detail("reason", error.to_string())
        })?;

    let mut snapshots = Vec::new();
    for row in rows {
        snapshots.push(row.map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to map snapshot row")
                .with_detail("reason", error.to_string())
        })?);
    }

    Ok(snapshots)
}

pub fn get_snapshot(connection: &Connection, id: String) -> AppResult<SnapshotRecord> {
    let id = validate_optional_uuid("snapshotId", Some(id))?
        .ok_or_else(|| AppError::validation("snapshotId is required"))?;

    connection
        .query_row(
            "SELECT id, workspace_id, name, description, payload_json, last_restore_at, last_restore_status, created_at, updated_at
             FROM snapshots
             WHERE id = ?1",
            [id.as_str()],
            map_snapshot_from_row,
        )
        .optional()
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to load snapshot")
                .with_detail("reason", error.to_string())
        })?
        .ok_or_else(|| AppError::new("SNAPSHOT_NOT_FOUND", "snapshot was not found").with_detail("snapshotId", id))
}

pub fn create_snapshot(
    connection: &mut Connection,
    input: CreateSnapshotInput,
    payload: SnapshotPayload,
) -> AppResult<SnapshotRecord> {
    let workspace_id = validate_optional_uuid("workspaceId", Some(input.workspace_id))?
        .ok_or_else(|| AppError::validation("workspaceId is required"))?;
    let name = require_non_empty("name", &input.name, 100)?;
    let description = input
        .description
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if let Some(value) = &description {
        if value.len() > 240 {
            return Err(AppError::validation("description exceeds 240 characters"));
        }
    }

    if payload.projects.is_empty() {
        return Err(AppError::new(
            "SNAPSHOT_SOURCE_EMPTY",
            "workspace has no projects to snapshot",
        )
        .with_detail("workspaceId", workspace_id));
    }

    let payload_json = serde_json::to_string(&payload).map_err(|error| {
        AppError::new("VALIDATION_ERROR", "snapshot payload serialization failed")
            .with_detail("reason", error.to_string())
    })?;

    let timestamp = Utc::now().to_rfc3339();
    let snapshot_id = Uuid::new_v4().to_string();

    let transaction = connection.transaction().map_err(|error| {
        AppError::new("DB_WRITE_FAILED", "failed to open snapshot transaction")
            .with_detail("reason", error.to_string())
    })?;

    let workspace_exists: Option<String> = transaction
        .query_row(
            "SELECT id FROM workspaces WHERE id = ?1",
            [workspace_id.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| {
            AppError::new(
                "DB_READ_FAILED",
                "failed to query snapshot workspace source",
            )
            .with_detail("reason", error.to_string())
        })?;

    if workspace_exists.is_none() {
        return Err(
            AppError::new("WORKSPACE_NOT_FOUND", "workspace was not found")
                .with_detail("workspaceId", workspace_id),
        );
    }

    let result = transaction.execute(
        "INSERT INTO snapshots
         (id, workspace_id, name, description, payload_json, last_restore_at, last_restore_status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, ?6, ?7)",
        params![
            snapshot_id,
            workspace_id,
            name,
            description,
            payload_json,
            timestamp,
            timestamp
        ],
    );

    if let Err(error) = result {
        return Err(map_snapshot_write_error(error));
    }

    transaction.commit().map_err(|error| {
        AppError::new("DB_WRITE_FAILED", "failed to commit snapshot transaction")
            .with_detail("reason", error.to_string())
    })?;

    get_snapshot(connection, snapshot_id)
}

pub fn update_snapshot(
    connection: &mut Connection,
    input: UpdateSnapshotInput,
) -> AppResult<SnapshotRecord> {
    let snapshot_id = validate_optional_uuid("snapshotId", Some(input.id))?
        .ok_or_else(|| AppError::validation("snapshotId is required"))?;
    let name = require_non_empty("name", &input.name, 100)?;
    let description = input
        .description
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if let Some(value) = &description {
        if value.len() > 240 {
            return Err(AppError::validation("description exceeds 240 characters"));
        }
    }

    let timestamp = Utc::now().to_rfc3339();
    let affected_rows = connection
        .execute(
            "UPDATE snapshots
             SET name = ?2, description = ?3, updated_at = ?4
             WHERE id = ?1",
            params![snapshot_id, name, description, timestamp],
        )
        .map_err(map_snapshot_write_error)?;

    if affected_rows == 0 {
        return Err(
            AppError::new("SNAPSHOT_NOT_FOUND", "snapshot was not found")
                .with_detail("snapshotId", snapshot_id),
        );
    }

    get_snapshot(connection, snapshot_id)
}

fn map_snapshot_write_error(error: SqlError) -> AppError {
    match error {
        SqlError::SqliteFailure(result, _) if result.code == ErrorCode::ConstraintViolation => {
            AppError::new(
                "DUPLICATE_NAME",
                "snapshot name already exists in this workspace",
            )
        }
        other => AppError::new("DB_WRITE_FAILED", "failed to save snapshot")
            .with_detail("reason", other.to_string()),
    }
}
