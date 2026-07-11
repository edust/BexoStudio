use chrono::Utc;
use rusqlite::{params, Connection, Error as SqlError, ErrorCode, OptionalExtension};
use uuid::Uuid;

use crate::{
    domain::{
        ensure_absolute_path, require_non_empty, validate_codex_auth_json,
        validate_codex_config_toml, validate_optional_uuid, CodexAuthProfileRecord,
        CodexAuthQuotaResult, DeleteResult, UpsertCodexAuthProfileInput,
    },
    error::{AppError, AppResult},
};

fn map_codex_auth_profile_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<CodexAuthProfileRecord> {
    let last_quota_json: Option<String> = row.get("last_quota_json")?;
    let last_quota = match last_quota_json {
        Some(value) => Some(
            serde_json::from_str::<CodexAuthQuotaResult>(&value).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?,
        ),
        None => None,
    };

    Ok(CodexAuthProfileRecord {
        id: row.get("id")?,
        name: row.get("name")?,
        description: row.get("description")?,
        codex_home: row.get("codex_home")?,
        auth_json: row.get("auth_json")?,
        config_toml: row.get("config_toml")?,
        is_active: row.get::<_, i64>("is_active")? != 0,
        last_quota,
        last_quota_checked_at: row.get("last_quota_checked_at")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

const SELECT_CODEX_AUTH_PROFILE: &str = "SELECT id, name, description, codex_home, auth_json,
        config_toml, is_active, last_quota_json, last_quota_checked_at, created_at, updated_at
    FROM codex_auth_profiles";

pub fn list_codex_auth_profiles(connection: &Connection) -> AppResult<Vec<CodexAuthProfileRecord>> {
    let mut statement = connection
        .prepare(&format!(
            "{SELECT_CODEX_AUTH_PROFILE} ORDER BY is_active DESC, updated_at DESC"
        ))
        .map_err(|error| {
            AppError::new(
                "DB_READ_FAILED",
                "failed to prepare codex auth profile query",
            )
            .with_detail("reason", error.to_string())
        })?;

    let rows = statement
        .query_map([], map_codex_auth_profile_from_row)
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to query codex auth profiles")
                .with_detail("reason", error.to_string())
        })?;

    let mut profiles = Vec::new();
    for row in rows {
        profiles.push(row.map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to map codex auth profile row")
                .with_detail("reason", error.to_string())
        })?);
    }

    Ok(profiles)
}

pub fn get_codex_auth_profile(
    connection: &Connection,
    id: &str,
) -> AppResult<Option<CodexAuthProfileRecord>> {
    connection
        .query_row(
            &format!("{SELECT_CODEX_AUTH_PROFILE} WHERE id = ?1"),
            [id],
            map_codex_auth_profile_from_row,
        )
        .optional()
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to load codex auth profile")
                .with_detail("reason", error.to_string())
        })
}

pub fn get_active_codex_auth_profile_id(connection: &Connection) -> AppResult<Option<String>> {
    connection
        .query_row(
            "SELECT id FROM codex_auth_profiles WHERE is_active = 1 ORDER BY updated_at DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to load active codex auth profile")
                .with_detail("reason", error.to_string())
        })
}

pub fn restore_codex_auth_active_profile(
    connection: &mut Connection,
    active_id: Option<String>,
) -> AppResult<()> {
    let transaction = connection.savepoint().map_err(|error| {
        AppError::new(
            "DB_WRITE_FAILED",
            "failed to open codex auth active rollback transaction",
        )
        .with_detail("reason", error.to_string())
    })?;

    if let Some(active_id) = active_id.as_deref() {
        let exists = transaction
            .query_row(
                "SELECT 1 FROM codex_auth_profiles WHERE id = ?1 LIMIT 1",
                [active_id],
                |_| Ok(()),
            )
            .optional()
            .map_err(|error| {
                AppError::new(
                    "DB_READ_FAILED",
                    "failed to validate previous active profile",
                )
                .with_detail("reason", error.to_string())
            })?;
        if exists.is_none() {
            return Err(AppError::new(
                "CODEX_AUTH_ROLLBACK_FAILED",
                "previous active Codex Auth profile no longer exists",
            )
            .with_detail("id", active_id.to_string()));
        }
    }

    transaction
        .execute("UPDATE codex_auth_profiles SET is_active = 0", [])
        .map_err(|error| {
            AppError::new("DB_WRITE_FAILED", "failed to reset Codex Auth active state")
                .with_detail("reason", error.to_string())
        })?;
    if let Some(active_id) = active_id {
        transaction
            .execute(
                "UPDATE codex_auth_profiles SET is_active = 1 WHERE id = ?1",
                [active_id],
            )
            .map_err(|error| {
                AppError::new(
                    "DB_WRITE_FAILED",
                    "failed to restore previous Codex Auth active profile",
                )
                .with_detail("reason", error.to_string())
            })?;
    }

    transaction.commit().map_err(|error| {
        AppError::new(
            "DB_WRITE_FAILED",
            "failed to commit Codex Auth active rollback",
        )
        .with_detail("reason", error.to_string())
    })
}

pub fn upsert_codex_auth_profile(
    connection: &mut Connection,
    input: UpsertCodexAuthProfileInput,
) -> AppResult<CodexAuthProfileRecord> {
    let id = validate_optional_uuid("id", input.id)?.unwrap_or_else(|| Uuid::new_v4().to_string());
    let name = require_non_empty("name", &input.name, 100)?;
    let description = input
        .description
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let codex_home = ensure_absolute_path(&input.codex_home, "INVALID_CODEX_HOME")?;
    let auth_json = input.auth_json;
    let config_toml = input.config_toml;

    validate_codex_auth_json(&auth_json)?;
    validate_codex_config_toml(&config_toml)?;

    let timestamp = Utc::now().to_rfc3339();
    let transaction = connection.savepoint().map_err(|error| {
        AppError::new(
            "DB_WRITE_FAILED",
            "failed to open codex auth profile transaction",
        )
        .with_detail("reason", error.to_string())
    })?;

    let existing: Option<(String, i64, Option<String>, Option<String>)> = transaction
        .query_row(
            "SELECT created_at, is_active, last_quota_json, last_quota_checked_at
             FROM codex_auth_profiles WHERE id = ?1",
            [id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(|error| {
            AppError::new(
                "DB_READ_FAILED",
                "failed to query existing codex auth profile",
            )
            .with_detail("reason", error.to_string())
        })?;

    let (created_at, is_active, last_quota_json, last_quota_checked_at) =
        existing.unwrap_or((timestamp.clone(), 0, None, None));

    let result = transaction.execute(
        "INSERT INTO codex_auth_profiles
         (id, name, description, codex_home, auth_json, config_toml, is_active,
          last_quota_json, last_quota_checked_at, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(id) DO UPDATE SET
           name = excluded.name,
           description = excluded.description,
           codex_home = excluded.codex_home,
           auth_json = excluded.auth_json,
           config_toml = excluded.config_toml,
           updated_at = excluded.updated_at",
        params![
            id,
            name,
            description,
            codex_home,
            auth_json,
            config_toml,
            is_active,
            last_quota_json,
            last_quota_checked_at,
            created_at,
            timestamp,
        ],
    );

    if let Err(error) = result {
        return Err(map_codex_auth_profile_write_error(error));
    }

    transaction.commit().map_err(|error| {
        AppError::new(
            "DB_WRITE_FAILED",
            "failed to commit codex auth profile transaction",
        )
        .with_detail("reason", error.to_string())
    })?;

    get_codex_auth_profile(connection, &id)?
        .ok_or_else(|| AppError::new("DB_READ_FAILED", "failed to load saved codex auth profile"))
}

pub fn delete_codex_auth_profile(
    connection: &mut Connection,
    id: String,
) -> AppResult<DeleteResult> {
    let trimmed = require_non_empty("id", &id, 80)?;
    let affected = connection
        .execute(
            "DELETE FROM codex_auth_profiles WHERE id = ?1",
            [trimmed.as_str()],
        )
        .map_err(|error| {
            AppError::new("DB_WRITE_FAILED", "failed to delete codex auth profile")
                .with_detail("reason", error.to_string())
        })?;
    if affected == 0 {
        return Err(AppError::new(
            "CODEX_AUTH_PROFILE_NOT_FOUND",
            "codex auth profile was not found",
        )
        .with_detail("id", trimmed.clone()));
    }
    Ok(DeleteResult { id: trimmed })
}

pub fn mark_codex_auth_profile_active(
    connection: &mut Connection,
    id: String,
) -> AppResult<CodexAuthProfileRecord> {
    let timestamp = Utc::now().to_rfc3339();
    let transaction = connection.savepoint().map_err(|error| {
        AppError::new(
            "DB_WRITE_FAILED",
            "failed to open codex auth active profile transaction",
        )
        .with_detail("reason", error.to_string())
    })?;

    let existing = transaction
        .query_row(
            "SELECT 1 FROM codex_auth_profiles WHERE id = ?1 LIMIT 1",
            [id.as_str()],
            |_| Ok(()),
        )
        .optional()
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to check codex auth profile")
                .with_detail("reason", error.to_string())
        })?;
    if existing.is_none() {
        return Err(AppError::new(
            "CODEX_AUTH_PROFILE_NOT_FOUND",
            "codex auth profile was not found",
        ));
    }

    transaction
        .execute("UPDATE codex_auth_profiles SET is_active = 0", [])
        .map_err(|error| {
            AppError::new(
                "DB_WRITE_FAILED",
                "failed to reset active codex auth profile",
            )
            .with_detail("reason", error.to_string())
        })?;
    transaction
        .execute(
            "UPDATE codex_auth_profiles
             SET is_active = 1, updated_at = ?2
             WHERE id = ?1",
            params![id, timestamp],
        )
        .map_err(|error| {
            AppError::new(
                "DB_WRITE_FAILED",
                "failed to mark active codex auth profile",
            )
            .with_detail("reason", error.to_string())
        })?;
    transaction.commit().map_err(|error| {
        AppError::new(
            "DB_WRITE_FAILED",
            "failed to commit active codex auth profile transaction",
        )
        .with_detail("reason", error.to_string())
    })?;

    get_codex_auth_profile(connection, &id)?
        .ok_or_else(|| AppError::new("DB_READ_FAILED", "failed to load active codex auth profile"))
}

pub fn update_codex_auth_profile_quota(
    connection: &mut Connection,
    id: String,
    quota: CodexAuthQuotaResult,
) -> AppResult<CodexAuthProfileRecord> {
    let quota_json = serde_json::to_string(&quota).map_err(|error| {
        AppError::new("DB_WRITE_FAILED", "failed to serialize codex auth quota")
            .with_detail("reason", error.to_string())
    })?;
    let checked_at = quota.queried_at.clone();
    connection
        .execute(
            "UPDATE codex_auth_profiles
             SET last_quota_json = ?2, last_quota_checked_at = ?3, updated_at = ?3
             WHERE id = ?1",
            params![id, quota_json, checked_at],
        )
        .map_err(|error| {
            AppError::new("DB_WRITE_FAILED", "failed to update codex auth quota")
                .with_detail("reason", error.to_string())
        })?;

    get_codex_auth_profile(connection, &id)?.ok_or_else(|| {
        AppError::new(
            "CODEX_AUTH_PROFILE_NOT_FOUND",
            "codex auth profile was not found",
        )
    })
}

fn map_codex_auth_profile_write_error(error: SqlError) -> AppError {
    match error {
        SqlError::SqliteFailure(result, _) if result.code == ErrorCode::ConstraintViolation => {
            AppError::new("DUPLICATE_NAME", "codex auth profile name already exists")
        }
        other => AppError::new("DB_WRITE_FAILED", "failed to save codex auth profile")
            .with_detail("reason", other.to_string()),
    }
}
