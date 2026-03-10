use std::{fs, path::PathBuf, time::Duration};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{
    domain::RestoreRunDetail,
    error::{AppError, AppResult},
};

#[derive(Debug, Clone)]
pub struct RestoreLogStore {
    log_dir: PathBuf,
    write_timeout: Duration,
    read_timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RestoreRunLogRecord {
    exported_at: String,
    detail: RestoreRunDetail,
}

impl RestoreLogStore {
    pub fn new(log_dir: PathBuf) -> Self {
        Self {
            log_dir,
            write_timeout: Duration::from_secs(2),
            read_timeout: Duration::from_secs(2),
        }
    }

    pub async fn write_run_detail(&self, detail: RestoreRunDetail) -> AppResult<()> {
        let log_dir = self.log_dir.clone();
        let file_name = format!("restore-run-{}.json", detail.run.id);

        run_blocking("write_restore_run_log", self.write_timeout, move || {
            fs::create_dir_all(&log_dir).map_err(|error| {
                AppError::new(
                    "RESTORE_LOG_WRITE_FAILED",
                    "failed to create restore log directory",
                )
                .with_detail("path", log_dir.display().to_string())
                .with_detail("reason", error.to_string())
            })?;

            let payload = serde_json::to_string_pretty(&RestoreRunLogRecord {
                exported_at: Utc::now().to_rfc3339(),
                detail,
            })
            .map_err(|error| {
                AppError::new(
                    "RESTORE_LOG_WRITE_FAILED",
                    "failed to serialize restore run log",
                )
                .with_detail("reason", error.to_string())
            })?;

            let file_path = log_dir.join(file_name);
            fs::write(&file_path, payload).map_err(|error| {
                AppError::new(
                    "RESTORE_LOG_WRITE_FAILED",
                    "failed to write restore run log",
                )
                .with_detail("path", file_path.display().to_string())
                .with_detail("reason", error.to_string())
            })?;
            Ok(())
        })
        .await
    }

    pub async fn read_run_detail(&self, run_id: String) -> AppResult<Option<RestoreRunDetail>> {
        let log_dir = self.log_dir.clone();
        let file_path = log_dir.join(format!("restore-run-{run_id}.json"));

        run_blocking("read_restore_run_log", self.read_timeout, move || {
            if !file_path.exists() {
                return Ok(None);
            }

            let payload = fs::read_to_string(&file_path).map_err(|error| {
                AppError::new("RESTORE_LOG_READ_FAILED", "failed to read restore run log")
                    .with_detail("path", file_path.display().to_string())
                    .with_detail("reason", error.to_string())
            })?;

            let record =
                serde_json::from_str::<RestoreRunLogRecord>(&payload).map_err(|error| {
                    AppError::new(
                        "RESTORE_LOG_READ_FAILED",
                        "failed to deserialize restore run log",
                    )
                    .with_detail("path", file_path.display().to_string())
                    .with_detail("reason", error.to_string())
                })?;

            Ok(Some(record.detail))
        })
        .await
    }

    pub async fn ensure_log_dir(&self) -> AppResult<PathBuf> {
        let log_dir = self.log_dir.clone();

        run_blocking("ensure_restore_log_dir", self.write_timeout, move || {
            fs::create_dir_all(&log_dir).map_err(|error| {
                AppError::new(
                    "RESTORE_LOG_WRITE_FAILED",
                    "failed to create restore log directory",
                )
                .with_detail("path", log_dir.display().to_string())
                .with_detail("reason", error.to_string())
            })?;
            Ok(log_dir)
        })
        .await
    }
}

async fn run_blocking<T, F>(
    operation_name: &'static str,
    timeout: Duration,
    operation: F,
) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> AppResult<T> + Send + 'static,
{
    let handle = tauri::async_runtime::spawn_blocking(operation);
    match tokio::time::timeout(timeout, handle).await {
        Ok(joined) => match joined {
            Ok(result) => result,
            Err(error) => Err(AppError::new(
                "RESTORE_LOG_TASK_FAILED",
                "restore log task join failed",
            )
            .with_detail("operation", operation_name)
            .with_detail("reason", error.to_string())),
        },
        Err(_) => Err(
            AppError::new("RESTORE_LOG_TIMEOUT", "restore log write timed out")
                .with_detail("operation", operation_name)
                .retryable(true),
        ),
    }
}
