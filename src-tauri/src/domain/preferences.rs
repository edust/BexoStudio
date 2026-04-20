use serde::{Deserialize, Serialize};

pub const DEFAULT_SCREENSHOT_CAPTURE_HOTKEY: &str = "Ctrl+Shift+X";
pub const PREVIOUS_DEFAULT_SCREENSHOT_CAPTURE_HOTKEY: &str = "Ctrl+Shift+1";
pub const EARLIER_DEFAULT_SCREENSHOT_CAPTURE_HOTKEY: &str = "Ctrl+Shift+4";
pub const LEGACY_SCREENSHOT_CAPTURE_HOTKEY: &str = "Ctrl+Alt+A";

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexHomeDirectoryInfo {
    pub path: Option<String>,
    pub source: String,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct TerminalPreferences {
    pub windows_terminal_path: Option<String>,
    pub codex_cli_path: Option<String>,
    pub command_templates: Vec<TerminalCommandTemplate>,
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
