use chrono::Utc;

use crate::{
    domain::{ensure_absolute_directory, AdapterAvailability, SnapshotCodexProfilePayload},
    error::{AppError, AppResult},
};

use super::process::{find_first_executable, resolve_configured_executable};

pub trait CodexAdapter {
    fn detect(&self, configured_path: Option<&str>) -> AdapterAvailability;
    fn build_launch_plan(
        &self,
        executable_path: &str,
        input: CodexLaunchInput<'_>,
    ) -> AppResult<CodexLaunchPlan>;
}

#[derive(Debug, Clone, Copy)]
pub struct CodexLaunchInput<'a> {
    pub profile: &'a SnapshotCodexProfilePayload,
    pub startup_mode_override: Option<&'a str>,
    pub extra_args: &'a [String],
}

#[derive(Debug, Clone)]
pub struct CodexLaunchPlan {
    pub terminal_command: Option<Vec<String>>,
    pub envs: Vec<(String, String)>,
    pub executable_path: String,
}

#[derive(Debug, Default, Clone)]
pub struct DefaultCodexAdapter;

impl CodexAdapter for DefaultCodexAdapter {
    fn detect(&self, configured_path: Option<&str>) -> AdapterAvailability {
        let detected_at = Utc::now().to_rfc3339();
        if let Some(raw_path) = configured_path.filter(|value| !value.trim().is_empty()) {
            return match resolve_configured_executable(
                raw_path,
                &["codex.exe", "codex.cmd", "codex.bat"],
                "CODEX_PATH_INVALID",
                "Codex CLI",
            ) {
                Ok(path) => AdapterAvailability {
                    key: "codex".to_string(),
                    label: "Codex CLI".to_string(),
                    available: true,
                    status: "available".to_string(),
                    executable_path: Some(path.display().to_string()),
                    source: "user_config".to_string(),
                    message: format!("已使用用户配置的 Codex CLI 路径 · {}", detected_at),
                },
                Err(error) => AdapterAvailability {
                    key: "codex".to_string(),
                    label: "Codex CLI".to_string(),
                    available: false,
                    status: "invalid".to_string(),
                    executable_path: Some(raw_path.trim().to_string()),
                    source: "user_config".to_string(),
                    message: format!(
                        "Codex CLI 配置路径无效：{} · {}",
                        error.message, detected_at
                    ),
                },
            };
        }

        match find_first_executable(&["codex.cmd", "codex.exe", "codex"]) {
            Some(path) => AdapterAvailability {
                key: "codex".to_string(),
                label: "Codex CLI".to_string(),
                available: true,
                status: "available".to_string(),
                executable_path: Some(path.display().to_string()),
                source: "PATH".to_string(),
                message: format!("已探测到 Codex CLI · {}", detected_at),
            },
            None => AdapterAvailability {
                key: "codex".to_string(),
                label: "Codex CLI".to_string(),
                available: false,
                status: "missing".to_string(),
                executable_path: None,
                source: "PATH".to_string(),
                message: format!("未在 PATH 中找到 codex · {}", detected_at),
            },
        }
    }

    fn build_launch_plan(
        &self,
        executable_path: &str,
        input: CodexLaunchInput<'_>,
    ) -> AppResult<CodexLaunchPlan> {
        let codex_home =
            ensure_absolute_directory(&input.profile.codex_home, "INVALID_CODEX_HOME")?;

        if executable_path.trim().is_empty() {
            return Err(AppError::new(
                "CODEX_ADAPTER_UNAVAILABLE",
                "codex executable path is required",
            ));
        }

        let startup_mode = input
            .startup_mode_override
            .unwrap_or(input.profile.startup_mode.as_str());
        let terminal_command = match startup_mode {
            "terminal_only" => None,
            "run_codex" => {
                let mut command = vec![
                    "cmd.exe".to_string(),
                    "/K".to_string(),
                    executable_path.to_string(),
                ];
                command.extend(input.profile.default_args.iter().cloned());
                command.extend(input.extra_args.iter().cloned());
                Some(command)
            }
            "resume_last" => {
                let mut command = vec![
                    "cmd.exe".to_string(),
                    "/K".to_string(),
                    executable_path.to_string(),
                    "resume".to_string(),
                    "--last".to_string(),
                ];
                command.extend(input.extra_args.iter().cloned());
                Some(command)
            }
            other => {
                return Err(AppError::new(
                    "CODEX_STARTUP_MODE_INVALID",
                    "unsupported codex startup mode",
                )
                .with_detail("startupMode", other.to_string()))
            }
        };

        Ok(CodexLaunchPlan {
            terminal_command,
            envs: vec![("CODEX_HOME".to_string(), codex_home)],
            executable_path: executable_path.to_string(),
        })
    }
}
