use std::{env, path::PathBuf, time::Duration};

use chrono::Utc;

use crate::{
    domain::AdapterAvailability,
    error::{AppError, AppResult},
};

use super::process::{find_first_executable, resolve_configured_executable, LaunchCommand};

pub fn build_windows_shell_command_line(command: &str, args: &[String]) -> String {
    let command = command.trim();
    if args.is_empty() {
        return command.to_string();
    }

    let mut segments = Vec::with_capacity(args.len() + 1);
    segments.push(quote_cmd_argument(command));
    segments.extend(args.iter().filter_map(|arg| {
        let trimmed = arg.trim();
        if trimmed.is_empty() {
            return None;
        }
        if is_cmd_control_operator(trimmed) {
            Some(trimmed.to_string())
        } else {
            Some(quote_cmd_argument(trimmed))
        }
    }));
    segments.join(" ")
}

pub fn build_windows_command_shell_startup_command(
    shell_executable: &str,
    command_line: &str,
) -> Vec<String> {
    vec![
        shell_executable.to_string(),
        "/D".to_string(),
        "/K".to_string(),
        command_line.to_string(),
    ]
}

pub fn build_windows_command_shell_run_args(command_line: &str) -> Vec<String> {
    vec!["/D".to_string(), "/C".to_string(), command_line.to_string()]
}

pub trait TerminalAdapter {
    fn detect(&self, configured_path: Option<&str>) -> AdapterAvailability;
    fn build_launch_plan(
        &self,
        executable_path: &str,
        input: TerminalLaunchInput,
    ) -> AppResult<LaunchCommand>;
}

#[derive(Debug, Clone)]
pub struct TerminalLaunchInput {
    pub project_path: String,
    pub startup_command: Option<Vec<String>>,
    pub envs: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct WindowsTerminalTabLaunchInput {
    pub project_path: String,
    pub startup_command: Option<Vec<String>>,
    pub envs: Vec<(String, String)>,
    pub window_target: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct WindowsTerminalAdapter;

impl TerminalAdapter for WindowsTerminalAdapter {
    fn detect(&self, configured_path: Option<&str>) -> AdapterAvailability {
        let detected_at = Utc::now().to_rfc3339();
        if let Some(raw_path) = configured_path.filter(|value| !value.trim().is_empty()) {
            return match resolve_configured_executable(
                raw_path,
                &["wt.exe", "wt.cmd", "wt.bat"],
                "WINDOWS_TERMINAL_PATH_INVALID",
                "Windows Terminal",
            ) {
                Ok(path) => AdapterAvailability {
                    key: "windows_terminal".to_string(),
                    label: "Windows Terminal".to_string(),
                    available: true,
                    status: "available".to_string(),
                    executable_path: Some(path.display().to_string()),
                    source: "user_config".to_string(),
                    message: format!("已使用用户配置的 Windows Terminal 路径 · {}", detected_at),
                },
                Err(error) => AdapterAvailability {
                    key: "windows_terminal".to_string(),
                    label: "Windows Terminal".to_string(),
                    available: false,
                    status: "invalid".to_string(),
                    executable_path: Some(raw_path.trim().to_string()),
                    source: "user_config".to_string(),
                    message: format!(
                        "Windows Terminal 配置路径无效：{} · {}",
                        error.message, detected_at
                    ),
                },
            };
        }

        match find_first_executable(&["wt.exe", "wt"]) {
            Some(path) => AdapterAvailability {
                key: "windows_terminal".to_string(),
                label: "Windows Terminal".to_string(),
                available: true,
                status: "available".to_string(),
                executable_path: Some(path.display().to_string()),
                source: "PATH".to_string(),
                message: format!("已探测到 Windows Terminal · {}", detected_at),
            },
            None => AdapterAvailability {
                key: "windows_terminal".to_string(),
                label: "Windows Terminal".to_string(),
                available: false,
                status: "missing".to_string(),
                executable_path: None,
                source: "PATH".to_string(),
                message: format!("未在 PATH 中找到 wt.exe · {}", detected_at),
            },
        }
    }

    fn build_launch_plan(
        &self,
        executable_path: &str,
        input: TerminalLaunchInput,
    ) -> AppResult<LaunchCommand> {
        self.build_tab_launch_plan(
            executable_path,
            WindowsTerminalTabLaunchInput {
                project_path: input.project_path,
                startup_command: input.startup_command,
                envs: input.envs,
                window_target: None,
                title: None,
            },
        )
    }
}

impl WindowsTerminalAdapter {
    pub fn build_tab_launch_plan(
        &self,
        executable_path: &str,
        input: WindowsTerminalTabLaunchInput,
    ) -> AppResult<LaunchCommand> {
        if executable_path.trim().is_empty() {
            return Err(AppError::new(
                "TERMINAL_ADAPTER_UNAVAILABLE",
                "terminal executable path is required",
            ));
        }

        let mut args = Vec::new();
        if let Some(window_target) = input
            .window_target
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            args.push("-w".to_string());
            args.push(window_target.to_string());
        }

        args.push("new-tab".to_string());
        if let Some(title) = input
            .title
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            args.push("--title".to_string());
            args.push(title.to_string());
            args.push("--suppressApplicationTitle".to_string());
        }
        args.push("-d".to_string());
        args.push(input.project_path.clone());
        if let Some(startup_command) = input.startup_command {
            args.extend(startup_command);
        }

        Ok(LaunchCommand {
            executable_path: PathBuf::from(executable_path),
            args,
            current_dir: None,
            envs: input.envs,
            timeout: Duration::from_millis(900),
            tracking: None,
            retain_after_timeout: false,
        })
    }

    pub fn detect_shell_executable(&self) -> Option<PathBuf> {
        env::var_os("COMSPEC")
            .map(PathBuf::from)
            .filter(|path| path.is_file())
            .or_else(|| find_first_executable(&["cmd.exe", "cmd"]))
    }
}

fn quote_cmd_argument(value: &str) -> String {
    if value.is_empty() {
        return "\"\"".to_string();
    }

    if !requires_cmd_quotes(value) {
        return value.to_string();
    }

    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    for character in value.chars() {
        if character == '"' {
            quoted.push_str("\\\"");
        } else {
            quoted.push(character);
        }
    }
    quoted.push('"');
    quoted
}

fn requires_cmd_quotes(value: &str) -> bool {
    value
        .chars()
        .any(|character| character.is_whitespace() || is_cmd_metacharacter(character))
}

fn is_cmd_metacharacter(character: char) -> bool {
    matches!(character, '&' | '|' | '<' | '>' | '^' | '(' | ')' | '"')
}

fn is_cmd_control_operator(value: &str) -> bool {
    matches!(value, "&&" | "||" | "|" | "&")
        || value.starts_with('>')
        || value.starts_with('<')
        || value.starts_with("2>")
}

#[cfg(test)]
mod tests {
    use super::{
        build_windows_command_shell_run_args, build_windows_command_shell_startup_command,
        build_windows_shell_command_line, WindowsTerminalAdapter, WindowsTerminalTabLaunchInput,
    };

    #[test]
    fn tab_launch_plan_uses_fixed_title_when_title_is_provided() {
        let adapter = WindowsTerminalAdapter;
        let launch_command = adapter
            .build_tab_launch_plan(
                r"C:\\Tools\\wt.exe",
                WindowsTerminalTabLaunchInput {
                    project_path: r"D:\\workspace\\demo".to_string(),
                    startup_command: Some(vec![
                        "pwsh.exe".to_string(),
                        "-NoExit".to_string(),
                        "-Command".to_string(),
                        "echo ready".to_string(),
                    ]),
                    envs: Vec::new(),
                    window_target: Some("new".to_string()),
                    title: Some("BexoStudio".to_string()),
                },
            )
            .expect("build tab launch plan");

        assert!(launch_command.args.contains(&"--title".to_string()));
        assert!(launch_command.args.contains(&"BexoStudio".to_string()));
        assert!(launch_command
            .args
            .contains(&"--suppressApplicationTitle".to_string()));
    }

    #[test]
    fn shell_command_line_preserves_raw_command_when_args_are_empty() {
        let command = r#"cd "D:\Desktop\a\StarExpoHub\go-lottery\backend" && go run ./cmd"#;

        assert_eq!(build_windows_shell_command_line(command, &[]), command);
    }

    #[test]
    fn shell_command_line_keeps_legacy_control_operators_unquoted() {
        assert_eq!(
            build_windows_shell_command_line(
                "cd",
                &[
                    r"D:\Desktop\a\StarExpoHub\go-lottery\backend".to_string(),
                    "&&".to_string(),
                    "go".to_string(),
                    "run".to_string(),
                    "./cmd".to_string(),
                ],
            ),
            r"cd D:\Desktop\a\StarExpoHub\go-lottery\backend && go run ./cmd"
        );
    }

    #[test]
    fn shell_command_line_quotes_legacy_arguments_for_cmd() {
        assert_eq!(
            build_windows_shell_command_line(
                r"C:\Tools\My App\tool.exe",
                &[r#"say "hi""#.to_string()],
            ),
            r#""C:\Tools\My App\tool.exe" "say \"hi\"""#
        );
    }

    #[test]
    fn command_shell_startup_uses_cmd_keep_open_semantics() {
        let command = r#"cd "D:\workspace\backend" && go run ./cmd"#;

        assert_eq!(
            build_windows_command_shell_startup_command("cmd.exe", command),
            vec![
                "cmd.exe".to_string(),
                "/D".to_string(),
                "/K".to_string(),
                command.to_string(),
            ]
        );
        assert_eq!(
            build_windows_command_shell_run_args(command),
            vec!["/D".to_string(), "/C".to_string(), command.to_string()]
        );
    }
}
