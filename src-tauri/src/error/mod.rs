use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Clone, Error, Serialize, Deserialize)]
#[error("{code}: {message}")]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum CommandResponse<T> {
    Success {
        ok: bool,
        data: T,
        #[serde(skip_serializing_if = "Option::is_none")]
        run_id: Option<String>,
    },
    Failure {
        ok: bool,
        error: AppError,
    },
}

impl AppError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
            retryable: None,
        }
    }

    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let mut details = self.details.unwrap_or_default();
        details.insert(key.into(), value.into());
        self.details = Some(details);
        self
    }

    pub fn retryable(mut self, retryable: bool) -> Self {
        self.retryable = Some(retryable);
        self
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new("VALIDATION_ERROR", message)
    }

    pub fn plugin_init(plugin: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::new(
            "PLUGIN_INIT_FAILED",
            format!("plugin init failed: {}", plugin.into()),
        )
        .with_detail("reason", reason.into())
    }

    pub fn tray_setup(reason: impl Into<String>) -> Self {
        Self::new("TRAY_SETUP_FAILED", "tray setup failed").with_detail("reason", reason.into())
    }

    pub fn window_not_found(label: impl Into<String>) -> Self {
        Self::new("WINDOW_NOT_FOUND", "window not found").with_detail("label", label.into())
    }

    pub fn window_action(reason: impl Into<String>) -> Self {
        Self::new("WINDOW_ACTION_FAILED", "window action failed")
            .with_detail("reason", reason.into())
    }
}

impl<T> CommandResponse<T> {
    pub fn success(data: T) -> Self {
        Self::Success {
            ok: true,
            data,
            run_id: None,
        }
    }

    pub fn failure(error: AppError) -> Self {
        Self::Failure { ok: false, error }
    }
}
