use chrono::Utc;
use rusqlite::{params, Connection, Error as SqlError, ErrorCode, OptionalExtension};
use uuid::Uuid;

use crate::{
    domain::{
        ensure_absolute_directory, parse_json_string_list, require_non_empty,
        validate_optional_uuid, CodexProfileRecord, UpsertCodexProfileInput,
    },
    error::{AppError, AppResult},
};

fn map_codex_profile_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CodexProfileRecord> {
    let default_args_json: String = row.get("default_args_json")?;
    let default_args = parse_json_string_list(&default_args_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })?;

    Ok(CodexProfileRecord {
        id: row.get("id")?,
        name: row.get("name")?,
        description: row.get("description")?,
        codex_home: row.get("codex_home")?,
        startup_mode: row.get("startup_mode")?,
        resume_strategy: row.get("resume_strategy")?,
        default_args,
        is_default: row.get::<_, i64>("is_default")? != 0,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn list_codex_profiles(connection: &Connection) -> AppResult<Vec<CodexProfileRecord>> {
    let mut statement = connection
        .prepare(
            "SELECT id, name, description, codex_home, startup_mode, resume_strategy,
                    default_args_json, is_default, created_at, updated_at
             FROM codex_profiles
             ORDER BY is_default DESC, updated_at DESC",
        )
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to prepare codex profile query")
                .with_detail("reason", error.to_string())
        })?;

    let rows = statement
        .query_map([], map_codex_profile_from_row)
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to query codex profiles")
                .with_detail("reason", error.to_string())
        })?;

    let mut profiles = Vec::new();
    for row in rows {
        profiles.push(row.map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to map codex profile row")
                .with_detail("reason", error.to_string())
        })?);
    }

    Ok(profiles)
}

pub fn upsert_codex_profile(
    connection: &mut Connection,
    input: UpsertCodexProfileInput,
) -> AppResult<CodexProfileRecord> {
    let id = validate_optional_uuid("id", input.id)?.unwrap_or_else(|| Uuid::new_v4().to_string());
    let name = require_non_empty("name", &input.name, 80)?;
    let description = input
        .description
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let codex_home = ensure_absolute_directory(&input.codex_home, "INVALID_CODEX_HOME")?;
    let startup_mode = require_non_empty("startupMode", &input.startup_mode, 40)?;
    let resume_strategy = require_non_empty("resumeStrategy", &input.resume_strategy, 40)?;
    let default_args_json = serde_json::to_string(&input.default_args).map_err(|error| {
        AppError::new("VALIDATION_ERROR", "defaultArgs must be serializable")
            .with_detail("reason", error.to_string())
    })?;
    let is_default = input.is_default.unwrap_or(false);
    let timestamp = Utc::now().to_rfc3339();

    let transaction = connection.transaction().map_err(|error| {
        AppError::new(
            "DB_WRITE_FAILED",
            "failed to open codex profile transaction",
        )
        .with_detail("reason", error.to_string())
    })?;

    let existing_created_at: Option<String> = transaction
        .query_row(
            "SELECT created_at FROM codex_profiles WHERE id = ?1",
            [id.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to query existing codex profile")
                .with_detail("reason", error.to_string())
        })?;

    if is_default {
        transaction
            .execute(
                "UPDATE codex_profiles SET is_default = 0 WHERE id <> ?1",
                [id.as_str()],
            )
            .map_err(|error| {
                AppError::new("DB_WRITE_FAILED", "failed to reset default codex profile")
                    .with_detail("reason", error.to_string())
            })?;
    }

    let created_at = existing_created_at.unwrap_or_else(|| timestamp.clone());
    let result = transaction.execute(
        "INSERT INTO codex_profiles
         (id, name, description, codex_home, startup_mode, resume_strategy, default_args_json,
          is_default, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(id) DO UPDATE SET
           name = excluded.name,
           description = excluded.description,
           codex_home = excluded.codex_home,
           startup_mode = excluded.startup_mode,
           resume_strategy = excluded.resume_strategy,
           default_args_json = excluded.default_args_json,
           is_default = excluded.is_default,
           updated_at = excluded.updated_at",
        params![
            id,
            name,
            description,
            codex_home,
            startup_mode,
            resume_strategy,
            default_args_json,
            if is_default { 1 } else { 0 },
            created_at,
            timestamp,
        ],
    );

    if let Err(error) = result {
        return Err(map_codex_profile_write_error(error));
    }

    transaction.commit().map_err(|error| {
        AppError::new(
            "DB_WRITE_FAILED",
            "failed to commit codex profile transaction",
        )
        .with_detail("reason", error.to_string())
    })?;

    connection
        .query_row(
            "SELECT id, name, description, codex_home, startup_mode, resume_strategy,
                    default_args_json, is_default, created_at, updated_at
             FROM codex_profiles WHERE id = ?1",
            [id],
            map_codex_profile_from_row,
        )
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to load saved codex profile")
                .with_detail("reason", error.to_string())
        })
}

fn map_codex_profile_write_error(error: SqlError) -> AppError {
    match error {
        SqlError::SqliteFailure(result, _) if result.code == ErrorCode::ConstraintViolation => {
            AppError::new("DUPLICATE_NAME", "codex profile name already exists")
        }
        other => AppError::new("DB_WRITE_FAILED", "failed to save codex profile")
            .with_detail("reason", other.to_string()),
    }
}
