use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use chrono::Utc;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::{
    domain::{
        AppPreferences, HotkeyAction, HotkeyHealth, HotkeyHealthStatus,
        HotkeyRegisteredBindingView, HotkeyTriggerEvent, HOTKEY_TRIGGER_EVENT_NAME,
    },
    error::{AppError, AppResult},
};

use super::{
    screenshot_service::ScreenshotService,
    windows_hook_hotkey::{
        classify_supported_shortcut, requires_windows_hook, HotkeyShortcutKind,
        HotkeyTriggeredCallback, WindowsHookHotkeyBinding, WindowsHookHotkeyManager,
    },
    PromptQuickPasteService,
};

#[derive(Clone)]
pub struct HotkeyService {
    state: Arc<Mutex<HotkeyServiceState>>,
    apply_lock: Arc<Mutex<()>>,
    hook_manager: Arc<WindowsHookHotkeyManager>,
}

#[derive(Debug, Clone)]
struct HotkeyServiceState {
    registered: Vec<RegisteredHotkeyBinding>,
    status: HotkeyHealthStatus,
    last_error: Option<AppError>,
    updated_at: Option<String>,
}

impl Default for HotkeyServiceState {
    fn default() -> Self {
        Self {
            registered: Vec::new(),
            status: HotkeyHealthStatus::Uninitialized,
            last_error: None,
            updated_at: None,
        }
    }
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
            apply_lock: Arc::new(Mutex::new(())),
            hook_manager: Arc::new(WindowsHookHotkeyManager::new()),
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
        let _apply_guard = self.apply_lock.lock().map_err(|_| {
            AppError::new(
                "HOTKEY_APPLY_LOCK_FAILED",
                "热键更新状态异常，请重启 Bexo Studio 后重试",
            )
        })?;
        let desired = build_desired_bindings(preferences);
        let previous = self.current_registered()?;
        if previous == desired {
            return self.replace_health(desired, HotkeyHealthStatus::Ready, None);
        }

        self.unregister_bindings(app, &previous);

        match self.register_bindings(app, &desired) {
            Ok(()) => self.replace_health(desired, HotkeyHealthStatus::Ready, None),
            Err(error) => {
                let rollback_result = self.register_bindings(app, &previous);
                if let Err(rollback_error) = rollback_result {
                    log::error!(
                        target: "bexo::service::hotkey",
                        "hotkey rollback failed: {}",
                        rollback_error
                    );
                    let failure = error
                        .with_detail("rollback", "failed")
                        .with_detail("rollbackReason", rollback_error.to_string());
                    return Err(self.record_failure(Vec::new(), failure));
                }

                Err(self.record_failure(previous, error))
            }
        }
    }

    pub fn health(&self) -> AppResult<HotkeyHealth> {
        let state = self.state.lock().map_err(|_| {
            AppError::new("HOTKEY_LOCK_FAILED", "failed to read hotkey health state")
        })?;
        Ok(HotkeyHealth {
            status: state.status,
            initialized: state.status != HotkeyHealthStatus::Uninitialized,
            registered_bindings: state
                .registered
                .iter()
                .map(|binding| HotkeyRegisteredBindingView {
                    action: binding.action,
                    shortcut: binding.shortcut.clone(),
                })
                .collect(),
            last_error: state.last_error.clone(),
            updated_at: state.updated_at.clone(),
        })
    }

    fn register_bindings<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        bindings: &[RegisteredHotkeyBinding],
    ) -> AppResult<()> {
        let classified = classify_bindings(bindings)?;
        self.clear_all_basic_bindings(app);
        let mut registered_basic: Vec<RegisteredHotkeyBinding> =
            Vec::with_capacity(classified.basic.len());

        for binding in &classified.basic {
            if let Err(error) = self.register_basic_binding(app, binding) {
                self.unregister_basic_bindings(app, &registered_basic);
                return Err(error);
            }
            registered_basic.push(binding.clone());
        }

        if let Err(error) = self.register_hook_bindings(app, &classified.hook) {
            self.unregister_basic_bindings(app, &registered_basic);
            self.hook_manager.clear_bindings();
            return Err(error);
        }

        Ok(())
    }

    fn clear_all_basic_bindings<R: Runtime>(&self, app: &AppHandle<R>) {
        if let Err(error) = app.global_shortcut().unregister_all() {
            log::warn!(
                target: "bexo::service::hotkey",
                "failed to clear existing basic hotkey registrations before re-registering reason={}",
                error
            );
        }
    }

    fn register_basic_binding<R: Runtime>(
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

        match app.global_shortcut().unregister(shortcut.as_str()) {
            Ok(()) => {
                log::warn!(
                    target: "bexo::service::hotkey",
                    "cleared stale basic hotkey registration before re-register shortcut={}",
                    shortcut
                );
            }
            Err(error) => {
                let reason = error.to_string();
                if !reason.contains("Failed to unregister hotkey") {
                    log::warn!(
                        target: "bexo::service::hotkey",
                        "pre-register cleanup returned unexpected error shortcut={} reason={}",
                        shortcut,
                        reason
                    );
                }
            }
        }

        app.global_shortcut()
            .on_shortcut(shortcut.as_str(), move |app, _shortcut, event| {
                let should_handle = if action.prompt_quick_paste_slot().is_some() {
                    event.state == ShortcutState::Released
                } else {
                    event.state == ShortcutState::Pressed
                };
                if should_handle {
                    let state = if event.state == ShortcutState::Released {
                        "released"
                    } else {
                        "pressed"
                    };
                    log::info!(
                        target: "bexo::service::hotkey",
                        "received hotkey action={} shortcut={} source=global_shortcut state={}",
                        action.key(),
                        shortcut_for_event,
                        state
                    );
                    emit_hotkey_trigger(app, action, &shortcut_for_event, "global_shortcut");
                    handle_hotkey_action(app, action, shortcut_for_event.as_str());
                }
            })
            .map_err(|error| register_shortcut_error(binding, error.to_string()))?;

        log::info!(
            target: "bexo::service::hotkey",
            "registered hotkey action={} shortcut={} source=global_shortcut",
            binding.action.key(),
            binding.shortcut
        );

        Ok(())
    }

    fn register_hook_bindings<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        bindings: &[WindowsHookHotkeyBinding],
    ) -> AppResult<()> {
        if bindings.is_empty() {
            return Ok(());
        }

        let callback = build_windows_hook_callback(app);
        self.hook_manager
            .apply_bindings(bindings, callback)
            .map_err(|reason| register_hook_bindings_error(bindings, reason))?;

        let shortcuts = bindings
            .iter()
            .map(|binding| binding.shortcut.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        log::info!(
            target: "bexo::service::hotkey",
            "registered {} windows hook hotkey binding(s) shortcuts={}",
            bindings.len(),
            shortcuts
        );

        Ok(())
    }

    fn unregister_bindings<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        bindings: &[RegisteredHotkeyBinding],
    ) {
        self.unregister_basic_bindings(app, bindings);

        if bindings
            .iter()
            .any(|binding| requires_windows_hook(binding.shortcut.as_str()))
        {
            self.hook_manager.clear_bindings();
        }
    }

    fn unregister_basic_bindings<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        bindings: &[RegisteredHotkeyBinding],
    ) {
        for binding in bindings {
            if requires_windows_hook(binding.shortcut.as_str()) {
                continue;
            }

            if let Err(error) = app.global_shortcut().unregister(binding.shortcut.as_str()) {
                log::warn!(
                    target: "bexo::service::hotkey",
                    "unregister hotkey failed action={} shortcut={} source=global_shortcut reason={}",
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

    fn replace_health(
        &self,
        registered: Vec<RegisteredHotkeyBinding>,
        status: HotkeyHealthStatus,
        last_error: Option<AppError>,
    ) -> AppResult<()> {
        let mut guard = self.state.lock().map_err(|_| {
            AppError::new("HOTKEY_LOCK_FAILED", "failed to update hotkey health state")
        })?;
        guard.registered = registered;
        guard.status = status;
        guard.last_error = last_error;
        guard.updated_at = Some(Utc::now().to_rfc3339());
        Ok(())
    }

    fn record_failure(
        &self,
        registered: Vec<RegisteredHotkeyBinding>,
        error: AppError,
    ) -> AppError {
        match self.replace_health(
            registered,
            HotkeyHealthStatus::Degraded,
            Some(error.clone()),
        ) {
            Ok(()) => error,
            Err(health_error) => error
                .with_detail("healthStateUpdate", "failed")
                .with_detail("healthStateReason", health_error.to_string()),
        }
    }
}

impl Default for HotkeyService {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Default)]
struct ClassifiedBindings {
    basic: Vec<RegisteredHotkeyBinding>,
    hook: Vec<WindowsHookHotkeyBinding>,
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

    if let Some(voice_toggle) =
        normalize_optional_shortcut(preferences.hotkey.voice_input_toggle.as_deref())
    {
        bindings.push(RegisteredHotkeyBinding {
            action: HotkeyAction::VoiceInputToggle,
            shortcut: voice_toggle,
        });
    }

    if let Some(voice_hold) =
        normalize_optional_shortcut(preferences.hotkey.voice_input_hold.as_deref())
    {
        bindings.push(RegisteredHotkeyBinding {
            action: HotkeyAction::VoiceInputHold,
            shortcut: voice_hold,
        });
    }

    for slot in &preferences.hotkey.prompt_quick_paste_slots {
        if !slot.enabled || slot.prompt_id.is_none() {
            continue;
        }
        if let Some(action) = HotkeyAction::from_prompt_quick_paste_slot(slot.slot) {
            bindings.push(RegisteredHotkeyBinding {
                action,
                shortcut: slot.shortcut.trim().to_string(),
            });
        }
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

fn classify_bindings(bindings: &[RegisteredHotkeyBinding]) -> AppResult<ClassifiedBindings> {
    let mut classified = ClassifiedBindings::default();

    for binding in bindings {
        match classify_supported_shortcut(binding.shortcut.as_str())
            .map_err(|reason| invalid_shortcut_error(binding, reason))?
        {
            HotkeyShortcutKind::Basic => classified.basic.push(binding.clone()),
            HotkeyShortcutKind::WindowsHook => {
                classified.hook.push(WindowsHookHotkeyBinding {
                    action: binding.action,
                    shortcut: binding.shortcut.clone(),
                });
            }
        }
    }

    Ok(classified)
}

fn build_windows_hook_callback<R: Runtime>(app: &AppHandle<R>) -> HotkeyTriggeredCallback {
    let app_handle = app.clone();
    Arc::new(move |action, shortcut| {
        log::info!(
            target: "bexo::service::hotkey",
            "received hotkey action={} shortcut={} source=windows_hook state=pressed",
            action.key(),
            shortcut
        );
        emit_hotkey_trigger(&app_handle, action, shortcut.as_str(), "windows_hook");
        handle_hotkey_action(&app_handle, action, shortcut.as_str());
    })
}

fn emit_hotkey_trigger<R: Runtime>(
    app: &AppHandle<R>,
    action: HotkeyAction,
    shortcut: &str,
    source: &str,
) {
    let payload = HotkeyTriggerEvent {
        action,
        shortcut: shortcut.to_string(),
        triggered_at: Utc::now().to_rfc3339(),
        source: source.to_string(),
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

fn handle_hotkey_action<R: Runtime>(app: &AppHandle<R>, action: HotkeyAction, shortcut: &str) {
    match action {
        HotkeyAction::ScreenshotCapture => {
            let app_handle = app.clone();
            std::thread::spawn(move || {
                let started_at = Instant::now();
                let screenshot_service = app_handle.state::<ScreenshotService>();
                match screenshot_service.start_session(&app_handle) {
                    Ok(result) => {
                        log::info!(
                            target: "bexo::service::hotkey",
                            "started screenshot session from hotkey session_id={} window_label={} total_ms={}",
                            result.session_id,
                            result.window_label,
                            started_at.elapsed().as_millis()
                        );
                    }
                    Err(error) => {
                        log::error!(
                            target: "bexo::service::hotkey",
                            "start screenshot session from hotkey failed total_ms={} reason={}",
                            started_at.elapsed().as_millis(),
                            error
                        );
                    }
                }
            });
        }
        HotkeyAction::VoiceInputToggle | HotkeyAction::VoiceInputHold => {}
        HotkeyAction::PromptQuickPaste1
        | HotkeyAction::PromptQuickPaste2
        | HotkeyAction::PromptQuickPaste3
        | HotkeyAction::PromptQuickPaste4
        | HotkeyAction::PromptQuickPaste5 => {
            let app_handle = app.clone();
            let shortcut = shortcut.to_string();
            let service = app.state::<PromptQuickPasteService>().inner().clone();
            tauri::async_runtime::spawn(async move {
                service.execute(app_handle, action, shortcut).await;
            });
        }
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

fn register_hook_bindings_error(bindings: &[WindowsHookHotkeyBinding], reason: String) -> AppError {
    AppError::new("HOTKEY_REGISTER_FAILED", "热键注册失败")
        .with_detail(
            "actions",
            bindings
                .iter()
                .map(|binding| binding.action.key())
                .collect::<Vec<_>>()
                .join(","),
        )
        .with_detail(
            "shortcuts",
            bindings
                .iter()
                .map(|binding| binding.shortcut.as_str())
                .collect::<Vec<_>>()
                .join(","),
        )
        .with_detail("source", "windows_hook")
        .with_detail("reason", reason)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desired_bindings_include_only_enabled_prompt_slots_with_ids() {
        let mut preferences = AppPreferences::default();
        preferences.hotkey.prompt_quick_paste_slots[0].enabled = true;
        preferences.hotkey.prompt_quick_paste_slots[0].prompt_id =
            Some(uuid::Uuid::new_v4().to_string());
        preferences.hotkey.prompt_quick_paste_slots[1].prompt_id =
            Some(uuid::Uuid::new_v4().to_string());

        let bindings = build_desired_bindings(&preferences);
        assert!(bindings.iter().any(|binding| {
            binding.action == HotkeyAction::PromptQuickPaste1
                && binding.shortcut == "Ctrl+Alt+Shift+1"
        }));
        assert!(!bindings
            .iter()
            .any(|binding| binding.action == HotkeyAction::PromptQuickPaste2));
    }
}
