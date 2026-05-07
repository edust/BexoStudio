use serde::{Deserialize, Deserializer, Serialize};

pub const DEFAULT_SCREENSHOT_CAPTURE_HOTKEY: &str = "Ctrl+Shift+X";
pub const PREVIOUS_DEFAULT_SCREENSHOT_CAPTURE_HOTKEY: &str = "Ctrl+Shift+1";
pub const EARLIER_DEFAULT_SCREENSHOT_CAPTURE_HOTKEY: &str = "Ctrl+Shift+4";
pub const LEGACY_SCREENSHOT_CAPTURE_HOTKEY: &str = "Ctrl+Alt+A";
pub const DEFAULT_CODEX_HISTORY_MESSAGE_FONT_SIZE: i32 = 12;
pub const DEFAULT_CODEX_AUTH_QUOTA_REFRESH_INTERVAL_SECONDS: i32 = 60;
pub const DEFAULT_CODEX_AUTH_PROXY_MODE: &str = "system";
pub const DEFAULT_TERMINAL_COMMAND_SHELL: TerminalCommandShell = TerminalCommandShell::PowerShell7;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct AppPreferences {
    pub terminal: TerminalPreferences,
    pub ide: IdePreferences,
    pub workspace: WorkspacePreferences,
    pub startup: StartupPreferences,
    pub hotkey: HotkeyPreferences,
    pub tray: TrayPreferences,
    pub diagnostics: DiagnosticsPreferences,
    pub codex_history: CodexHistoryViewPreferences,
    pub codex_auth: CodexAuthPreferences,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexHomeDirectoryInfo {
    pub path: Option<String>,
    pub source: String,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TerminalPreferences {
    pub windows_terminal_path: Option<String>,
    pub codex_cli_path: Option<String>,
    #[serde(
        default = "default_terminal_command_shell",
        deserialize_with = "deserialize_terminal_command_shell"
    )]
    pub command_shell: TerminalCommandShell,
    pub command_templates: Vec<TerminalCommandTemplate>,
}

impl Default for TerminalPreferences {
    fn default() -> Self {
        Self {
            windows_terminal_path: None,
            codex_cli_path: None,
            command_shell: DEFAULT_TERMINAL_COMMAND_SHELL,
            command_templates: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalCommandShell {
    #[serde(rename = "powershell7")]
    PowerShell7,
    #[serde(rename = "cmd")]
    Cmd,
}

impl Default for TerminalCommandShell {
    fn default() -> Self {
        DEFAULT_TERMINAL_COMMAND_SHELL
    }
}

fn default_terminal_command_shell() -> TerminalCommandShell {
    DEFAULT_TERMINAL_COMMAND_SHELL
}

fn deserialize_terminal_command_shell<'de, D>(
    deserializer: D,
) -> Result<TerminalCommandShell, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    Ok(match value.as_deref().map(str::trim) {
        Some("cmd") => TerminalCommandShell::Cmd,
        Some("powershell7") => TerminalCommandShell::PowerShell7,
        _ => DEFAULT_TERMINAL_COMMAND_SHELL,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct TerminalCommandTemplate {
    pub id: String,
    pub name: String,
    pub command_line: String,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct IdePreferences {
    pub vscode_path: Option<String>,
    pub jetbrains_path: Option<String>,
    pub custom_editors: Vec<CustomEditorPreference>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct CustomEditorPreference {
    pub id: String,
    pub name: String,
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct WorkspacePreferences {
    pub selected_workspace_ids: Vec<String>,
    pub pinned_workspace_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct StartupPreferences {
    pub launch_at_login: bool,
    pub start_silently: bool,
}

fn default_screenshot_capture_hotkey() -> String {
    DEFAULT_SCREENSHOT_CAPTURE_HOTKEY.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HotkeyPreferences {
    #[serde(default = "default_screenshot_capture_hotkey")]
    pub screenshot_capture: String,
    pub voice_input_toggle: Option<String>,
    pub voice_input_hold: Option<String>,
}

impl Default for HotkeyPreferences {
    fn default() -> Self {
        Self {
            screenshot_capture: default_screenshot_capture_hotkey(),
            voice_input_toggle: None,
            voice_input_hold: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TrayPreferences {
    pub close_to_tray: bool,
    pub show_recent_workspaces: bool,
}

impl Default for TrayPreferences {
    fn default() -> Self {
        Self {
            close_to_tray: true,
            show_recent_workspaces: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DiagnosticsPreferences {
    pub show_adapter_sources: bool,
    pub show_executable_paths: bool,
}

impl Default for DiagnosticsPreferences {
    fn default() -> Self {
        Self {
            show_adapter_sources: true,
            show_executable_paths: true,
        }
    }
}

fn default_codex_history_message_font_size() -> i32 {
    DEFAULT_CODEX_HISTORY_MESSAGE_FONT_SIZE
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CodexHistoryViewPreferences {
    pub message_font_family: String,
    #[serde(default = "default_codex_history_message_font_size")]
    pub message_font_size: i32,
}

impl Default for CodexHistoryViewPreferences {
    fn default() -> Self {
        Self {
            message_font_family: String::new(),
            message_font_size: default_codex_history_message_font_size(),
        }
    }
}

fn default_codex_auth_quota_refresh_interval_seconds() -> i32 {
    DEFAULT_CODEX_AUTH_QUOTA_REFRESH_INTERVAL_SECONDS
}

fn default_codex_auth_proxy_mode() -> String {
    DEFAULT_CODEX_AUTH_PROXY_MODE.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CodexAuthPreferences {
    #[serde(default = "default_codex_auth_quota_refresh_interval_seconds")]
    pub quota_refresh_interval_seconds: i32,
    pub proxy: CodexAuthProxyPreferences,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CodexAuthProxyPreferences {
    #[serde(default = "default_codex_auth_proxy_mode")]
    pub mode: String,
    pub manual_proxy_url: String,
}

impl Default for CodexAuthPreferences {
    fn default() -> Self {
        Self {
            quota_refresh_interval_seconds: default_codex_auth_quota_refresh_interval_seconds(),
            proxy: CodexAuthProxyPreferences::default(),
        }
    }
}

impl Default for CodexAuthProxyPreferences {
    fn default() -> Self {
        Self {
            mode: default_codex_auth_proxy_mode(),
            manual_proxy_url: String::new(),
        }
    }
}
