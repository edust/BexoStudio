//! Apple Terminal adapter. Commands are passed as AppleScript argv, never source text.
use super::{
    LaunchCommand, TerminalAdapter, TerminalLaunchInput, WindowsShellLaunchPlan,
    WindowsTerminalTabLaunchInput,
};
use crate::{
    domain::{AdapterAvailability, TerminalCommandShell},
    error::AppResult,
};
use std::{path::PathBuf, time::Duration};

#[derive(Debug, Default, Clone)]
pub struct MacTerminalAdapter;

pub fn quote_shell(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

impl TerminalAdapter for MacTerminalAdapter {
    fn detect(&self, _configured_path: Option<&str>) -> AdapterAvailability {
        let available =
            std::path::Path::new("/System/Applications/Utilities/Terminal.app").is_dir();
        AdapterAvailability {
            key: "windows_terminal".into(), // Existing IPC capability key, shared by both platforms.
            label: "Terminal".into(),
            available,
            status: if available { "available" } else { "missing" }.into(),
            executable_path: available.then(|| "/usr/bin/osascript".into()),
            source: "system".into(),
            message: if available {
                "使用 macOS Terminal；首次启动需允许自动化，批量标签页需要辅助功能权限"
            } else {
                "未找到 macOS Terminal"
            }
            .into(),
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

impl MacTerminalAdapter {
    pub fn resolve_shell_launch_plan(
        &self,
        _preferred: TerminalCommandShell,
        command: &str,
    ) -> AppResult<WindowsShellLaunchPlan> {
        Ok(WindowsShellLaunchPlan {
            shell: TerminalCommandShell::Zsh,
            executable_path: PathBuf::from("/bin/zsh"),
            startup_args: vec!["-lic".into(), format!("{command}\nexec /bin/zsh -l")],
            run_args: vec!["-lc".into(), command.into()],
        })
    }

    pub fn build_tab_launch_plan(
        &self,
        _executable_path: &str,
        input: WindowsTerminalTabLaunchInput,
    ) -> AppResult<LaunchCommand> {
        let mut line = format!("cd -- {}", quote_shell(&input.project_path));
        for (key, value) in input.envs {
            line.push_str(&format!(
                " && export {}",
                quote_shell(&format!("{key}={value}"))
            ));
        }
        if let Some(command) = input.startup_command {
            line.push_str(" && ");
            line.push_str(
                &command
                    .iter()
                    .map(|arg| quote_shell(arg))
                    .collect::<Vec<_>>()
                    .join(" "),
            );
        }
        // Terminal's dictionary exposes do script, but creating a tab requires its menu shortcut.
        // Group identity lives in the first tab's custom title; never send commands into an existing busy tab.
        let script = r#"on run argv
set commandText to item 1 of argv
set groupName to item 2 of argv
set tabTitle to item 3 of argv
with timeout of 15 seconds
 tell application "Terminal"
  activate
  set targetWindow to missing value
  if groupName is not "" then
   repeat with w in windows
    if custom title of tab 1 of w is groupName then
     set targetWindow to w
     exit repeat
    end if
   end repeat
  end if
  if targetWindow is missing value then
   set newTab to do script commandText
   if groupName is not "" then
    set custom title of newTab to groupName
   else
    set custom title of newTab to tabTitle
   end if
  else
   set index of targetWindow to 1
   set previousCount to count of tabs of targetWindow
   tell application "System Events" to tell process "Terminal" to keystroke "t" using command down
   repeat 100 times
    if (count of tabs of targetWindow) > previousCount then exit repeat
    delay 0.05
   end repeat
   if (count of tabs of targetWindow) ≤ previousCount then error "无法创建 Terminal 标签页，请检查辅助功能权限"
   set newTab to selected tab of targetWindow
   do script commandText in newTab
   set custom title of newTab to tabTitle
  end if
 end tell
end timeout
end run"#;
        Ok(LaunchCommand {
            executable_path: PathBuf::from("/usr/bin/osascript"),
            args: vec![
                "-e".into(),
                script.into(),
                line,
                input
                    .window_target
                    .filter(|s| s != "new")
                    .unwrap_or_default(),
                input.title.unwrap_or_else(|| "Bexo Studio".into()),
            ],
            current_dir: None,
            envs: vec![],
            timeout: Duration::from_secs(20),
            tracking: None,
            retain_after_timeout: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shell_arguments_preserve_metacharacters_and_empty_values() {
        assert_eq!(quote_shell("a'b $HOME"), "'a'\\''b $HOME'");
        assert_eq!(quote_shell(""), "''");
    }
    #[test]
    fn startup_preserves_working_directory_and_profile() {
        let plan = MacTerminalAdapter
            .build_launch_plan(
                "",
                TerminalLaunchInput {
                    project_path: "/tmp/a b'c".into(),
                    startup_command: Some(vec!["codex".into(), "resume".into(), "--last".into()]),
                    envs: vec![("CODEX_HOME".into(), "/tmp/profile a".into())],
                },
            )
            .unwrap();
        assert_eq!(plan.args[2], "cd -- '/tmp/a b'\\''c' && export 'CODEX_HOME=/tmp/profile a' && 'codex' 'resume' '--last'");
        assert!(plan.args[1].contains("on run argv"));
    }
}
