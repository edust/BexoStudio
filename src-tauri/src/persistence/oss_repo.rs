use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::{
    domain::{
        validate_account_id, validate_operation_id, validate_target_id, DeleteResult,
        OssAccountRecord, OssTargetRecord, OssTransferTaskRecord, UpsertOssAccountMetadataInput,
        UpsertOssTargetInput,
    },
    error::{AppError, AppResult},
};

fn map_account_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<OssAccountRecord> {
    Ok(OssAccountRecord {
        id: row.get("id")?,
        display_name: row.get("display_name")?,
        access_key_id_hint: row.get("access_key_id_hint")?,
        credential_ref: row.get("credential_ref")?,
        last_probe_status: row.get("last_probe_status")?,
        last_probe_error: row.get("last_probe_error")?,
        last_probe_at: row.get("last_probe_at")?,
        is_disabled: row.get::<_, i64>("is_disabled")? != 0,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn map_target_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<OssTargetRecord> {
    Ok(OssTargetRecord {
        id: row.get("id")?,
        account_id: row.get("account_id")?,
        display_name: row.get("display_name")?,
        bucket: row.get("bucket")?,
        region: row.get("region")?,
        endpoint: row.get("endpoint")?,
        prefix: row.get("prefix")?,
        is_default: row.get::<_, i64>("is_default")? != 0,
        sort_order: row.get("sort_order")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn map_transfer_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<OssTransferTaskRecord> {
    let bytes_completed = row.get::<_, i64>("bytes_completed")?;
    let total_bytes = row.get::<_, i64>("total_bytes")?;
    if bytes_completed < 0 || total_bytes < 0 {
        return Err(rusqlite::Error::InvalidColumnType(
            0,
            "bytes".into(),
            rusqlite::types::Type::Integer,
        ));
    }

    Ok(OssTransferTaskRecord {
        id: row.get("id")?,
        account_id: row.get("account_id")?,
        target_id: row.get("target_id")?,
        operation: row.get("operation")?,
        object_key: row.get("object_key")?,
        local_path: row.get("local_path")?,
        status: row.get("status")?,
        bytes_completed: bytes_completed as u64,
        total_bytes: total_bytes as u64,
        upload_id: row.get("upload_id")?,
        checkpoint_json: row.get("checkpoint_json")?,
        last_error_code: row.get("last_error_code")?,
        last_error_message: row.get("last_error_message")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn list_oss_accounts(connection: &Connection) -> AppResult<Vec<OssAccountRecord>> {
    let mut statement = connection
        .prepare(
            "SELECT id, display_name, access_key_id_hint, credential_ref,
                    last_probe_status, last_probe_error, last_probe_at,
                    is_disabled, created_at, updated_at
             FROM oss_accounts
             ORDER BY display_name COLLATE NOCASE ASC, created_at ASC",
        )
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to prepare OSS account query")
                .with_detail("reason", error.to_string())
        })?;

    let records = statement
        .query_map([], map_account_from_row)
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to query OSS accounts")
                .with_detail("reason", error.to_string())
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to map OSS account row")
                .with_detail("reason", error.to_string())
        })?;

    Ok(records)
}

pub fn get_oss_account(
    connection: &Connection,
    account_id: String,
) -> AppResult<Option<OssAccountRecord>> {
    let account_id = validate_account_id(account_id)?;
    connection
        .query_row(
            "SELECT id, display_name, access_key_id_hint, credential_ref,
                    last_probe_status, last_probe_error, last_probe_at,
                    is_disabled, created_at, updated_at
             FROM oss_accounts
             WHERE id = ?1",
            [account_id.as_str()],
            map_account_from_row,
        )
        .optional()
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to query OSS account")
                .with_detail("accountId", account_id)
                .with_detail("reason", error.to_string())
        })
}

pub fn upsert_oss_account(
    connection: &mut Connection,
    input: UpsertOssAccountMetadataInput,
) -> AppResult<OssAccountRecord> {
    let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let timestamp = Utc::now().to_rfc3339();
    let transaction = connection.savepoint().map_err(|error| {
        AppError::new("DB_WRITE_FAILED", "failed to open OSS account transaction")
            .with_detail("reason", error.to_string())
    })?;

    let existing_created_at: Option<String> = transaction
        .query_row(
            "SELECT created_at FROM oss_accounts WHERE id = ?1",
            [id.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to query existing OSS account")
                .with_detail("accountId", id.clone())
                .with_detail("reason", error.to_string())
        })?;

    let created_at = existing_created_at.unwrap_or_else(|| timestamp.clone());
    transaction
        .execute(
            "INSERT INTO oss_accounts
             (id, display_name, access_key_id_hint, credential_ref,
              last_probe_status, last_probe_error, last_probe_at,
              is_disabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'unknown', NULL, NULL, 0, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
               display_name = excluded.display_name,
               access_key_id_hint = excluded.access_key_id_hint,
               credential_ref = excluded.credential_ref,
               updated_at = excluded.updated_at",
            params![
                id,
                input.display_name,
                input.access_key_id_hint,
                input.credential_ref,
                created_at,
                timestamp
            ],
        )
        .map_err(map_oss_account_write_error)?;

    transaction.commit().map_err(|error| {
        AppError::new(
            "DB_WRITE_FAILED",
            "failed to commit OSS account transaction",
        )
        .with_detail("reason", error.to_string())
    })?;

    get_oss_account(connection, id)?
        .ok_or_else(|| AppError::new("DB_READ_FAILED", "failed to load saved OSS account"))
}

pub fn update_oss_account_probe(
    connection: &mut Connection,
    account_id: String,
    status: String,
    error_message: Option<String>,
) -> AppResult<OssAccountRecord> {
    let account_id = validate_account_id(account_id)?;
    let timestamp = Utc::now().to_rfc3339();
    let affected = connection
        .execute(
            "UPDATE oss_accounts
             SET last_probe_status = ?1, last_probe_error = ?2, last_probe_at = ?3, updated_at = ?3
             WHERE id = ?4",
            params![status, error_message, timestamp, account_id.as_str()],
        )
        .map_err(|error| {
            AppError::new("DB_WRITE_FAILED", "failed to update OSS probe status")
                .with_detail("accountId", account_id.clone())
                .with_detail("reason", error.to_string())
        })?;
    if affected != 1 {
        return Err(
            AppError::new("OSS_ACCOUNT_NOT_FOUND", "OSS account was not found")
                .with_detail("accountId", account_id),
        );
    }
    get_oss_account(connection, account_id)?
        .ok_or_else(|| AppError::new("DB_READ_FAILED", "failed to load OSS probe result"))
}

pub fn delete_oss_account(
    connection: &mut Connection,
    account_id: String,
) -> AppResult<DeleteResult> {
    let account_id = validate_account_id(account_id)?;
    ensure_oss_account_deletable(connection, account_id.clone())?;

    let affected = connection
        .execute(
            "DELETE FROM oss_accounts WHERE id = ?1",
            [account_id.as_str()],
        )
        .map_err(|error| {
            AppError::new("DB_WRITE_FAILED", "failed to delete OSS account")
                .with_detail("accountId", account_id.clone())
                .with_detail("reason", error.to_string())
        })?;
    if affected != 1 {
        return Err(
            AppError::new("OSS_ACCOUNT_NOT_FOUND", "OSS account was not found")
                .with_detail("accountId", account_id),
        );
    }
    Ok(DeleteResult { id: account_id })
}

pub fn ensure_oss_account_deletable(connection: &Connection, account_id: String) -> AppResult<()> {
    let account_id = validate_account_id(account_id)?;
    let active_count: i64 = connection
        .query_row(
            "SELECT COUNT(1) FROM oss_transfer_tasks
             WHERE account_id = ?1
               AND status IN ('queued', 'running', 'paused', 'failed', 'cancelling', 'needs_confirmation')",
            [account_id.as_str()],
            |row| row.get(0),
        )
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to inspect OSS transfer tasks")
                .with_detail("accountId", account_id.clone())
                .with_detail("reason", error.to_string())
        })?;
    if active_count > 0 {
        return Err(AppError::new(
            "OSS_ACCOUNT_HAS_ACTIVE_TRANSFERS",
            "OSS account has active transfers",
        )
        .with_detail("accountId", account_id)
        .with_detail("activeCount", active_count.to_string()));
    }
    Ok(())
}

pub fn list_oss_targets(
    connection: &Connection,
    account_id: String,
) -> AppResult<Vec<OssTargetRecord>> {
    let account_id = validate_account_id(account_id)?;
    let mut statement = connection
        .prepare(
            "SELECT id, account_id, display_name, bucket, region, endpoint, prefix,
                    is_default, sort_order, created_at, updated_at
             FROM oss_targets
             WHERE account_id = ?1
             ORDER BY is_default DESC, sort_order ASC, display_name COLLATE NOCASE ASC",
        )
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to prepare OSS target query")
                .with_detail("reason", error.to_string())
        })?;
    let records = statement
        .query_map([account_id.as_str()], map_target_from_row)
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to query OSS targets")
                .with_detail("accountId", account_id.clone())
                .with_detail("reason", error.to_string())
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to map OSS target row")
                .with_detail("reason", error.to_string())
        })?;
    Ok(records)
}

pub fn get_oss_target(
    connection: &Connection,
    target_id: String,
) -> AppResult<Option<OssTargetRecord>> {
    let target_id = validate_target_id(target_id)?;
    connection
        .query_row(
            "SELECT id, account_id, display_name, bucket, region, endpoint, prefix,
                    is_default, sort_order, created_at, updated_at
             FROM oss_targets
             WHERE id = ?1",
            [target_id.as_str()],
            map_target_from_row,
        )
        .optional()
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to query OSS target")
                .with_detail("targetId", target_id)
                .with_detail("reason", error.to_string())
        })
}

pub fn upsert_oss_target(
    connection: &mut Connection,
    input: UpsertOssTargetInput,
) -> AppResult<OssTargetRecord> {
    let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let timestamp = Utc::now().to_rfc3339();
    let is_default = input.is_default.unwrap_or(false);
    let sort_order = input.sort_order.unwrap_or(0);
    let transaction = connection.savepoint().map_err(|error| {
        AppError::new("DB_WRITE_FAILED", "failed to open OSS target transaction")
            .with_detail("reason", error.to_string())
    })?;

    let account_exists: i64 = transaction
        .query_row(
            "SELECT COUNT(1) FROM oss_accounts WHERE id = ?1",
            [input.account_id.as_str()],
            |row| row.get(0),
        )
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to verify OSS target account")
                .with_detail("accountId", input.account_id.clone())
                .with_detail("reason", error.to_string())
        })?;
    if account_exists != 1 {
        return Err(
            AppError::new("OSS_ACCOUNT_NOT_FOUND", "OSS account was not found")
                .with_detail("accountId", input.account_id),
        );
    }

    if is_default {
        transaction
            .execute(
                "UPDATE oss_targets SET is_default = 0, updated_at = ?1 WHERE account_id = ?2 AND id <> ?3",
                params![timestamp, input.account_id.as_str(), id.as_str()],
            )
            .map_err(|error| {
                AppError::new("DB_WRITE_FAILED", "failed to reset default OSS target")
                    .with_detail("reason", error.to_string())
            })?;
    }

    let existing_created_at: Option<String> = transaction
        .query_row(
            "SELECT created_at FROM oss_targets WHERE id = ?1",
            [id.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to query existing OSS target")
                .with_detail("targetId", id.clone())
                .with_detail("reason", error.to_string())
        })?;
    let created_at = existing_created_at.unwrap_or_else(|| timestamp.clone());

    transaction
        .execute(
            "INSERT INTO oss_targets
             (id, account_id, display_name, bucket, region, endpoint, prefix,
              is_default, sort_order, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(id) DO UPDATE SET
               account_id = excluded.account_id,
               display_name = excluded.display_name,
               bucket = excluded.bucket,
               region = excluded.region,
               endpoint = excluded.endpoint,
               prefix = excluded.prefix,
               is_default = excluded.is_default,
               sort_order = excluded.sort_order,
               updated_at = excluded.updated_at",
            params![
                id,
                input.account_id,
                input.display_name,
                input.bucket,
                input.region,
                input.endpoint,
                input.prefix.unwrap_or_default(),
                if is_default { 1 } else { 0 },
                sort_order,
                created_at,
                timestamp
            ],
        )
        .map_err(map_oss_target_write_error)?;

    transaction.commit().map_err(|error| {
        AppError::new("DB_WRITE_FAILED", "failed to commit OSS target transaction")
            .with_detail("reason", error.to_string())
    })?;

    get_oss_target(connection, id)?
        .ok_or_else(|| AppError::new("DB_READ_FAILED", "failed to load saved OSS target"))
}

pub fn delete_oss_target(
    connection: &mut Connection,
    target_id: String,
) -> AppResult<DeleteResult> {
    let target_id = validate_target_id(target_id)?;
    let active_count: i64 = connection
        .query_row(
            "SELECT COUNT(1) FROM oss_transfer_tasks
             WHERE target_id = ?1
               AND status IN ('queued', 'running', 'paused', 'failed', 'cancelling', 'needs_confirmation')",
            [target_id.as_str()],
            |row| row.get(0),
        )
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to inspect OSS target transfers")
                .with_detail("targetId", target_id.clone())
                .with_detail("reason", error.to_string())
        })?;
    if active_count > 0 {
        return Err(AppError::new(
            "OSS_TARGET_HAS_ACTIVE_TRANSFERS",
            "OSS target has active transfers",
        )
        .with_detail("targetId", target_id)
        .with_detail("activeCount", active_count.to_string()));
    }
    let affected = connection
        .execute(
            "DELETE FROM oss_targets WHERE id = ?1",
            [target_id.as_str()],
        )
        .map_err(|error| {
            AppError::new("DB_WRITE_FAILED", "failed to delete OSS target")
                .with_detail("targetId", target_id.clone())
                .with_detail("reason", error.to_string())
        })?;
    if affected != 1 {
        return Err(
            AppError::new("OSS_TARGET_NOT_FOUND", "OSS target was not found")
                .with_detail("targetId", target_id),
        );
    }
    Ok(DeleteResult { id: target_id })
}

pub fn ensure_oss_target_identity_editable(
    connection: &Connection,
    target_id: String,
) -> AppResult<()> {
    let target_id = validate_target_id(target_id)?;
    let transfer_count: i64 = connection
        .query_row(
            "SELECT COUNT(1) FROM oss_transfer_tasks
             WHERE target_id = ?1
               AND status IN ('queued', 'running', 'paused', 'failed', 'cancelling', 'needs_confirmation')",
            [target_id.as_str()],
            |row| row.get(0),
        )
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to inspect OSS target transfers")
                .with_detail("targetId", target_id.clone())
                .with_detail("reason", error.to_string())
        })?;
    if transfer_count > 0 {
        return Err(AppError::new(
            "OSS_TARGET_IDENTITY_LOCKED",
            "Bucket or prefix cannot change while transfer history exists for this target",
        )
        .with_detail("targetId", target_id)
        .with_detail("transferCount", transfer_count.to_string()));
    }
    Ok(())
}

pub fn insert_oss_transfer_task(
    connection: &mut Connection,
    task: OssTransferTaskRecord,
) -> AppResult<OssTransferTaskRecord> {
    let task_id = task.id.clone();
    let timestamp = Utc::now().to_rfc3339();
    connection
        .execute(
            "INSERT INTO oss_transfer_tasks
             (id, account_id, target_id, operation, object_key, local_path, status,
              bytes_completed, total_bytes, upload_id, checkpoint_json,
              last_error_code, last_error_message, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14)",
            params![
                task_id.as_str(),
                task.account_id,
                task.target_id,
                task.operation,
                task.object_key,
                task.local_path,
                task.status,
                task.bytes_completed as i64,
                task.total_bytes as i64,
                task.upload_id,
                task.checkpoint_json,
                task.last_error_code,
                task.last_error_message,
                timestamp,
            ],
        )
        .map_err(|error| {
            AppError::new("DB_WRITE_FAILED", "failed to insert OSS transfer task")
                .with_detail("reason", error.to_string())
        })?;
    get_oss_transfer_task(connection, task_id)?
        .ok_or_else(|| AppError::new("DB_READ_FAILED", "failed to load OSS transfer task"))
}

pub fn get_oss_transfer_task(
    connection: &Connection,
    operation_id: String,
) -> AppResult<Option<OssTransferTaskRecord>> {
    let operation_id = validate_operation_id(operation_id)?;
    connection
        .query_row(
            "SELECT id, account_id, target_id, operation, object_key, local_path, status,
                    bytes_completed, total_bytes, upload_id, checkpoint_json,
                    last_error_code, last_error_message, created_at, updated_at
             FROM oss_transfer_tasks
             WHERE id = ?1",
            [operation_id.as_str()],
            map_transfer_from_row,
        )
        .optional()
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to query OSS transfer task")
                .with_detail("operationId", operation_id)
                .with_detail("reason", error.to_string())
        })
}

pub fn update_oss_transfer_task(
    connection: &mut Connection,
    task: OssTransferTaskRecord,
) -> AppResult<OssTransferTaskRecord> {
    let operation_id = validate_operation_id(task.id.clone())?;
    let timestamp = Utc::now().to_rfc3339();
    let affected = connection
        .execute(
            "UPDATE oss_transfer_tasks
             SET status = ?1, bytes_completed = ?2, total_bytes = ?3, upload_id = ?4,
                 checkpoint_json = ?5, last_error_code = ?6, last_error_message = ?7, updated_at = ?8
             WHERE id = ?9",
            params![
                task.status,
                task.bytes_completed as i64,
                task.total_bytes as i64,
                task.upload_id,
                task.checkpoint_json,
                task.last_error_code,
                task.last_error_message,
                timestamp,
                operation_id.as_str(),
            ],
        )
        .map_err(|error| {
            AppError::new("DB_WRITE_FAILED", "failed to update OSS transfer task")
                .with_detail("operationId", operation_id.clone())
                .with_detail("reason", error.to_string())
        })?;
    if affected != 1 {
        return Err(
            AppError::new("OSS_TRANSFER_NOT_FOUND", "OSS transfer task was not found")
                .with_detail("operationId", operation_id),
        );
    }
    get_oss_transfer_task(connection, operation_id)?
        .ok_or_else(|| AppError::new("DB_READ_FAILED", "failed to load updated OSS transfer task"))
}

pub fn list_resumable_oss_transfer_tasks(
    connection: &Connection,
) -> AppResult<Vec<OssTransferTaskRecord>> {
    let mut statement = connection
        .prepare(
            "SELECT id, account_id, target_id, operation, object_key, local_path, status,
                    bytes_completed, total_bytes, upload_id, checkpoint_json,
                    last_error_code, last_error_message, created_at, updated_at
             FROM oss_transfer_tasks
             WHERE status IN ('queued', 'running', 'paused', 'cancelling')
             ORDER BY updated_at ASC",
        )
        .map_err(|error| {
            AppError::new(
                "DB_READ_FAILED",
                "failed to prepare OSS transfer task query",
            )
            .with_detail("reason", error.to_string())
        })?;
    let records = statement
        .query_map([], map_transfer_from_row)
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to query OSS transfer tasks")
                .with_detail("reason", error.to_string())
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to map OSS transfer task row")
                .with_detail("reason", error.to_string())
        })?;
    Ok(records)
}

pub fn list_oss_transfer_tasks(connection: &Connection) -> AppResult<Vec<OssTransferTaskRecord>> {
    let mut statement = connection
        .prepare(
            "SELECT id, account_id, target_id, operation, object_key, local_path, status,
                    bytes_completed, total_bytes, upload_id, checkpoint_json,
                    last_error_code, last_error_message, created_at, updated_at
             FROM oss_transfer_tasks
             ORDER BY updated_at DESC",
        )
        .map_err(|error| {
            AppError::new(
                "DB_READ_FAILED",
                "failed to prepare OSS transfer task query",
            )
            .with_detail("reason", error.to_string())
        })?;
    let records = statement
        .query_map([], map_transfer_from_row)
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to query OSS transfer tasks")
                .with_detail("reason", error.to_string())
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to map OSS transfer task row")
                .with_detail("reason", error.to_string())
        })?;
    Ok(records)
}

fn map_oss_account_write_error(error: rusqlite::Error) -> AppError {
    if let rusqlite::Error::SqliteFailure(sql_error, _) = &error {
        if sql_error.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE {
            return AppError::new(
                "OSS_ACCOUNT_NAME_ALREADY_EXISTS",
                "an OSS account with this name already exists",
            );
        }
    }
    AppError::new("DB_WRITE_FAILED", "failed to write OSS account")
        .with_detail("reason", error.to_string())
}

fn map_oss_target_write_error(error: rusqlite::Error) -> AppError {
    if let rusqlite::Error::SqliteFailure(sql_error, _) = &error {
        if sql_error.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE {
            return AppError::new(
                "OSS_TARGET_ALREADY_EXISTS",
                "this OSS target is already registered",
            );
        }
        if sql_error.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY {
            return AppError::new("OSS_ACCOUNT_NOT_FOUND", "OSS account was not found");
        }
    }
    AppError::new("DB_WRITE_FAILED", "failed to write OSS target")
        .with_detail("reason", error.to_string())
}
