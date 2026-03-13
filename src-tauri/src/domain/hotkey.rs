use serde::{Deserialize, Serialize};

pub const HOTKEY_TRIGGER_EVENT_NAME: &str = "hotkey://trigger";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HotkeyAction {
    ScreenshotCapture,
    VoiceInputToggle,
    VoiceInputHold,
}

impl HotkeyAction {
    pub fn key(self) -> &'static str {
        match self {
            Self::ScreenshotCapture => "screenshot_capture",
            Self::VoiceInputToggle => "voice_input_toggle",
            Self::VoiceInputHold => "voice_input_hold",
        }
    }

    pub fn preference_field(self) -> &'static str {
        match self {
            Self::ScreenshotCapture => "hotkey.screenshotCapture",
            Self::VoiceInputToggle => "hotkey.voiceInputToggle",
            Self::VoiceInputHold => "hotkey.voiceInputHold",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyTriggerEvent {
    pub action: HotkeyAction,
    pub shortcut: String,
    pub triggered_at: String,
    pub source: String,
}
