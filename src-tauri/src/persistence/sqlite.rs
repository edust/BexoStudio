use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

use super::schema;

#[derive(Debug, Clone)]
pub struct Database {
    db_path: PathBuf,
    operation_timeout: Duration,
    init_timeout: Duration,
}

impl Database {
    pub fn new(db_path: PathBuf) -> Self {
        Self {
            db_path,
            operation_timeout: Duration::from_secs(3),
            init_timeout: Duration::from_secs(5),
        }
    }

    pub async fn initialize(&self) -> AppResult<()> {
        let db_path = self.db_path.clone();
        run_connection_operation(
            db_path,
            "initialize_database",
            self.init_timeout,
            true,
            DatabaseOperationMode::Write,
            move |connection| {
                connection.execute_batch(schema::SCHEMA).map_err(|error| {
                    AppError::new("DB_INIT_FAILED", "failed to initialize database schema")
                        .with_detail("reason", error.to_string())
                })?;
                Ok(())
            },
        )
        .await
    }

    pub async fn read<T, F>(&self, operation_name: &'static str, operation: F) -> AppResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> AppResult<T> + Send + 'static,
    {
        let db_path = self.db_path.clone();
        run_connection_operation(
            db_path,
            operation_name,
            self.operation_timeout,
            false,
            DatabaseOperationMode::Read,
            move |connection| operation(connection),
        )
        .await
    }

    pub async fn write<T, F>(&self, operation_name: &'static str, operation: F) -> AppResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> AppResult<T> + Send + 'static,
    {
        let db_path = self.db_path.clone();
        run_connection_operation(
            db_path,
            operation_name,
            self.operation_timeout,
            false,
            DatabaseOperationMode::Write,
            operation,
        )
        .await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DatabaseOperationMode {
    Read,
    Write,
}

fn open_connection(path: &Path) -> AppResult<Connection> {
    let connection = Connection::open(path).map_err(|error| {
        AppError::new("DB_OPEN_FAILED", "failed to open sqlite database")
            .with_detail("path", path.display().to_string())
            .with_detail("reason", error.to_string())
    })?;

    connection
        .busy_timeout(Duration::from_millis(1500))
        .map_err(|error| {
            AppError::new("DB_OPEN_FAILED", "failed to configure sqlite busy timeout")
                .with_detail("reason", error.to_string())
        })?;

    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(|error| {
            AppError::new("DB_OPEN_FAILED", "failed to enable sqlite foreign keys")
                .with_detail("reason", error.to_string())
        })?;

    Ok(connection)
}

async fn run_connection_operation<T, F>(
    db_path: PathBuf,
    operation_name: &'static str,
    timeout: Duration,
    create_parent: bool,
    mode: DatabaseOperationMode,
    operation: F,
) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce(&mut Connection) -> AppResult<T> + Send + 'static,
{
    let cancellation_requested = Arc::new(AtomicBool::new(false));
    let worker_cancellation = Arc::clone(&cancellation_requested);
    let (interrupt_sender, interrupt_receiver) = mpsc::sync_channel(1);

    let mut handle = tauri::async_runtime::spawn_blocking(move || {
        if create_parent {
            if let Some(parent) = db_path.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    AppError::new("DB_INIT_FAILED", "failed to create database directory")
                        .with_detail("path", parent.display().to_string())
                        .with_detail("reason", error.to_string())
                })?;
            }
        }

        if worker_cancellation.load(Ordering::Acquire) {
            return Err(database_cancelled_error());
        }

        let mut connection = open_connection(&db_path)?;
        let interrupt_handle = connection.get_interrupt_handle();
        let _ = interrupt_sender.send(interrupt_handle);

        if worker_cancellation.load(Ordering::Acquire) {
            return Err(database_cancelled_error());
        }

        match mode {
            DatabaseOperationMode::Read => {
                let result = operation(&mut connection);
                if worker_cancellation.load(Ordering::Acquire) {
                    Err(database_cancelled_error())
                } else {
                    result
                }
            }
            DatabaseOperationMode::Write => {
                run_write_transaction(&mut connection, &worker_cancellation, operation)
            }
        }
    });

    match tokio::time::timeout(timeout, &mut handle).await {
        Ok(joined) => map_database_join_result(operation_name, joined),
        Err(_) => {
            cancellation_requested.store(true, Ordering::Release);
            if let Ok(interrupt_handle) = interrupt_receiver.try_recv() {
                interrupt_handle.interrupt();
            }

            let joined = handle.await;
            match joined {
                Ok(Ok(value)) => Ok(value),
                Ok(Err(error))
                    if matches!(
                        error.code.as_str(),
                        "DB_COMMIT_FAILED" | "DB_ROLLBACK_FAILED"
                    ) =>
                {
                    Err(error.with_detail("operation", operation_name))
                }
                Ok(Err(_)) => Err(AppError::new("DB_TIMEOUT", "database operation timed out")
                    .with_detail("operation", operation_name)
                    .with_detail("terminalState", "rolled_back_or_not_started")
                    .retryable(true)),
                Err(error) => Err(database_join_error(operation_name, error.to_string())),
            }
        }
    }
}

fn run_write_transaction<T, F>(
    connection: &mut Connection,
    cancellation_requested: &AtomicBool,
    operation: F,
) -> AppResult<T>
where
    F: FnOnce(&mut Connection) -> AppResult<T>,
{
    connection
        .execute_batch("BEGIN IMMEDIATE")
        .map_err(|error| {
            AppError::new(
                "DB_WRITE_FAILED",
                "failed to begin database write transaction",
            )
            .with_detail("reason", error.to_string())
        })?;

    if cancellation_requested.load(Ordering::Acquire) {
        rollback_transaction(connection, None)?;
        return Err(database_cancelled_error());
    }

    let result = operation(connection);
    match result {
        Ok(value) => {
            if cancellation_requested.load(Ordering::Acquire) {
                rollback_transaction(connection, None)?;
                return Err(database_cancelled_error());
            }

            connection.execute_batch("COMMIT").map_err(|error| {
                let mut app_error =
                    AppError::new("DB_COMMIT_FAILED", "failed to commit database transaction")
                        .with_detail("reason", error.to_string());
                if let Err(rollback_error) = connection.execute_batch("ROLLBACK") {
                    app_error = app_error.with_detail("rollbackReason", rollback_error.to_string());
                }
                app_error
            })?;
            Ok(value)
        }
        Err(error) => {
            rollback_transaction(connection, Some(&error))?;
            Err(error)
        }
    }
}

fn rollback_transaction(
    connection: &Connection,
    original_error: Option<&AppError>,
) -> AppResult<()> {
    connection
        .execute_batch("ROLLBACK")
        .map_err(|rollback_error| {
            let mut error = AppError::new(
                "DB_ROLLBACK_FAILED",
                "failed to roll back database transaction",
            )
            .with_detail("reason", rollback_error.to_string());
            if let Some(original_error) = original_error {
                error = error
                    .with_detail("originalCode", original_error.code.clone())
                    .with_detail("originalMessage", original_error.message.clone());
            }
            error
        })
}

fn database_cancelled_error() -> AppError {
    AppError::new(
        "DB_OPERATION_CANCELLED",
        "database operation was cancelled before commit",
    )
}

fn map_database_join_result<T>(
    operation_name: &'static str,
    joined: Result<AppResult<T>, impl ToString>,
) -> AppResult<T> {
    match joined {
        Ok(result) => result,
        Err(error) => Err(database_join_error(operation_name, error.to_string())),
    }
}

fn database_join_error(operation_name: &'static str, reason: String) -> AppError {
    AppError::new("DB_TASK_FAILED", "database task join failed")
        .with_detail("operation", operation_name)
        .with_detail("reason", reason)
}

#[cfg(test)]
mod tests {
    use std::time::{Instant, SystemTime, UNIX_EPOCH};

    use super::*;

    fn unique_database_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "bexo-studio-{label}-{}-{nonce}.sqlite3",
            std::process::id()
        ))
    }

    fn sqlite_test_error(operation: &'static str, error: rusqlite::Error) -> AppError {
        AppError::new("DB_TEST_FAILED", "sqlite test operation failed")
            .with_detail("operation", operation)
            .with_detail("reason", error.to_string())
    }

    #[tokio::test]
    async fn timed_out_write_is_rolled_back_before_returning() {
        let db_path = unique_database_path("timeout-rollback");
        let database = Database::new(db_path.clone());
        database.initialize().await.expect("database init");
        database
            .write("create_timeout_test_table", |connection| {
                connection
                    .execute_batch(
                        "CREATE TABLE timeout_test (id INTEGER PRIMARY KEY, value TEXT NOT NULL);",
                    )
                    .map_err(|error| sqlite_test_error("create table", error))
            })
            .await
            .expect("create timeout test table");

        let short_timeout_database = Database {
            db_path: db_path.clone(),
            operation_timeout: Duration::from_millis(20),
            init_timeout: Duration::from_secs(5),
        };
        let started_at = Instant::now();
        let error = short_timeout_database
            .write("slow_timeout_test_write", |connection| {
                connection
                    .execute("INSERT INTO timeout_test (value) VALUES ('pending')", [])
                    .map_err(|error| sqlite_test_error("insert pending row", error))?;
                std::thread::sleep(Duration::from_millis(80));
                Ok(())
            })
            .await
            .expect_err("slow write should time out");

        assert_eq!(error.code, "DB_TIMEOUT");
        assert_eq!(error.retryable, Some(true));
        assert!(
            started_at.elapsed() >= Duration::from_millis(70),
            "timeout response must wait for the blocking transaction to terminate"
        );

        let row_count = database
            .read("count_timeout_test_rows", |connection| {
                connection
                    .query_row("SELECT COUNT(*) FROM timeout_test", [], |row| {
                        row.get::<_, i64>(0)
                    })
                    .map_err(|error| sqlite_test_error("count rows", error))
            })
            .await
            .expect("count timeout test rows");
        assert_eq!(
            row_count, 0,
            "timed-out write must not commit in background"
        );

        fs::remove_file(&db_path).expect("remove timeout test database");
    }

    #[tokio::test]
    async fn timed_out_read_never_returns_a_late_success() {
        let db_path = unique_database_path("timeout-read");
        let database = Database::new(db_path.clone());
        database.initialize().await.expect("database init");
        let short_timeout_database = Database {
            db_path: db_path.clone(),
            operation_timeout: Duration::from_millis(20),
            init_timeout: Duration::from_secs(5),
        };

        let started_at = Instant::now();
        let error = short_timeout_database
            .read("slow_timeout_test_read", |_| {
                std::thread::sleep(Duration::from_millis(80));
                Ok(42_u32)
            })
            .await
            .expect_err("late read success must be reported as a timeout");

        assert_eq!(error.code, "DB_TIMEOUT");
        assert_eq!(error.retryable, Some(true));
        assert!(
            started_at.elapsed() >= Duration::from_millis(70),
            "timeout response must wait for the blocking read to terminate"
        );

        fs::remove_file(&db_path).expect("remove timeout read database");
    }
}
