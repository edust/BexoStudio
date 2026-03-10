use std::{
    fs,
    path::{Path, PathBuf},
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
        run_blocking("initialize_database", self.init_timeout, move || {
            if let Some(parent) = db_path.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    AppError::new("DB_INIT_FAILED", "failed to create database directory")
                        .with_detail("path", parent.display().to_string())
                        .with_detail("reason", error.to_string())
                })?;
            }

            let connection = open_connection(&db_path)?;
            connection.execute_batch(schema::SCHEMA).map_err(|error| {
                AppError::new("DB_INIT_FAILED", "failed to initialize database schema")
                    .with_detail("reason", error.to_string())
            })?;
            Ok(())
        })
        .await
    }

    pub async fn read<T, F>(&self, operation_name: &'static str, operation: F) -> AppResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> AppResult<T> + Send + 'static,
    {
        let db_path = self.db_path.clone();
        run_blocking(operation_name, self.operation_timeout, move || {
            let connection = open_connection(&db_path)?;
            operation(&connection)
        })
        .await
    }

    pub async fn write<T, F>(&self, operation_name: &'static str, operation: F) -> AppResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> AppResult<T> + Send + 'static,
    {
        let db_path = self.db_path.clone();
        run_blocking(operation_name, self.operation_timeout, move || {
            let mut connection = open_connection(&db_path)?;
            operation(&mut connection)
        })
        .await
    }
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
            Err(error) => Err(AppError::new("DB_TASK_FAILED", "database task join failed")
                .with_detail("operation", operation_name)
                .with_detail("reason", error.to_string())),
        },
        Err(_) => Err(AppError::new("DB_TIMEOUT", "database operation timed out")
            .with_detail("operation", operation_name)
            .retryable(true)),
    }
}
