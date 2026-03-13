use std::sync::{Arc, Mutex};

use chrono::Utc;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::{
    domain::{AppPreferences, HotkeyAction, HotkeyTriggerEvent, HOTKEY_TRIGGER_EVENT_NAME},
    error::{AppError, AppResult},
};

use super::screenshot_service::ScreenshotService;

#[derive(Debug, Clone)]
pub struct HotkeyService {
    state: Arc<Mutex<HotkeyServiceState>>,
}

#[derive(Debug, Default, Clone)]
struct HotkeyServiceState {
    registered: Vec<RegisteredHotkeyBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RegisteredHotkeyBinding {
    action: HotkeyAction,
    shortcut: String,
}

impl HotkeyService {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(HotkeyServiceState::default())),
        }
    }

    pub fn initialize<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        preferences: &AppPreferences,
    ) -> AppResult<()> {
        self.apply_preferences(app, preferences)
    }

    pub fn apply_preferences<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        preferences: &AppPreferences,
    ) -> AppResult<()> {
        let desired = build_desired_bindings(preferences);
        let previous = self.current_registered()?;
        if previous == desired {
            return Ok(());
        }

        self.unregister_bindings(app, &previous);

        match self.register_bindings(app, &desired) {
            Ok(()) => {
                self.replace_registered(desired)?;
                Ok(())
            }
            Err(error) => {
                let rollback_result = self.register_bindings(app, &previous);
                if let Err(rollback_error) = rollback_result {
                    log::error!(
                        target: "bexo::service::hotkey",
                        "hotkey rollback failed: {}",
                        rollback_error
                    );
                    self.replace_registered(Vec::new())?;
                    return Err(error
                        .with_detail("rollback", "failed")
                        .with_detail("rollbackReason", rollback_error.to_string()));
                }

                self.replace_registered(previous)?;
                Err(error)
            }
        }
    }

    fn register_bindings<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        bindings: &[RegisteredHotkeyBinding],
    ) -> AppResult<()> {
        let mut registered: Vec<RegisteredHotkeyBinding> = Vec::with_capacity(bindings.len());

        for binding in bindings {
            if let Err(error) = self.register_binding(app, binding) {
                self.unregister_bindings(app, &registered);
                return Err(error);
            }
            registered.push(binding.clone());
        }

        Ok(())
    }

    fn register_binding<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        binding: &RegisteredHotkeyBinding,
    ) -> AppResult<()> {
        binding
            .shortcut
            .parse::<Shortcut>()
            .map_err(|error| invalid_shortcut_error(binding, error.to_string()))?;

        let action = binding.action;
        let shortcut = binding.shortcut.clone();
        let shortcut_for_event = shortcut.clone();

        app.global_shortcut()
            .on_shortcut(shortcut.as_str(), move |app, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    emit_hotkey_trigger(app, action, &shortcut_for_event);
                    handle_hotkey_action(app, action);
                }
            })
            .map_err(|error| register_shortcut_error(binding, error.to_string()))?;

        log::info!(
            target: "bexo::service::hotkey",
            "registered hotkey action={} shortcut={}",
            binding.action.key(),
            binding.shortcut
        );

        Ok(())
    }

    fn unregister_bindings<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        bindings: &[RegisteredHotkeyBinding],
    ) {
        for binding in bindings {
            if let Err(error) = app.global_shortcut().unregister(binding.shortcut.as_str()) {
                log::warn!(
                    target: "bexo::service::hotkey",
                    "unregister hotkey failed action={} shortcut={} reason={}",
                    binding.action.key(),
                    binding.shortcut,
                    error
                );
            }
        }
    }

    fn current_registered(&self) -> AppResult<Vec<RegisteredHotkeyBinding>> {
        self.state
            .lock()
            .map(|guard| guard.registered.clone())
            .map_err(|_| {
                AppError::new(
                    "HOTKEY_LOCK_FAILED",
                    "failed to read hotkey registration state",
                )
            })
    }

    fn replace_registered(&self, registered: Vec<RegisteredHotkeyBinding>) -> AppResult<()> {
        let mut guard = self.state.lock().map_err(|_| {
            AppError::new(
                "HOTKEY_LOCK_FAILED",
                "failed to update hotkey registration state",
            )
        })?;
        guard.registered = registered;
        Ok(())
    }
}

fn build_desired_bindings(preferences: &AppPreferences) -> Vec<RegisteredHotkeyBinding> {
    let mut bindings = Vec::new();

    let screenshot_capture = preferences.hotkey.screenshot_capture.trim();
    if !screenshot_capture.is_empty() {
        bindings.push(RegisteredHotkeyBinding {
            action: HotkeyAction::ScreenshotCapture,
            shortcut: screenshot_capture.to_string(),
        });
    }

    if let Some(voice_toggle) = normalize_optional_shortcut(
        preferences
            .hotkey
            .voice_input_toggle
            .as_ref()
            .map(|value| value.as_str()),
    ) {
        bindings.push(RegisteredHotkeyBinding {
            action: HotkeyAction::VoiceInputToggle,
            shortcut: voice_toggle,
        });
    }

    if let Some(voice_hold) = normalize_optional_shortcut(
        preferences
            .hotkey
            .voice_input_hold
            .as_ref()
            .map(|value| value.as_str()),
    ) {
        bindings.push(RegisteredHotkeyBinding {
            action: HotkeyAction::VoiceInputHold,
            shortcut: voice_hold,
        });
    }

    bindings
}

fn normalize_optional_shortcut(value: Option<&str>) -> Option<String> {
    let raw = value?.trim();
    if raw.is_empty() {
        return None;
    }
    Some(raw.to_string())
}

fn emit_hotkey_trigger<R: Runtime>(app: &AppHandle<R>, action: HotkeyAction, shortcut: &str) {
    let payload = HotkeyTriggerEvent {
        action,
        shortcut: shortcut.to_string(),
        triggered_at: Utc::now().to_rfc3339(),
        source: "global_shortcut".to_string(),
    };

    if let Err(error) = app.emit(HOTKEY_TRIGGER_EVENT_NAME, payload) {
        log::error!(
            target: "bexo::service::hotkey",
            "emit hotkey trigger failed action={} shortcut={} reason={}",
            action.key(),
            shortcut,
            error
        );
    }
}

fn handle_hotkey_action<R: Runtime>(app: &AppHandle<R>, action: HotkeyAction) {
    match action {
        HotkeyAction::ScreenshotCapture => {
            let app_handle = app.clone();
            std::thread::spawn(move || {
                let screenshot_service = app_handle.state::<ScreenshotService>();
                if let Err(error) = screenshot_service.start_session(&app_handle) {
                    log::error!(
                        target: "bexo::service::hotkey",
                        "start screenshot session from hotkey failed: {}",
                        error
                    );
                }
            });
        }
        HotkeyAction::VoiceInputToggle | HotkeyAction::VoiceInputHold => {}
    }
}

fn invalid_shortcut_error(binding: &RegisteredHotkeyBinding, reason: String) -> AppError {
    AppError::new("HOTKEY_SHORTCUT_INVALID", "热键格式无效")
        .with_detail("action", binding.action.key().to_string())
        .with_detail("field", binding.action.preference_field().to_string())
        .with_detail("shortcut", binding.shortcut.to_string())
        .with_detail("reason", reason)
}

fn register_shortcut_error(binding: &RegisteredHotkeyBinding, reason: String) -> AppError {
    let normalized = reason.to_ascii_lowercase();
    let message = if normalized.contains("already registered") {
        "热键已被占用，请更换组合键"
    } else {
        "热键注册失败"
    };

    AppError::new("HOTKEY_REGISTER_FAILED", message)
        .with_detail("action", binding.action.key().to_string())
        .with_detail("field", binding.action.preference_field().to_string())
        .with_detail("shortcut", binding.shortcut.to_string())
        .with_detail("reason", reason)
}
