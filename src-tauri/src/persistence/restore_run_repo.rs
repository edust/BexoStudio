use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::{
    domain::{
        validate_optional_uuid, RestoreProjectPlan, RestoreRunSummary, RestoreRunTaskRecord,
        SnapshotRecord,
    },
    error::{AppError, AppResult},
};

fn map_restore_run_task_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<RestoreRunTaskRecord> {
    Ok(RestoreRunTaskRecord {
        id: row.get("id")?,
        restore_run_id: row.get("restore_run_id")?,
        project_id: row.get("project_id")?,
        launch_task_id: row.get("launch_task_id")?,
        status: row.get("status")?,
        attempt_count: row.get("attempt_count")?,
        started_at: row.get("started_at")?,
        finished_at: row.get("finished_at")?,
        error_message: row.get("error_message")?,
    })
}

fn map_restore_run_summary_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<RestoreRunSummary> {
    Ok(RestoreRunSummary {
        id: row.get("id")?,
        workspace_id: row.get("workspace_id")?,
        workspace_name: row.get("workspace_name")?,
        snapshot_id: row.get("snapshot_id")?,
        snapshot_name: row.get("snapshot_name")?,
        run_mode: row.get("run_mode")?,
        status: row.get("status")?,
        started_at: row.get("started_at")?,
        finished_at: row.get("finished_at")?,
        error_summary: row.get("error_summary")?,
        planned_task_count: row.get("planned_task_count")?,
        running_task_count: row.get("running_task_count")?,
        completed_task_count: row.get("completed_task_count")?,
        cancelled_task_count: row.get("cancelled_task_count")?,
        failed_task_count: row.get("failed_task_count")?,
        blocked_task_count: row.get("blocked_task_count")?,
        skipped_task_count: row.get("skipped_task_count")?,
    })
}

pub fn insert_restore_run_plan(
    connection: &mut Connection,
    snapshot: &SnapshotRecord,
    mode: &str,
    projects: &[RestoreProjectPlan],
) -> AppResult<(String, Vec<RestoreRunTaskRecord>)> {
    let run_id = Uuid::new_v4().to_string();
    let started_at = Utc::now().to_rfc3339();
    let transaction = connection.transaction().map_err(|error| {
        AppError::new("DB_WRITE_FAILED", "failed to open restore run transaction")
            .with_detail("reason", error.to_string())
    })?;

    transaction
        .execute(
            "INSERT INTO restore_runs
             (id, workspace_id, snapshot_id, run_mode, status, started_at, finished_at, error_summary)
             VALUES (?1, ?2, ?3, ?4, 'running', ?5, NULL, NULL)",
            params![run_id, snapshot.workspace_id, snapshot.id, mode, started_at],
        )
        .map_err(|error| {
            AppError::new("DB_WRITE_FAILED", "failed to insert restore run")
                .with_detail("reason", error.to_string())
        })?;

    let mut task_rows = Vec::with_capacity(projects.len());
    for project in projects {
        let task_id = Uuid::new_v4().to_string();
        let status = match project.status.as_str() {
            "blocked" => "blocked",
            "skipped" => "skipped",
            _ => "planned",
        };
        let reason = project.reason.clone().or_else(|| {
            project
                .actions
                .iter()
                .find_map(|action| action.reason.clone())
        });
        let started = (status != "planned").then(|| started_at.clone());
        let finished = (status != "planned").then(|| started_at.clone());

        transaction
            .execute(
                "INSERT INTO restore_run_tasks
                 (id, restore_run_id, project_id, launch_task_id, status, attempt_count, started_at, finished_at, error_message)
                 VALUES (?1, ?2, ?3, NULL, ?4, 0, ?5, ?6, ?7)",
                params![task_id, run_id, project.project_id, status, started, finished, reason],
            )
            .map_err(|error| {
                AppError::new("DB_WRITE_FAILED", "failed to insert restore run task")
                    .with_detail("reason", error.to_string())
                    .with_detail("projectId", project.project_id.clone())
            })?;

        task_rows.push(RestoreRunTaskRecord {
            id: task_id,
            restore_run_id: run_id.clone(),
            project_id: Some(project.project_id.clone()),
            launch_task_id: None,
            status: status.to_string(),
            attempt_count: 0,
            started_at: started,
            finished_at: finished,
            error_message: reason,
        });

        for action in &project.actions {
            let Some(launch_task_id) = action.launch_task_id.clone() else {
                continue;
            };

            let action_task_id = Uuid::new_v4().to_string();
            let action_status = match action.status.as_str() {
                "blocked" => "blocked",
                "skipped" => "skipped",
                _ => "planned",
            };
            let action_started = (action_status != "planned").then(|| started_at.clone());
            let action_finished = (action_status != "planned").then(|| started_at.clone());

            transaction
                .execute(
                    "INSERT INTO restore_run_tasks
                     (id, restore_run_id, project_id, launch_task_id, status, attempt_count, started_at, finished_at, error_message)
                     VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7, ?8)",
                    params![
                        action_task_id,
                        run_id,
                        project.project_id,
                        launch_task_id.as_str(),
                        action_status,
                        action_started,
                        action_finished,
                        action.reason.as_deref()
                    ],
                )
                .map_err(|error| {
                    AppError::new("DB_WRITE_FAILED", "failed to insert launch task row")
                        .with_detail("reason", error.to_string())
                        .with_detail("projectId", project.project_id.clone())
                })?;

            task_rows.push(RestoreRunTaskRecord {
                id: action_task_id,
                restore_run_id: run_id.clone(),
                project_id: Some(project.project_id.clone()),
                launch_task_id: Some(launch_task_id),
                status: action_status.to_string(),
                attempt_count: 0,
                started_at: action_started,
                finished_at: action_finished,
                error_message: action.reason.clone(),
            });
        }
    }

    transaction.commit().map_err(|error| {
        AppError::new(
            "DB_WRITE_FAILED",
            "failed to commit restore run transaction",
        )
        .with_detail("reason", error.to_string())
    })?;

    Ok((run_id, task_rows))
}

pub fn update_restore_run_task(
    connection: &mut Connection,
    task_id: &str,
    status: &str,
    attempt_count: i64,
    started_at: Option<&str>,
    finished_at: Option<&str>,
    error_message: Option<&str>,
) -> AppResult<()> {
    connection
        .execute(
            "UPDATE restore_run_tasks
             SET status = ?2,
                 attempt_count = ?3,
                 started_at = COALESCE(?4, started_at),
                 finished_at = ?5,
                 error_message = ?6
             WHERE id = ?1",
            params![
                task_id,
                status,
                attempt_count,
                started_at,
                finished_at,
                error_message
            ],
        )
        .map_err(|error| {
            AppError::new("DB_WRITE_FAILED", "failed to update restore run task")
                .with_detail("reason", error.to_string())
                .with_detail("taskId", task_id.to_string())
        })?;
    Ok(())
}

pub fn update_restore_run_status(
    connection: &Connection,
    run_id: &str,
    status: &str,
    error_summary: Option<&str>,
) -> AppResult<()> {
    connection
        .execute(
            "UPDATE restore_runs
             SET status = ?2,
                 error_summary = COALESCE(?3, error_summary)
             WHERE id = ?1",
            params![run_id, status, error_summary],
        )
        .map_err(|error| {
            AppError::new("DB_WRITE_FAILED", "failed to update restore run status")
                .with_detail("reason", error.to_string())
                .with_detail("restoreRunId", run_id.to_string())
        })?;
    Ok(())
}

pub fn finalize_restore_run(
    connection: &mut Connection,
    snapshot: &SnapshotRecord,
    run_id: &str,
    status: &str,
    error_summary: Option<&str>,
) -> AppResult<()> {
    let finished_at = Utc::now().to_rfc3339();
    let snapshot_status = match status {
        "completed" => "completed",
        "failed" => "failed",
        "cancelled" => "cancelled",
        _ => "completed_with_warnings",
    };

    let transaction = connection.transaction().map_err(|error| {
        AppError::new(
            "DB_WRITE_FAILED",
            "failed to open restore finalize transaction",
        )
        .with_detail("reason", error.to_string())
    })?;

    transaction
        .execute(
            "UPDATE restore_runs
             SET status = ?2, finished_at = ?3, error_summary = ?4
             WHERE id = ?1",
            params![run_id, status, finished_at, error_summary],
        )
        .map_err(|error| {
            AppError::new("DB_WRITE_FAILED", "failed to finalize restore run")
                .with_detail("reason", error.to_string())
                .with_detail("restoreRunId", run_id.to_string())
        })?;

    transaction
        .execute(
            "UPDATE snapshots
             SET last_restore_at = ?2, last_restore_status = ?3, updated_at = ?2
             WHERE id = ?1",
            params![snapshot.id, finished_at, snapshot_status],
        )
        .map_err(|error| {
            AppError::new("DB_WRITE_FAILED", "failed to update snapshot restore state")
                .with_detail("reason", error.to_string())
                .with_detail("snapshotId", snapshot.id.clone())
        })?;

    transaction.commit().map_err(|error| {
        AppError::new(
            "DB_WRITE_FAILED",
            "failed to commit restore finalize transaction",
        )
        .with_detail("reason", error.to_string())
    })?;

    Ok(())
}

pub fn recover_interrupted_restore_runs(connection: &mut Connection) -> AppResult<Vec<String>> {
    let mut statement = connection
        .prepare(
            "SELECT id, snapshot_id
             FROM restore_runs
             WHERE status IN ('running', 'cancel_requested')",
        )
        .map_err(|error| {
            AppError::new(
                "DB_READ_FAILED",
                "failed to prepare interrupted restore run query",
            )
            .with_detail("reason", error.to_string())
        })?;

    let interrupted_rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>("id")?,
                row.get::<_, Option<String>>("snapshot_id")?,
            ))
        })
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to query interrupted restore runs")
                .with_detail("reason", error.to_string())
        })?;

    let mut interrupted = Vec::new();
    for row in interrupted_rows {
        interrupted.push(row.map_err(|error| {
            AppError::new(
                "DB_READ_FAILED",
                "failed to map interrupted restore run row",
            )
            .with_detail("reason", error.to_string())
        })?);
    }
    drop(statement);

    if interrupted.is_empty() {
        return Ok(Vec::new());
    }

    let finished_at = Utc::now().to_rfc3339();
    let transaction = connection.transaction().map_err(|error| {
        AppError::new(
            "DB_WRITE_FAILED",
            "failed to open interrupted restore recovery transaction",
        )
        .with_detail("reason", error.to_string())
    })?;

    let running_reason = "恢复执行在应用关闭或重启后被回收";
    let skipped_reason = "恢复执行已中断，未开始的任务已跳过";

    for (run_id, snapshot_id) in &interrupted {
        transaction
            .execute(
                "UPDATE restore_run_tasks
                 SET status = CASE
                        WHEN status = 'running' THEN 'cancelled'
                        WHEN status = 'planned' THEN 'skipped'
                        ELSE status
                     END,
                     finished_at = COALESCE(finished_at, ?2),
                     error_message = CASE
                        WHEN status = 'running' AND error_message IS NULL THEN ?3
                        WHEN status = 'planned' AND error_message IS NULL THEN ?4
                        ELSE error_message
                     END
                 WHERE restore_run_id = ?1
                   AND status IN ('running', 'planned')",
                params![run_id, finished_at, running_reason, skipped_reason],
            )
            .map_err(|error| {
                AppError::new(
                    "DB_WRITE_FAILED",
                    "failed to recover interrupted restore run tasks",
                )
                .with_detail("reason", error.to_string())
                .with_detail("restoreRunId", run_id.clone())
            })?;

        transaction
            .execute(
                "UPDATE restore_runs
                 SET status = 'cancelled',
                     finished_at = ?2,
                     error_summary = COALESCE(error_summary, ?3)
                 WHERE id = ?1",
                params![run_id, finished_at, running_reason],
            )
            .map_err(|error| {
                AppError::new(
                    "DB_WRITE_FAILED",
                    "failed to recover interrupted restore run",
                )
                .with_detail("reason", error.to_string())
                .with_detail("restoreRunId", run_id.clone())
            })?;

        if let Some(snapshot_id) = snapshot_id {
            transaction
                .execute(
                    "UPDATE snapshots
                     SET last_restore_at = ?2,
                         last_restore_status = 'cancelled',
                         updated_at = ?2
                     WHERE id = ?1",
                    params![snapshot_id, finished_at],
                )
                .map_err(|error| {
                    AppError::new(
                        "DB_WRITE_FAILED",
                        "failed to update recovered snapshot state",
                    )
                    .with_detail("reason", error.to_string())
                    .with_detail("snapshotId", snapshot_id.clone())
                })?;
        }
    }

    transaction.commit().map_err(|error| {
        AppError::new(
            "DB_WRITE_FAILED",
            "failed to commit interrupted restore recovery transaction",
        )
        .with_detail("reason", error.to_string())
    })?;

    Ok(interrupted.into_iter().map(|(run_id, _)| run_id).collect())
}

pub fn insert_restore_dry_run(
    connection: &mut Connection,
    snapshot: &SnapshotRecord,
    mode: &str,
    projects: &[RestoreProjectPlan],
) -> AppResult<String> {
    let run_id = Uuid::new_v4().to_string();
    let started_at = Utc::now().to_rfc3339();
    let finished_at = Utc::now().to_rfc3339();
    let blocked_count = projects
        .iter()
        .filter(|project| project.status == "blocked")
        .count();
    let error_summary =
        (blocked_count > 0).then(|| format!("{blocked_count} project(s) blocked during dry-run"));
    let final_status = if blocked_count > 0 {
        "completed_with_blocks"
    } else {
        "completed"
    };

    let transaction = connection.transaction().map_err(|error| {
        AppError::new("DB_WRITE_FAILED", "failed to open restore run transaction")
            .with_detail("reason", error.to_string())
    })?;

    transaction
        .execute(
            "INSERT INTO restore_runs
             (id, workspace_id, snapshot_id, run_mode, status, started_at, finished_at, error_summary)
             VALUES (?1, ?2, ?3, ?4, 'planning', ?5, NULL, NULL)",
            params![run_id, snapshot.workspace_id, snapshot.id, format!("dry_run:{mode}"), started_at],
        )
        .map_err(|error| {
            AppError::new("DB_WRITE_FAILED", "failed to insert restore run")
                .with_detail("reason", error.to_string())
        })?;

    for project in projects {
        let task_id = Uuid::new_v4().to_string();
        let reason = project.reason.clone().or_else(|| {
            project
                .actions
                .iter()
                .find_map(|action| action.reason.clone())
        });

        transaction
            .execute(
                "INSERT INTO restore_run_tasks
                 (id, restore_run_id, project_id, launch_task_id, status, attempt_count, started_at, finished_at, error_message)
                 VALUES (?1, ?2, ?3, NULL, ?4, 0, ?5, ?6, ?7)",
                params![
                    task_id,
                    run_id,
                    project.project_id,
                    project.status,
                    started_at,
                    finished_at,
                    reason
                ],
            )
            .map_err(|error| {
                AppError::new("DB_WRITE_FAILED", "failed to insert restore run task")
                    .with_detail("reason", error.to_string())
                    .with_detail("projectId", project.project_id.clone())
            })?;

        for action in &project.actions {
            let Some(launch_task_id) = action.launch_task_id.clone() else {
                continue;
            };

            transaction
                .execute(
                    "INSERT INTO restore_run_tasks
                     (id, restore_run_id, project_id, launch_task_id, status, attempt_count, started_at, finished_at, error_message)
                     VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7, ?8)",
                    params![
                        Uuid::new_v4().to_string(),
                        run_id,
                        project.project_id,
                        launch_task_id.as_str(),
                        action.status.as_str(),
                        started_at,
                        finished_at,
                        action.reason.as_deref()
                    ],
                )
                .map_err(|error| {
                    AppError::new("DB_WRITE_FAILED", "failed to insert launch task dry-run row")
                        .with_detail("reason", error.to_string())
                        .with_detail("projectId", project.project_id.clone())
                })?;
        }
    }

    transaction
        .execute(
            "UPDATE restore_runs
             SET status = ?2, finished_at = ?3, error_summary = ?4
             WHERE id = ?1",
            params![run_id, final_status, finished_at, error_summary],
        )
        .map_err(|error| {
            AppError::new("DB_WRITE_FAILED", "failed to finalize restore run")
                .with_detail("reason", error.to_string())
        })?;

    let snapshot_status = if blocked_count > 0 {
        "completed_with_warnings"
    } else {
        "completed"
    };
    transaction
        .execute(
            "UPDATE snapshots
             SET last_restore_at = ?2, last_restore_status = ?3, updated_at = ?2
             WHERE id = ?1",
            params![snapshot.id, finished_at, snapshot_status],
        )
        .map_err(|error| {
            AppError::new("DB_WRITE_FAILED", "failed to update snapshot restore state")
                .with_detail("reason", error.to_string())
                .with_detail("snapshotId", snapshot.id.clone())
        })?;

    transaction.commit().map_err(|error| {
        AppError::new(
            "DB_WRITE_FAILED",
            "failed to commit restore run transaction",
        )
        .with_detail("reason", error.to_string())
    })?;

    Ok(run_id)
}

pub fn list_restore_runs(connection: &Connection) -> AppResult<Vec<RestoreRunSummary>> {
    let mut statement = connection
        .prepare(&format!(
            "{RESTORE_RUN_SUMMARY_SELECT}{RESTORE_RUN_SUMMARY_GROUP_ORDER}"
        ))
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to prepare restore run query")
                .with_detail("reason", error.to_string())
        })?;

    let rows = statement
        .query_map([], map_restore_run_summary_from_row)
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to query restore runs")
                .with_detail("reason", error.to_string())
        })?;

    let mut runs = Vec::new();
    for row in rows {
        runs.push(row.map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to map restore run row")
                .with_detail("reason", error.to_string())
        })?);
    }

    Ok(runs)
}

pub fn get_restore_run_summary(
    connection: &Connection,
    id: String,
) -> AppResult<RestoreRunSummary> {
    let id = validate_optional_uuid("restoreRunId", Some(id))?
        .ok_or_else(|| AppError::validation("restoreRunId is required"))?;
    let query =
        format!("{RESTORE_RUN_SUMMARY_SELECT} AND rr.id = ?1 {RESTORE_RUN_SUMMARY_GROUP_ONLY}");

    connection
        .query_row(&query, [id.as_str()], map_restore_run_summary_from_row)
        .optional()
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to load restore run detail")
                .with_detail("reason", error.to_string())
        })?
        .ok_or_else(|| {
            AppError::new("RESTORE_RUN_NOT_FOUND", "restore run was not found")
                .with_detail("restoreRunId", id)
        })
}

pub fn list_restore_run_tasks(
    connection: &Connection,
    restore_run_id: String,
) -> AppResult<Vec<RestoreRunTaskRecord>> {
    let restore_run_id = validate_optional_uuid("restoreRunId", Some(restore_run_id))?
        .ok_or_else(|| AppError::validation("restoreRunId is required"))?;

    let mut statement = connection
        .prepare(
            "SELECT id, restore_run_id, project_id, launch_task_id, status, attempt_count, started_at, finished_at, error_message
             FROM restore_run_tasks
             WHERE restore_run_id = ?1
             ORDER BY started_at ASC, id ASC",
        )
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to prepare restore run task query")
                .with_detail("reason", error.to_string())
        })?;

    let rows = statement
        .query_map([restore_run_id.as_str()], map_restore_run_task_from_row)
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to query restore run tasks")
                .with_detail("reason", error.to_string())
        })?;

    let mut tasks = Vec::new();
    for row in rows {
        tasks.push(row.map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to map restore run task row")
                .with_detail("reason", error.to_string())
        })?);
    }

    Ok(tasks)
}

const RESTORE_RUN_SUMMARY_SELECT: &str = "
SELECT rr.id,
       rr.workspace_id,
       w.name AS workspace_name,
       rr.snapshot_id,
       s.name AS snapshot_name,
       rr.run_mode,
       rr.status,
       rr.started_at,
       rr.finished_at,
       rr.error_summary,
       COALESCE(SUM(CASE WHEN rrt.launch_task_id IS NULL AND rrt.status = 'planned' THEN 1 ELSE 0 END), 0) AS planned_task_count,
       COALESCE(SUM(CASE WHEN rrt.launch_task_id IS NULL AND rrt.status = 'running' THEN 1 ELSE 0 END), 0) AS running_task_count,
       COALESCE(SUM(CASE WHEN rrt.launch_task_id IS NULL AND rrt.status = 'completed' THEN 1 ELSE 0 END), 0) AS completed_task_count,
       COALESCE(SUM(CASE WHEN rrt.launch_task_id IS NULL AND rrt.status = 'cancelled' THEN 1 ELSE 0 END), 0) AS cancelled_task_count,
       COALESCE(SUM(CASE WHEN rrt.launch_task_id IS NULL AND rrt.status = 'failed' THEN 1 ELSE 0 END), 0) AS failed_task_count,
       COALESCE(SUM(CASE WHEN rrt.launch_task_id IS NULL AND rrt.status = 'blocked' THEN 1 ELSE 0 END), 0) AS blocked_task_count,
       COALESCE(SUM(CASE WHEN rrt.launch_task_id IS NULL AND rrt.status = 'skipped' THEN 1 ELSE 0 END), 0) AS skipped_task_count
FROM restore_runs rr
INNER JOIN workspaces w ON w.id = rr.workspace_id
LEFT JOIN snapshots s ON s.id = rr.snapshot_id
LEFT JOIN restore_run_tasks rrt ON rrt.restore_run_id = rr.id
WHERE 1 = 1
";

const RESTORE_RUN_SUMMARY_GROUP_ONLY: &str = "
GROUP BY rr.id, rr.workspace_id, w.name, rr.snapshot_id, s.name, rr.run_mode, rr.status, rr.started_at, rr.finished_at, rr.error_summary
";

const RESTORE_RUN_SUMMARY_GROUP_ORDER: &str = "
GROUP BY rr.id, rr.workspace_id, w.name, rr.snapshot_id, s.name, rr.run_mode, rr.status, rr.started_at, rr.finished_at, rr.error_summary
ORDER BY rr.started_at DESC
";
