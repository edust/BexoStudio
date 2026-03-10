use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::{
    domain::{
        validate_launch_task_args, validate_launch_task_command, validate_launch_task_id,
        validate_launch_task_retry_policy, validate_launch_task_timeout, validate_launch_task_type,
        validate_launch_task_working_dir, validate_optional_uuid, DeleteResult, LaunchTaskRecord,
        LaunchTaskRetryPolicy, UpsertLaunchTaskInput,
    },
    error::{AppError, AppResult},
};

fn map_launch_task_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LaunchTaskRecord> {
    let args_json: String = row.get("args_json")?;
    let retry_policy_json: String = row.get("retry_policy_json")?;
    let args = serde_json::from_str(&args_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })?;
    let retry_policy =
        serde_json::from_str::<LaunchTaskRetryPolicy>(&retry_policy_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;

    Ok(LaunchTaskRecord {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        name: row.get("name")?,
        task_type: row.get("task_type")?,
        enabled: row.get::<_, i64>("enabled")? != 0,
        command: row.get("command")?,
        args,
        working_dir: row.get("working_dir")?,
        timeout_ms: row.get("timeout_ms")?,
        continue_on_failure: row.get::<_, i64>("continue_on_failure")? != 0,
        retry_policy,
        sort_order: row.get("sort_order")?,
    })
}

pub fn list_all_launch_tasks(connection: &Connection) -> AppResult<Vec<LaunchTaskRecord>> {
    let mut statement = connection
        .prepare(
            "SELECT id, project_id, name, task_type, enabled, command, args_json, working_dir,
                    timeout_ms, continue_on_failure, retry_policy_json, sort_order
             FROM launch_tasks
             ORDER BY sort_order ASC, name ASC",
        )
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to prepare launch task query")
                .with_detail("reason", error.to_string())
        })?;

    let rows = statement
        .query_map([], map_launch_task_from_row)
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to query launch tasks")
                .with_detail("reason", error.to_string())
        })?;

    let mut tasks = Vec::new();
    for row in rows {
        tasks.push(row.map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to map launch task row")
                .with_detail("reason", error.to_string())
        })?);
    }

    Ok(tasks)
}

pub fn list_launch_tasks(
    connection: &Connection,
    project_id: String,
) -> AppResult<Vec<LaunchTaskRecord>> {
    let project_id = validate_optional_uuid("projectId", Some(project_id))?
        .ok_or_else(|| AppError::validation("projectId is required"))?;

    let mut statement = connection
        .prepare(
            "SELECT id, project_id, name, task_type, enabled, command, args_json, working_dir,
                    timeout_ms, continue_on_failure, retry_policy_json, sort_order
             FROM launch_tasks
             WHERE project_id = ?1
             ORDER BY sort_order ASC, name ASC",
        )
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to prepare launch task query")
                .with_detail("reason", error.to_string())
        })?;

    let rows = statement
        .query_map([project_id.as_str()], map_launch_task_from_row)
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to query launch tasks")
                .with_detail("reason", error.to_string())
        })?;

    let mut tasks = Vec::new();
    for row in rows {
        tasks.push(row.map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to map launch task row")
                .with_detail("reason", error.to_string())
        })?);
    }

    Ok(tasks)
}

pub fn upsert_launch_task(
    connection: &mut Connection,
    input: UpsertLaunchTaskInput,
) -> AppResult<LaunchTaskRecord> {
    let id = validate_launch_task_id(input.id)?.unwrap_or_else(|| Uuid::new_v4().to_string());
    let project_id = validate_optional_uuid("projectId", Some(input.project_id))?
        .ok_or_else(|| AppError::validation("projectId is required"))?;
    let name = crate::domain::require_non_empty("name", &input.name, 100)?;
    let task_type = validate_launch_task_type(&input.task_type)?;
    let command = validate_launch_task_command(&task_type, &input.command)?;
    let args = validate_launch_task_args(input.args)?;
    let working_dir = validate_launch_task_working_dir(input.working_dir)?;
    let timeout_ms = validate_launch_task_timeout(input.timeout_ms)?;
    let continue_on_failure = input.continue_on_failure.unwrap_or(false);
    let enabled = input.enabled.unwrap_or(true);
    let retry_policy = validate_launch_task_retry_policy(input.retry_policy)?;
    let sort_order = input.sort_order.unwrap_or(0);

    if task_type == "open_path" && !args.is_empty() {
        return Err(AppError::validation(
            "open_path launch task cannot have args",
        ));
    }

    let project_exists = connection
        .query_row(
            "SELECT 1 FROM projects WHERE id = ?1 LIMIT 1",
            [project_id.as_str()],
            |_| Ok(true),
        )
        .optional()
        .map_err(|error| {
            AppError::new(
                "DB_READ_FAILED",
                "failed to query project before saving launch task",
            )
            .with_detail("reason", error.to_string())
        })?
        .unwrap_or(false);
    if !project_exists {
        return Err(AppError::new("PROJECT_NOT_FOUND", "project was not found")
            .with_detail("projectId", project_id));
    }

    let args_json = serde_json::to_string(&args).map_err(|error| {
        AppError::new("VALIDATION_ERROR", "failed to serialize launch task args")
            .with_detail("reason", error.to_string())
    })?;
    let retry_policy_json = serde_json::to_string(&retry_policy).map_err(|error| {
        AppError::new(
            "VALIDATION_ERROR",
            "failed to serialize launch task retry policy",
        )
        .with_detail("reason", error.to_string())
    })?;
    let timestamp = Utc::now().to_rfc3339();

    connection
        .execute(
            "INSERT INTO launch_tasks
             (id, project_id, name, task_type, enabled, command, args_json, working_dir, timeout_ms,
              continue_on_failure, retry_policy_json, sort_order)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(id) DO UPDATE SET
               project_id = excluded.project_id,
               name = excluded.name,
               task_type = excluded.task_type,
               enabled = excluded.enabled,
               command = excluded.command,
               args_json = excluded.args_json,
               working_dir = excluded.working_dir,
               timeout_ms = excluded.timeout_ms,
               continue_on_failure = excluded.continue_on_failure,
               retry_policy_json = excluded.retry_policy_json,
               sort_order = excluded.sort_order",
            params![
                id,
                project_id,
                name,
                task_type,
                if enabled { 1 } else { 0 },
                command,
                args_json,
                working_dir,
                timeout_ms,
                if continue_on_failure { 1 } else { 0 },
                retry_policy_json,
                sort_order
            ],
        )
        .map_err(|error| {
            AppError::new("DB_WRITE_FAILED", "failed to save launch task")
                .with_detail("reason", error.to_string())
                .with_detail("updatedAt", timestamp)
        })?;

    connection
        .query_row(
            "SELECT id, project_id, name, task_type, enabled, command, args_json, working_dir,
                    timeout_ms, continue_on_failure, retry_policy_json, sort_order
             FROM launch_tasks
             WHERE id = ?1",
            [id.as_str()],
            map_launch_task_from_row,
        )
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to load saved launch task")
                .with_detail("reason", error.to_string())
        })
}

pub fn delete_launch_task(connection: &mut Connection, id: String) -> AppResult<DeleteResult> {
    let id =
        validate_launch_task_id(Some(id))?.ok_or_else(|| AppError::validation("id is required"))?;

    let affected_rows = connection
        .execute("DELETE FROM launch_tasks WHERE id = ?1", [id.as_str()])
        .map_err(|error| {
            AppError::new("DB_WRITE_FAILED", "failed to delete launch task")
                .with_detail("reason", error.to_string())
        })?;

    if affected_rows == 0 {
        return Err(
            AppError::new("LAUNCH_TASK_NOT_FOUND", "launch task was not found")
                .with_detail("launchTaskId", id.clone()),
        );
    }

    Ok(DeleteResult { id })
}
