use std::{path::PathBuf, time::Duration};

use chrono::Utc;

use crate::{
    domain::AdapterAvailability,
    error::{AppError, AppResult},
};

use super::process::{find_first_executable, resolve_configured_executable, LaunchCommand};

pub trait IdeAdapter {
    fn detect(&self, configured_path: Option<&str>) -> AdapterAvailability;
    fn build_launch_plan(
        &self,
        executable_path: &str,
        project_path: &str,
    ) -> AppResult<LaunchCommand>;
}

#[derive(Debug, Default, Clone)]
pub struct VSCodeAdapter;

#[derive(Debug, Default, Clone)]
pub struct JetBrainsAdapter;

impl IdeAdapter for VSCodeAdapter {
    fn detect(&self, configured_path: Option<&str>) -> AdapterAvailability {
        let detected_at = Utc::now().to_rfc3339();
        if let Some(raw_path) = configured_path.filter(|value| !value.trim().is_empty()) {
            return match resolve_configured_executable(
                raw_path,
                &["code.cmd", "code.exe", "code.bat"],
                "VSCODE_PATH_INVALID",
                "VS Code",
            ) {
                Ok(path) => AdapterAvailability {
                    key: "vscode".to_string(),
                    label: "VS Code".to_string(),
                    available: true,
                    status: "available".to_string(),
                    executable_path: Some(path.display().to_string()),
                    source: "user_config".to_string(),
                    message: format!("已使用用户配置的 VS Code 路径 · {}", detected_at),
                },
                Err(error) => AdapterAvailability {
                    key: "vscode".to_string(),
                    label: "VS Code".to_string(),
                    available: false,
                    status: "invalid".to_string(),
                    executable_path: Some(raw_path.trim().to_string()),
                    source: "user_config".to_string(),
                    message: format!("VS Code 配置路径无效：{} · {}", error.message, detected_at),
                },
            };
        }

        match find_first_executable(&["code.cmd", "code.exe", "code"]) {
            Some(path) => AdapterAvailability {
                key: "vscode".to_string(),
                label: "VS Code".to_string(),
                available: true,
                status: "available".to_string(),
                executable_path: Some(path.display().to_string()),
                source: "PATH".to_string(),
                message: format!("已探测到 VS Code CLI · {}", detected_at),
            },
            None => AdapterAvailability {
                key: "vscode".to_string(),
                label: "VS Code".to_string(),
                available: false,
                status: "missing".to_string(),
                executable_path: None,
                source: "PATH".to_string(),
                message: format!("未在 PATH 中找到 code · {}", detected_at),
            },
        }
    }

    fn build_launch_plan(
        &self,
        executable_path: &str,
        project_path: &str,
    ) -> AppResult<LaunchCommand> {
        build_ide_launch_command(
            executable_path,
            vec!["-n".to_string(), project_path.to_string()],
        )
    }
}

impl IdeAdapter for JetBrainsAdapter {
    fn detect(&self, configured_path: Option<&str>) -> AdapterAvailability {
        let detected_at = Utc::now().to_rfc3339();
        if let Some(raw_path) = configured_path.filter(|value| !value.trim().is_empty()) {
            return match resolve_configured_executable(
                raw_path,
                &["idea64.exe", "idea.exe", "idea.cmd", "idea.bat"],
                "JETBRAINS_PATH_INVALID",
                "JetBrains IDE",
            ) {
                Ok(path) => AdapterAvailability {
                    key: "jetbrains".to_string(),
                    label: "JetBrains IDE".to_string(),
                    available: true,
                    status: "available".to_string(),
                    executable_path: Some(path.display().to_string()),
                    source: "user_config".to_string(),
                    message: format!("已使用用户配置的 JetBrains 路径 · {}", detected_at),
                },
                Err(error) => AdapterAvailability {
                    key: "jetbrains".to_string(),
                    label: "JetBrains IDE".to_string(),
                    available: false,
                    status: "invalid".to_string(),
                    executable_path: Some(raw_path.trim().to_string()),
                    source: "user_config".to_string(),
                    message: format!(
                        "JetBrains 配置路径无效：{} · {}",
                        error.message, detected_at
                    ),
                },
            };
        }

        match find_first_executable(&["idea64.exe", "idea.exe", "idea.cmd", "idea"]) {
            Some(path) => AdapterAvailability {
                key: "jetbrains".to_string(),
                label: "JetBrains IDE".to_string(),
                available: true,
                status: "available".to_string(),
                executable_path: Some(path.display().to_string()),
                source: "PATH".to_string(),
                message: format!("已探测到 JetBrains CLI · {}", detected_at),
            },
            None => AdapterAvailability {
                key: "jetbrains".to_string(),
                label: "JetBrains IDE".to_string(),
                available: false,
                status: "missing".to_string(),
                executable_path: None,
                source: "PATH".to_string(),
                message: format!("未在 PATH 中找到 idea / idea64.exe · {}", detected_at),
            },
        }
    }

    fn build_launch_plan(
        &self,
        executable_path: &str,
        project_path: &str,
    ) -> AppResult<LaunchCommand> {
        build_ide_launch_command(executable_path, vec![project_path.to_string()])
    }
}

fn build_ide_launch_command(executable_path: &str, args: Vec<String>) -> AppResult<LaunchCommand> {
    if executable_path.trim().is_empty() {
        return Err(AppError::new(
            "IDE_ADAPTER_UNAVAILABLE",
            "ide executable path is required",
        ));
    }

    Ok(LaunchCommand {
        executable_path: PathBuf::from(executable_path),
        args,
        current_dir: None,
        envs: Vec::new(),
        timeout: Duration::from_millis(900),
        tracking: None,
        retain_after_timeout: false,
    })
}
