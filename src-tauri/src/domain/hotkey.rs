use serde::{Deserialize, Serialize};

use crate::error::AppError;

pub const HOTKEY_TRIGGER_EVENT_NAME: &str = "hotkey://trigger";
pub const PROMPT_QUICK_PASTE_RESULT_EVENT_NAME: &str = "hotkey://prompt-quick-paste-result";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HotkeyAction {
    ScreenshotCapture,
    VoiceInputToggle,
    VoiceInputHold,
    PromptQuickPaste1,
    PromptQuickPaste2,
    PromptQuickPaste3,
    PromptQuickPaste4,
    PromptQuickPaste5,
}

impl HotkeyAction {
    pub fn key(self) -> &'static str {
        match self {
            Self::ScreenshotCapture => "screenshot_capture",
            Self::VoiceInputToggle => "voice_input_toggle",
            Self::VoiceInputHold => "voice_input_hold",
            Self::PromptQuickPaste1 => "prompt_quick_paste_1",
            Self::PromptQuickPaste2 => "prompt_quick_paste_2",
            Self::PromptQuickPaste3 => "prompt_quick_paste_3",
            Self::PromptQuickPaste4 => "prompt_quick_paste_4",
            Self::PromptQuickPaste5 => "prompt_quick_paste_5",
        }
    }

    pub fn preference_field(self) -> &'static str {
        match self {
            Self::ScreenshotCapture => "hotkey.screenshotCapture",
            Self::VoiceInputToggle => "hotkey.voiceInputToggle",
            Self::VoiceInputHold => "hotkey.voiceInputHold",
            Self::PromptQuickPaste1 => "hotkey.promptQuickPasteSlots[0].shortcut",
            Self::PromptQuickPaste2 => "hotkey.promptQuickPasteSlots[1].shortcut",
            Self::PromptQuickPaste3 => "hotkey.promptQuickPasteSlots[2].shortcut",
            Self::PromptQuickPaste4 => "hotkey.promptQuickPasteSlots[3].shortcut",
            Self::PromptQuickPaste5 => "hotkey.promptQuickPasteSlots[4].shortcut",
        }
    }

    pub fn prompt_quick_paste_slot(self) -> Option<u8> {
        match self {
            Self::PromptQuickPaste1 => Some(1),
            Self::PromptQuickPaste2 => Some(2),
            Self::PromptQuickPaste3 => Some(3),
            Self::PromptQuickPaste4 => Some(4),
            Self::PromptQuickPaste5 => Some(5),
            _ => None,
        }
    }

    pub fn from_prompt_quick_paste_slot(slot: u8) -> Option<Self> {
        match slot {
            1 => Some(Self::PromptQuickPaste1),
            2 => Some(Self::PromptQuickPaste2),
            3 => Some(Self::PromptQuickPaste3),
            4 => Some(Self::PromptQuickPaste4),
            5 => Some(Self::PromptQuickPaste5),
            _ => None,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptQuickPasteResultStatus {
    Sent,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptQuickPasteResultEvent {
    pub slot: u8,
    pub shortcut: String,
    pub status: PromptQuickPasteResultStatus,
    pub prompt_id: Option<String>,
    pub prompt_title: Option<String>,
    pub message: String,
    pub error: Option<AppError>,
    pub occurred_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HotkeyHealthStatus {
    Uninitialized,
    Ready,
    Degraded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyRegisteredBindingView {
    pub action: HotkeyAction,
    pub shortcut: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyHealth {
    pub status: HotkeyHealthStatus,
    pub initialized: bool,
    pub registered_bindings: Vec<HotkeyRegisteredBindingView>,
    pub last_error: Option<AppError>,
    pub updated_at: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_quick_paste_actions_map_all_five_slots() {
        for slot in 1..=5 {
            let action = HotkeyAction::from_prompt_quick_paste_slot(slot)
                .expect("slot must map to an action");
            assert_eq!(action.prompt_quick_paste_slot(), Some(slot));
            assert_eq!(action.key(), format!("prompt_quick_paste_{slot}"));
        }
        assert!(HotkeyAction::from_prompt_quick_paste_slot(0).is_none());
        assert!(HotkeyAction::from_prompt_quick_paste_slot(6).is_none());
    }
}
