use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

use super::{require_non_empty, validate_optional_uuid};

const SUPPORTED_LAUNCH_TASK_TYPES: &[&str] = &["terminal_command", "open_path", "codex", "ide"];
const SUPPORTED_LAUNCH_TASK_IDE_TARGETS: &[&str] = &["vscode", "jetbrains"];
const SUPPORTED_CODEX_LAUNCH_MODES: &[&str] = &[
    "inherit_profile",
    "terminal_only",
    "run_codex",
    "resume_last",
];
const DEFAULT_TIMEOUT_MS: i64 = 30_000;
const MIN_TIMEOUT_MS: i64 = 500;
const MAX_TIMEOUT_MS: i64 = 300_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct LaunchTaskRetryPolicy {
    pub max_attempts: i64,
    pub backoff_ms: i64,
}

impl Default for LaunchTaskRetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 1,
            backoff_ms: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchTaskRecord {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub task_type: String,
    pub enabled: bool,
    pub command: String,
    pub args: Vec<String>,
    pub working_dir: String,
    pub timeout_ms: i64,
    pub continue_on_failure: bool,
    pub retry_policy: LaunchTaskRetryPolicy,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotLaunchTaskPayload {
    pub id: String,
    pub name: String,
    pub task_type: String,
    pub enabled: bool,
    pub command: String,
    pub args: Vec<String>,
    pub working_dir: String,
    pub timeout_ms: i64,
    pub continue_on_failure: bool,
    pub retry_policy: LaunchTaskRetryPolicy,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertLaunchTaskInput {
    pub id: Option<String>,
    pub project_id: String,
    pub name: String,
    pub task_type: String,
    pub enabled: Option<bool>,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub working_dir: Option<String>,
    pub timeout_ms: Option<i64>,
    pub continue_on_failure: Option<bool>,
    pub retry_policy: Option<LaunchTaskRetryPolicy>,
    pub sort_order: Option<i64>,
}

pub fn validate_launch_task_type(value: &str) -> AppResult<String> {
    let normalized = require_non_empty("taskType", value, 40)?;
    if !SUPPORTED_LAUNCH_TASK_TYPES.contains(&normalized.as_str()) {
        return Err(AppError::validation(
            "taskType must be one of terminal_command, open_path, codex, ide",
        )
        .with_detail("taskType", normalized));
    }
    Ok(normalized)
}

pub fn validate_launch_task_timeout(value: Option<i64>) -> AppResult<i64> {
    let timeout_ms = value.unwrap_or(DEFAULT_TIMEOUT_MS);
    if !(MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&timeout_ms) {
        return Err(AppError::validation(format!(
            "timeoutMs must be between {MIN_TIMEOUT_MS} and {MAX_TIMEOUT_MS}"
        ))
        .with_detail("timeoutMs", timeout_ms.to_string()));
    }
    Ok(timeout_ms)
}

pub fn validate_launch_task_retry_policy(
    value: Option<LaunchTaskRetryPolicy>,
) -> AppResult<LaunchTaskRetryPolicy> {
    let policy = value.unwrap_or_default();
    if policy.max_attempts != 1 {
        return Err(AppError::validation(
            "retryPolicy.maxAttempts must be 1 for launch tasks in v1",
        )
        .with_detail("maxAttempts", policy.max_attempts.to_string()));
    }
    if policy.backoff_ms != 0 {
        return Err(
            AppError::validation("retryPolicy.backoffMs must be 0 for launch tasks in v1")
                .with_detail("backoffMs", policy.backoff_ms.to_string()),
        );
    }
    Ok(policy)
}

pub fn validate_launch_task_args(args: Vec<String>) -> AppResult<Vec<String>> {
    if args.len() > 24 {
        return Err(AppError::validation("args cannot exceed 24 items"));
    }

    let mut normalized = Vec::with_capacity(args.len());
    for raw in args {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(AppError::validation("args cannot contain empty values"));
        }
        if trimmed.len() > 240 {
            return Err(AppError::validation(
                "each arg cannot exceed 240 characters",
            ));
        }
        normalized.push(trimmed.to_string());
    }
    Ok(normalized)
}

pub fn validate_launch_task_working_dir(value: Option<String>) -> AppResult<String> {
    let Some(raw) = value else {
        return Ok(String::new());
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    let path = std::path::Path::new(trimmed);
    if !path.is_absolute() {
        return Err(AppError::new(
            "INVALID_LAUNCH_TASK_WORKING_DIR",
            "working directory must be an absolute path",
        )
        .with_detail("workingDir", trimmed.to_string()));
    }
    let metadata = std::fs::metadata(path).map_err(|error| {
        AppError::new(
            "INVALID_LAUNCH_TASK_WORKING_DIR",
            "working directory does not exist",
        )
        .with_detail("workingDir", trimmed.to_string())
        .with_detail("reason", error.to_string())
    })?;
    if !metadata.is_dir() {
        return Err(AppError::new(
            "INVALID_LAUNCH_TASK_WORKING_DIR",
            "working directory must point to a directory",
        )
        .with_detail("workingDir", trimmed.to_string()));
    }
    Ok(trimmed.to_string())
}

pub fn validate_launch_task_command(task_type: &str, command: &str) -> AppResult<String> {
    let normalized = require_non_empty("command", command, 512)?;
    match task_type {
        "open_path" => {
            let path = std::path::Path::new(normalized.as_str());
            if !path.is_absolute() {
                return Err(AppError::new(
                    "INVALID_LAUNCH_TASK_PATH",
                    "open_path command must be an absolute path",
                )
                .with_detail("command", normalized));
            }
            let exists = std::fs::metadata(path).is_ok();
            if !exists {
                return Err(AppError::new(
                    "INVALID_LAUNCH_TASK_PATH",
                    "open_path target does not exist",
                )
                .with_detail("command", normalized));
            }
            Ok(normalized)
        }
        "ide" => {
            if !SUPPORTED_LAUNCH_TASK_IDE_TARGETS.contains(&normalized.as_str()) {
                return Err(
                    AppError::validation("ide command must be vscode or jetbrains")
                        .with_detail("command", normalized),
                );
            }
            Ok(normalized)
        }
        "codex" => {
            if !SUPPORTED_CODEX_LAUNCH_MODES.contains(&normalized.as_str()) {
                return Err(AppError::validation(
                    "codex command must be inherit_profile, terminal_only, run_codex, or resume_last",
                )
                .with_detail("command", normalized));
            }
            Ok(normalized)
        }
        _ => Ok(normalized),
    }
}

pub fn validate_launch_task_id(value: Option<String>) -> AppResult<Option<String>> {
    validate_optional_uuid("id", value)
}
