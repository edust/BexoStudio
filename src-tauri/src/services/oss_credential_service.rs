use std::time::Duration;

use tauri::async_runtime::{spawn_blocking, JoinHandle};

use crate::{
    adapters::OssCredentialStore,
    domain::OssCredential,
    error::{AppError, AppResult},
};

const CREDENTIAL_IO_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Default)]
pub struct OssCredentialService {
    store: OssCredentialStore,
}

impl OssCredentialService {
    pub fn new() -> Self {
        Self {
            store: OssCredentialStore::new(),
        }
    }

    pub async fn read(&self, credential_ref: String) -> AppResult<OssCredential> {
        let store = self.store.clone();
        run_credential_operation("read_oss_credential", move || store.read(&credential_ref)).await
    }

    pub async fn write(&self, credential_ref: String, credential: OssCredential) -> AppResult<()> {
        let store = self.store.clone();
        run_credential_operation("write_oss_credential", move || {
            store.write(&credential_ref, &credential)
        })
        .await
    }

    pub async fn delete(&self, credential_ref: String) -> AppResult<()> {
        let store = self.store.clone();
        run_credential_operation("delete_oss_credential", move || {
            store.delete(&credential_ref)
        })
        .await
    }
}

async fn run_credential_operation<T, F>(operation_name: &'static str, operation: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> AppResult<T> + Send + 'static,
{
    let handle: JoinHandle<AppResult<T>> = spawn_blocking(operation);
    match tokio::time::timeout(CREDENTIAL_IO_TIMEOUT, handle).await {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => Err(AppError::new(
            "OSS_CREDENTIAL_STORE_UNAVAILABLE",
            "secure OSS credential operation failed",
        )
        .with_detail("operation", operation_name)
        .with_detail("reason", error.to_string())),
        Err(_) => Err(AppError::new(
            "OSS_CREDENTIAL_STORE_TIMEOUT",
            "secure OSS credential operation timed out",
        )
        .with_detail("operation", operation_name)
        .retryable(true)),
    }
}
