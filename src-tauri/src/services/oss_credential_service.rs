#[cfg(not(target_os = "macos"))]
use std::time::Duration;

use tauri::async_runtime::{spawn_blocking, JoinHandle};

use crate::{
    adapters::OssCredentialStore,
    domain::OssCredential,
    error::{AppError, AppResult},
};

#[cfg(not(target_os = "macos"))]
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
    // Keychain Services may wait for a system unlock/access dialog and exposes no
    // cancellation API. Await its real result: dropping a timed-out blocking task
    // could otherwise report failure while a credential write succeeds later.
    #[cfg(target_os = "macos")]
    let joined = handle.await;
    #[cfg(not(target_os = "macos"))]
    let joined = match tokio::time::timeout(CREDENTIAL_IO_TIMEOUT, handle).await {
        Ok(result) => result,
        Err(_) => {
            return Err(AppError::new(
                "OSS_CREDENTIAL_STORE_TIMEOUT",
                "secure OSS credential operation timed out",
            )
            .with_detail("operation", operation_name)
            .retryable(true))
        }
    };
    match joined {
        Ok(result) => result,
        Err(error) => Err(AppError::new(
            "OSS_CREDENTIAL_STORE_UNAVAILABLE",
            "secure OSS credential operation failed",
        )
        .with_detail("operation", operation_name)
        .with_detail("reason", error.to_string())),
    }
}
