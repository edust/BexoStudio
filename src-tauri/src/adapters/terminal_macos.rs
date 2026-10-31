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
        // Terminal's dictionary exposes do script but no tab constructor. Invoke its menu action
        // through AX, then identify the new tab by its TTY (native tabs can have separate window IDs).
        // Match the menu shortcut attributes rather than localized menu/profile titles.
        // Group identity lives in the first tab's custom title; never send commands into an existing busy tab.
        let script = r#"on clickNewTab(theMenu, remainingDepth)
tell application "System Events"
 repeat with candidate in menu items of theMenu
  if enabled of candidate then
   if (exists attribute "AXMenuItemCmdChar" of candidate) and (exists attribute "AXMenuItemCmdModifiers" of candidate) then
    if (value of attribute "AXMenuItemCmdChar" of candidate is "t") and (value of attribute "AXMenuItemCmdModifiers" of candidate is 0) then
     click candidate
     return true
    end if
   end if
   if remainingDepth > 0 and (exists menu 1 of candidate) then
    if my clickNewTab(menu 1 of candidate, remainingDepth - 1) then return true
   end if
  end if
 end repeat
end tell
return false
end clickNewTab

on run argv
try
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
     set targetWindow to window id (id of w)
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
   set frontmost of targetWindow to true
   repeat 100 times
    if frontmost and (frontmost of targetWindow) then exit repeat
    delay 0.05
   end repeat
   if not (frontmost and (frontmost of targetWindow)) then error "Terminal 窗口未能进入前台" number -2701
   set previousTtys to {}
   repeat with existingWindow in windows
    repeat with existingTab in tabs of existingWindow
     set end of previousTtys to tty of existingTab
    end repeat
   end repeat
   tell application "System Events"
    tell process "Terminal"
     set frontmost to true
     set openedTab to false
     repeat with topMenu in menu bar items of menu bar 1
      if exists menu 1 of topMenu then
       if my clickNewTab(menu 1 of topMenu, 1) then
        set openedTab to true
        exit repeat
       end if
      end if
     end repeat
     if not openedTab then error "未找到 Terminal 新建标签页菜单" number -2702
    end tell
   end tell
   set newTab to missing value
   repeat 100 times
    set candidateWindow to window id (id of front window)
    set candidateTab to selected tab of candidateWindow
    set candidateTty to tty of candidateTab
    if candidateTty is not "" and candidateTty is not in previousTtys then
     set newTab to candidateTab
     exit repeat
    end if
    delay 0.05
   end repeat
   if newTab is missing value then error "无法识别新建的 Terminal 标签页" number -2700
   do script commandText in newTab
   set custom title of newTab to tabTitle
  end if
 end tell
end timeout
on error ignoredMessage number errorNumber
 error "BEXO_TERMINAL_AUTOMATION" number errorNumber
end try
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

#[cfg(test)]
mod macos_execution_tests {
    use super::*;
    #[test]
    fn shell_quoting_round_trips_literal_arguments() {
        let values = ["a b", "a'b", "$HOME; echo wrong", "", "中文", "line\nbreak"];
        let script = format!(
            "printf '%s\\0' {}",
            values
                .iter()
                .map(|value| quote_shell(value))
                .collect::<Vec<_>>()
                .join(" ")
        );
        let output = std::process::Command::new("/bin/zsh")
            .args(["-f", "-c", &script])
            .output()
            .unwrap();
        assert!(output.status.success());
        let expected = values
            .iter()
            .flat_map(|value| value.as_bytes().iter().copied().chain(std::iter::once(0)))
            .collect::<Vec<_>>();
        assert_eq!(output.stdout, expected);
    }
    #[test]
    fn macos_shell_plan_uses_login_zsh_and_keeps_interactive_terminal_open() {
        let plan = MacTerminalAdapter
            .resolve_shell_launch_plan(TerminalCommandShell::Cmd, "printf hello")
            .unwrap();
        assert_eq!(plan.executable_path, PathBuf::from("/bin/zsh"));
        assert_eq!(plan.run_args, ["-lc", "printf hello"]);
        assert_eq!(
            plan.startup_args,
            ["-lic", "printf hello\nexec /bin/zsh -l"]
        );
    }
}
