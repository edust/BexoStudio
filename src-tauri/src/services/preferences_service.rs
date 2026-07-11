use std::{
    collections::HashSet,
    env, fs,
    path::PathBuf,
    sync::{Arc, Mutex, RwLock},
};

use chrono::Utc;
use tauri::{AppHandle, Manager, Runtime};
#[cfg(not(windows))]
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_store::{Store, StoreExt};

use crate::{
    adapters::{resolve_configured_executable, IdeAdapter, JetBrainsAdapter, VSCodeAdapter},
    domain::{
        AppPreferences, AppPreferencesPatch, CodexAuthPreferences, CodexAuthProxyPreferences,
        CodexHistoryViewPreferences, EditorPathDetectionResult, HotkeyAction, HotkeyPreferences,
        PromptQuickPasteHotkeySlot, TerminalCommandShell, TerminalPreferences,
        DEFAULT_CODEX_AUTH_PROXY_MODE, DEFAULT_CODEX_AUTH_QUOTA_REFRESH_INTERVAL_SECONDS,
        DEFAULT_CODEX_HISTORY_MESSAGE_FONT_SIZE, DEFAULT_PROMPT_QUICK_PASTE_HOTKEYS,
        DEFAULT_SCREENSHOT_CAPTURE_HOTKEY, EARLIER_DEFAULT_SCREENSHOT_CAPTURE_HOTKEY,
        LEGACY_SCREENSHOT_CAPTURE_HOTKEY, PREVIOUS_DEFAULT_SCREENSHOT_CAPTURE_HOTKEY,
        PROMPT_QUICK_PASTE_SLOT_COUNT,
    },
    error::{AppError, AppResult},
    services::HotkeyService,
};

use super::windows_hook_hotkey::classify_supported_shortcut;

const PREFERENCES_STORE_PATH: &str = "settings/preferences.json";
const PREFERENCES_STORE_KEY: &str = "appPreferences";
const CODEX_HISTORY_MIN_MESSAGE_FONT_SIZE: i32 = 10;
const CODEX_HISTORY_MAX_MESSAGE_FONT_SIZE: i32 = 24;
const CODEX_HISTORY_MAX_FONT_FAMILY_LENGTH: usize = 160;
const CODEX_AUTH_MIN_QUOTA_REFRESH_INTERVAL_SECONDS: i32 = 10;
const CODEX_AUTH_MAX_QUOTA_REFRESH_INTERVAL_SECONDS: i32 = 3600;
const CODEX_AUTH_MAX_MANUAL_PROXY_URL_LENGTH: usize = 512;

#[derive(Debug, Clone)]
pub struct PreferencesService {
    cache: Arc<RwLock<AppPreferences>>,
    update_lock: Arc<Mutex<()>>,
}

impl PreferencesService {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(AppPreferences::default())),
            update_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn initialize<R: Runtime>(&self, app: &AppHandle<R>) -> AppResult<AppPreferences> {
        let store = self.open_store(app)?;
        let preferences = self.load_or_seed_store(&store)?;
        self.replace_cache(preferences.clone())?;
        Ok(preferences)
    }

    pub fn get_preferences(&self) -> AppResult<AppPreferences> {
        self.cache.read().map(|guard| guard.clone()).map_err(|_| {
            AppError::new(
                "PREFERENCES_LOCK_FAILED",
                "failed to read app preferences cache",
            )
        })
    }

    pub fn update_preferences<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        hotkey_service: &HotkeyService,
        patch: AppPreferencesPatch,
    ) -> AppResult<AppPreferences> {
        let _update_guard = self.update_lock.lock().map_err(|_| {
            AppError::new(
                "PREFERENCES_UPDATE_LOCK_FAILED",
                "设置更新状态异常，请重启 Bexo Studio 后重试",
            )
        })?;
        let previous_preferences = self.get_preferences()?;
        let validated =
            validate_preferences(apply_preferences_patch(previous_preferences.clone(), patch))?;
        let launch_at_login_changed =
            validated.startup.launch_at_login != previous_preferences.startup.launch_at_login;
        if launch_at_login_changed {
            sync_autostart_launch_at_login(app, validated.startup.launch_at_login)?;
        }
        if let Err(error) = hotkey_service.apply_preferences(app, &validated) {
            let rollback_failures = rollback_runtime_preference_effects(
                app,
                hotkey_service,
                &previous_preferences,
                launch_at_login_changed,
            );
            return Err(attach_preference_rollback_failures(
                error,
                rollback_failures,
            ));
        }
        let store = match self.open_store(app) {
            Ok(store) => store,
            Err(error) => {
                let rollback_failures = rollback_runtime_preference_effects(
                    app,
                    hotkey_service,
                    &previous_preferences,
                    launch_at_login_changed,
                );
                return Err(attach_preference_rollback_failures(
                    error,
                    rollback_failures,
                ));
            }
        };
        if let Err(error) = self.write_store(&store, &validated) {
            let mut rollback_failures = Vec::new();
            if let Some(failure) = rollback_store_preferences(self, &store, &previous_preferences) {
                rollback_failures.push(failure);
            }
            rollback_failures.extend(rollback_runtime_preference_effects(
                app,
                hotkey_service,
                &previous_preferences,
                launch_at_login_changed,
            ));
            return Err(attach_preference_rollback_failures(
                error,
                rollback_failures,
            ));
        }
        if let Err(error) = self.replace_cache(validated.clone()) {
            let mut rollback_failures = Vec::new();
            if let Some(failure) = rollback_store_preferences(self, &store, &previous_preferences) {
                rollback_failures.push(failure);
            }
            rollback_failures.extend(rollback_runtime_preference_effects(
                app,
                hotkey_service,
                &previous_preferences,
                launch_at_login_changed,
            ));
            return Err(attach_preference_rollback_failures(
                error,
                rollback_failures,
            ));
        }
        Ok(validated)
    }

    pub fn sync_launch_at_login<R: Runtime>(&self, app: &AppHandle<R>) -> AppResult<()> {
        let preferences = self.get_preferences()?;
        sync_autostart_launch_at_login(app, preferences.startup.launch_at_login)
    }

    pub fn set_preferences_for_runtime(&self, input: AppPreferences) -> AppResult<AppPreferences> {
        let validated = validate_preferences(input)?;
        self.replace_cache(validated.clone())?;
        Ok(validated)
    }

    pub fn get_codex_home_directory<R: Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> AppResult<crate::domain::CodexHomeDirectoryInfo> {
        let configured_home = env::var("CODEX_HOME")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let resolved = if let Some(path) = configured_home {
            build_codex_home_directory_info(PathBuf::from(path), "env")
        } else {
            match app.path().home_dir() {
                Ok(home_dir) => build_codex_home_directory_info(home_dir.join(".codex"), "default"),
                Err(_) => crate::domain::CodexHomeDirectoryInfo {
                    path: None,
                    source: "unavailable".to_string(),
                    exists: false,
                },
            }
        };

        Ok(resolved)
    }

    pub fn detect_editors_from_path(&self) -> AppResult<EditorPathDetectionResult> {
        let vscode = VSCodeAdapter.detect(None);
        let jetbrains = JetBrainsAdapter.detect(None);
        Ok(EditorPathDetectionResult {
            checked_at: Utc::now().to_rfc3339(),
            vscode,
            jetbrains,
        })
    }

    #[cfg(test)]
    pub fn hydrate_for_test(&self, preferences: AppPreferences) {
        if let Ok(mut guard) = self.cache.write() {
            *guard = preferences;
        }
    }

    fn replace_cache(&self, preferences: AppPreferences) -> AppResult<()> {
        let mut guard = self.cache.write().map_err(|_| {
            AppError::new(
                "PREFERENCES_LOCK_FAILED",
                "failed to update app preferences cache",
            )
        })?;
        *guard = preferences;
        Ok(())
    }

    fn open_store<R: Runtime>(&self, app: &AppHandle<R>) -> AppResult<Arc<Store<R>>> {
        app.store_builder(PREFERENCES_STORE_PATH)
            .build()
            .map_err(|error| {
                AppError::new(
                    "PREFERENCES_STORE_FAILED",
                    "failed to open preferences store",
                )
                .with_detail("path", PREFERENCES_STORE_PATH)
                .with_detail("reason", error.to_string())
            })
    }

    fn load_or_seed_store<R: Runtime>(&self, store: &Store<R>) -> AppResult<AppPreferences> {
        match store.get(PREFERENCES_STORE_KEY) {
            Some(value) => {
                let parsed = serde_json::from_value::<AppPreferences>(value).map_err(|error| {
                    AppError::new(
                        "PREFERENCES_PARSE_FAILED",
                        "failed to parse persisted app preferences",
                    )
                    .with_detail("reason", error.to_string())
                })?;
                let migrated = migrate_legacy_preferences(parsed);
                let repaired = repair_invalid_preferences(migrated.preferences);

                if migrated.changed || repaired.changed {
                    log::info!(
                        target: "bexo::service::preferences",
                        "normalized persisted hotkey preferences to {}",
                        DEFAULT_SCREENSHOT_CAPTURE_HOTKEY
                    );
                    self.write_store(store, &repaired.preferences)?;
                }

                Ok(repaired.preferences)
            }
            None => {
                let defaults = AppPreferences::default();
                self.write_store(store, &defaults)?;
                Ok(defaults)
            }
        }
    }

    fn write_store<R: Runtime>(
        &self,
        store: &Store<R>,
        preferences: &AppPreferences,
    ) -> AppResult<()> {
        let serialized = serde_json::to_value(preferences).map_err(|error| {
            AppError::new(
                "PREFERENCES_SERIALIZE_FAILED",
                "failed to serialize app preferences",
            )
            .with_detail("reason", error.to_string())
        })?;
        store.set(PREFERENCES_STORE_KEY, serialized);
        store.save().map_err(|error| {
            AppError::new("PREFERENCES_STORE_FAILED", "failed to save app preferences")
                .with_detail("path", PREFERENCES_STORE_PATH)
                .with_detail("reason", error.to_string())
        })
    }
}

fn apply_preferences_patch(
    mut current: AppPreferences,
    patch: AppPreferencesPatch,
) -> AppPreferences {
    if let Some(terminal) = patch.terminal {
        current.terminal = terminal;
    }
    if let Some(ide) = patch.ide {
        current.ide = ide;
    }
    if let Some(workspace) = patch.workspace {
        if let Some(selected_workspace_ids) = workspace.selected_workspace_ids {
            current.workspace.selected_workspace_ids = selected_workspace_ids;
        }
        if let Some(pinned_workspace_ids) = workspace.pinned_workspace_ids {
            current.workspace.pinned_workspace_ids = pinned_workspace_ids;
        }
    }
    if let Some(startup) = patch.startup {
        current.startup = startup;
    }
    if let Some(hotkey) = patch.hotkey {
        current.hotkey = hotkey;
    }
    if let Some(tray) = patch.tray {
        current.tray = tray;
    }
    if let Some(diagnostics) = patch.diagnostics {
        current.diagnostics = diagnostics;
    }
    if let Some(codex_history) = patch.codex_history {
        current.codex_history = codex_history;
    }
    if let Some(codex_auth) = patch.codex_auth {
        current.codex_auth = codex_auth;
    }
    current
}

impl Default for PreferencesService {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
struct PreferenceMigrationResult {
    preferences: AppPreferences,
    changed: bool,
}

#[derive(Debug)]
struct PreferenceRepairResult {
    preferences: AppPreferences,
    changed: bool,
}

fn migrate_legacy_preferences(mut preferences: AppPreferences) -> PreferenceMigrationResult {
    let trimmed = preferences.hotkey.screenshot_capture.trim();
    if trimmed.eq_ignore_ascii_case(LEGACY_SCREENSHOT_CAPTURE_HOTKEY)
        || trimmed.eq_ignore_ascii_case(PREVIOUS_DEFAULT_SCREENSHOT_CAPTURE_HOTKEY)
        || trimmed.eq_ignore_ascii_case(EARLIER_DEFAULT_SCREENSHOT_CAPTURE_HOTKEY)
    {
        preferences.hotkey.screenshot_capture = DEFAULT_SCREENSHOT_CAPTURE_HOTKEY.to_string();
        return PreferenceMigrationResult {
            preferences,
            changed: true,
        };
    }

    PreferenceMigrationResult {
        preferences,
        changed: false,
    }
}

fn repair_invalid_preferences(mut preferences: AppPreferences) -> PreferenceRepairResult {
    let repaired_terminal = sanitize_terminal_preferences(preferences.terminal);
    preferences.terminal = repaired_terminal.preferences;
    let repaired_hotkeys = sanitize_hotkey_preferences(preferences.hotkey);
    preferences.hotkey = repaired_hotkeys.preferences;
    let repaired_codex_history = sanitize_codex_history_view_preferences(preferences.codex_history);
    preferences.codex_history = repaired_codex_history.preferences;
    let repaired_codex_auth = sanitize_codex_auth_preferences(preferences.codex_auth);
    preferences.codex_auth = repaired_codex_auth.preferences;

    PreferenceRepairResult {
        preferences,
        changed: repaired_terminal.changed
            || repaired_hotkeys.changed
            || repaired_codex_history.changed
            || repaired_codex_auth.changed,
    }
}

#[derive(Debug)]
struct TerminalPreferenceRepairResult {
    preferences: TerminalPreferences,
    changed: bool,
}

fn sanitize_terminal_preferences(input: TerminalPreferences) -> TerminalPreferenceRepairResult {
    let mut changed = false;
    let command_templates = match validate_command_templates(input.command_templates.clone()) {
        Ok(templates) => templates,
        Err(error) => {
            log::warn!(
                target: "bexo::service::preferences",
                "failed to preserve persisted terminal command templates during repair, fallback to empty templates reason={}",
                error
            );
            changed = true;
            Vec::new()
        }
    };

    TerminalPreferenceRepairResult {
        preferences: TerminalPreferences {
            windows_terminal_path: input.windows_terminal_path,
            codex_cli_path: input.codex_cli_path,
            command_shell: input.command_shell,
            command_templates,
        },
        changed,
    }
}

#[derive(Debug)]
struct HotkeyPreferenceRepairResult {
    preferences: HotkeyPreferences,
    changed: bool,
}

fn sanitize_hotkey_preferences(input: HotkeyPreferences) -> HotkeyPreferenceRepairResult {
    let defaults = HotkeyPreferences::default();
    let mut changed = false;

    let input_voice_toggle = input.voice_input_toggle.clone();
    let input_voice_hold = input.voice_input_hold.clone();
    let prompt_quick_paste_slots =
        sanitize_prompt_quick_paste_slots(input.prompt_quick_paste_slots, &mut changed);

    let screenshot_capture = sanitize_required_hotkey_shortcut(
        HotkeyAction::ScreenshotCapture,
        input.screenshot_capture,
        defaults.screenshot_capture.as_str(),
        &mut changed,
    );
    let voice_input_toggle = sanitize_optional_hotkey_shortcut(
        HotkeyAction::VoiceInputToggle,
        input.voice_input_toggle,
        &mut changed,
    );
    let voice_input_hold = sanitize_optional_hotkey_shortcut(
        HotkeyAction::VoiceInputHold,
        input.voice_input_hold,
        &mut changed,
    );

    let mut repaired = HotkeyPreferences {
        screenshot_capture,
        voice_input_toggle,
        voice_input_hold,
        prompt_quick_paste_slots,
    };

    repaired.voice_input_toggle = repaired
        .voice_input_toggle
        .take()
        .filter(|shortcut| !shortcut.eq_ignore_ascii_case(repaired.screenshot_capture.as_str()));
    if repaired.voice_input_toggle.is_none() && input_voice_toggle.is_some() {
        changed = true;
    }

    repaired.voice_input_hold = repaired.voice_input_hold.take().filter(|shortcut| {
        !shortcut.eq_ignore_ascii_case(repaired.screenshot_capture.as_str())
            && repaired
                .voice_input_toggle
                .as_deref()
                .map(|toggle| !shortcut.eq_ignore_ascii_case(toggle))
                .unwrap_or(true)
    });
    if repaired.voice_input_hold.is_none() && input_voice_hold.is_some() {
        changed = true;
    }

    let mut seen_active_shortcuts = HashSet::new();
    seen_active_shortcuts.insert(repaired.screenshot_capture.to_ascii_lowercase());
    if let Some(shortcut) = repaired.voice_input_toggle.as_deref() {
        seen_active_shortcuts.insert(shortcut.to_ascii_lowercase());
    }
    if let Some(shortcut) = repaired.voice_input_hold.as_deref() {
        seen_active_shortcuts.insert(shortcut.to_ascii_lowercase());
    }
    for slot in &mut repaired.prompt_quick_paste_slots {
        if slot.enabled && !seen_active_shortcuts.insert(slot.shortcut.to_ascii_lowercase()) {
            slot.enabled = false;
            changed = true;
        }
    }

    if let Err(error) = validate_hotkey_preferences(repaired.clone()) {
        log::warn!(
            target: "bexo::service::preferences",
            "failed to preserve persisted hotkeys during repair, fallback to defaults reason={}",
            error
        );
        return HotkeyPreferenceRepairResult {
            preferences: defaults,
            changed: true,
        };
    }

    HotkeyPreferenceRepairResult {
        preferences: repaired,
        changed,
    }
}

fn sanitize_prompt_quick_paste_slots(
    input: Vec<PromptQuickPasteHotkeySlot>,
    changed: &mut bool,
) -> Vec<PromptQuickPasteHotkeySlot> {
    let mut candidates = input;
    if candidates.len() != PROMPT_QUICK_PASTE_SLOT_COUNT {
        *changed = true;
    }

    let mut repaired = Vec::with_capacity(PROMPT_QUICK_PASTE_SLOT_COUNT);
    for slot_number in 1..=PROMPT_QUICK_PASTE_SLOT_COUNT as u8 {
        let action = HotkeyAction::from_prompt_quick_paste_slot(slot_number)
            .expect("prompt quick paste slot must map to a hotkey action");
        let default_shortcut = DEFAULT_PROMPT_QUICK_PASTE_HOTKEYS[(slot_number - 1) as usize];
        let Some(index) = candidates
            .iter()
            .position(|candidate| candidate.slot == slot_number)
        else {
            *changed = true;
            repaired.push(PromptQuickPasteHotkeySlot {
                slot: slot_number,
                enabled: false,
                prompt_id: None,
                shortcut: default_shortcut.to_string(),
            });
            continue;
        };
        let candidate = candidates.remove(index);
        let shortcut = sanitize_required_hotkey_shortcut(
            action,
            candidate.shortcut,
            default_shortcut,
            changed,
        );
        let prompt_id = candidate.prompt_id.and_then(|raw| {
            let trimmed = raw.trim();
            match uuid::Uuid::parse_str(trimmed) {
                Ok(id) => {
                    let normalized = id.to_string();
                    if normalized != trimmed {
                        *changed = true;
                    }
                    Some(normalized)
                }
                Err(_) => {
                    *changed = true;
                    None
                }
            }
        });
        let enabled = candidate.enabled && prompt_id.is_some();
        if enabled != candidate.enabled {
            *changed = true;
        }
        repaired.push(PromptQuickPasteHotkeySlot {
            slot: slot_number,
            enabled,
            prompt_id,
            shortcut,
        });
    }

    if !candidates.is_empty() {
        *changed = true;
    }
    repaired
}

fn sanitize_required_hotkey_shortcut(
    action: HotkeyAction,
    value: String,
    fallback: &str,
    changed: &mut bool,
) -> String {
    match validate_hotkey_shortcut(action, value.clone(), true) {
        Ok(Some(validated)) => validated,
        Ok(None) => fallback.to_string(),
        Err(error) => {
            log::warn!(
                target: "bexo::service::preferences",
                "repair required hotkey action={} shortcut={} reason={}",
                action.key(),
                value,
                error
            );
            *changed = true;
            fallback.to_string()
        }
    }
}

fn sanitize_optional_hotkey_shortcut(
    action: HotkeyAction,
    value: Option<String>,
    changed: &mut bool,
) -> Option<String> {
    let Some(raw) = value else {
        return None;
    };

    match validate_hotkey_shortcut(action, raw.clone(), false) {
        Ok(validated) => validated,
        Err(error) => {
            log::warn!(
                target: "bexo::service::preferences",
                "repair optional hotkey action={} shortcut={} reason={}",
                action.key(),
                raw,
                error
            );
            *changed = true;
            None
        }
    }
}

#[derive(Debug)]
struct CodexHistoryViewPreferenceRepairResult {
    preferences: CodexHistoryViewPreferences,
    changed: bool,
}

fn sanitize_codex_history_view_preferences(
    input: CodexHistoryViewPreferences,
) -> CodexHistoryViewPreferenceRepairResult {
    let defaults = CodexHistoryViewPreferences::default();
    let mut changed = false;

    let message_font_family =
        match validate_codex_history_font_family(input.message_font_family.clone()) {
            Ok(validated) => {
                if validated != input.message_font_family {
                    changed = true;
                }
                validated
            }
            Err(error) => {
                log::warn!(
                    target: "bexo::service::preferences",
                    "repair codex history font family reason={}",
                    error
                );
                changed = true;
                defaults.message_font_family.clone()
            }
        };

    let message_font_size = if (CODEX_HISTORY_MIN_MESSAGE_FONT_SIZE
        ..=CODEX_HISTORY_MAX_MESSAGE_FONT_SIZE)
        .contains(&input.message_font_size)
    {
        input.message_font_size
    } else {
        log::warn!(
            target: "bexo::service::preferences",
            "repair codex history message font size value={}",
            input.message_font_size
        );
        changed = true;
        DEFAULT_CODEX_HISTORY_MESSAGE_FONT_SIZE
    };

    CodexHistoryViewPreferenceRepairResult {
        preferences: CodexHistoryViewPreferences {
            message_font_family,
            message_font_size,
        },
        changed,
    }
}

#[derive(Debug)]
struct CodexAuthPreferenceRepairResult {
    preferences: CodexAuthPreferences,
    changed: bool,
}

fn sanitize_codex_auth_preferences(input: CodexAuthPreferences) -> CodexAuthPreferenceRepairResult {
    let mut changed = false;

    let quota_refresh_interval_seconds = if (CODEX_AUTH_MIN_QUOTA_REFRESH_INTERVAL_SECONDS
        ..=CODEX_AUTH_MAX_QUOTA_REFRESH_INTERVAL_SECONDS)
        .contains(&input.quota_refresh_interval_seconds)
    {
        input.quota_refresh_interval_seconds
    } else {
        log::warn!(
            target: "bexo::service::preferences",
            "repair codex auth quota refresh interval value={}",
            input.quota_refresh_interval_seconds
        );
        changed = true;
        DEFAULT_CODEX_AUTH_QUOTA_REFRESH_INTERVAL_SECONDS
    };
    let proxy = match validate_codex_auth_proxy_preferences(input.proxy.clone()) {
        Ok(validated) => {
            if validated.mode != input.proxy.mode
                || validated.manual_proxy_url != input.proxy.manual_proxy_url
            {
                changed = true;
            }
            validated
        }
        Err(error) => {
            log::warn!(
                target: "bexo::service::preferences",
                "repair codex auth proxy preferences reason={}",
                error
            );
            changed = true;
            CodexAuthProxyPreferences::default()
        }
    };

    CodexAuthPreferenceRepairResult {
        preferences: CodexAuthPreferences {
            quota_refresh_interval_seconds,
            proxy,
        },
        changed,
    }
}

fn sync_autostart_launch_at_login<R: Runtime>(
    app: &AppHandle<R>,
    should_enable: bool,
) -> AppResult<()> {
    #[cfg(windows)]
    {
        windows_autostart::sync_launch_at_login(app.package_info().name.as_str(), should_enable)
    }

    #[cfg(not(windows))]
    {
        let autolaunch = app.autolaunch();
        let current_enabled = autolaunch.is_enabled().map_err(|error| {
            AppError::new("AUTOSTART_STATUS_READ_FAILED", "读取开机启动状态失败")
                .with_detail("reason", error.to_string())
        })?;

        if current_enabled == should_enable {
            return Ok(());
        }

        if should_enable {
            autolaunch.enable().map_err(|error| {
                AppError::new("AUTOSTART_ENABLE_FAILED", "开启开机启动失败")
                    .with_detail("reason", error.to_string())
            })?;
        } else {
            autolaunch.disable().map_err(|error| {
                AppError::new("AUTOSTART_DISABLE_FAILED", "关闭开机启动失败")
                    .with_detail("reason", error.to_string())
            })?;
        }

        Ok(())
    }
}

fn rollback_autostart_launch_at_login<R: Runtime>(
    app: &AppHandle<R>,
    previous_preferences: &AppPreferences,
    launch_at_login_changed: bool,
) -> Option<String> {
    if !launch_at_login_changed {
        return None;
    }

    match sync_autostart_launch_at_login(app, previous_preferences.startup.launch_at_login) {
        Ok(()) => None,
        Err(error) => {
            log::error!(
                target: "bexo::service::preferences",
                "autostart rollback failed after preferences update error code={} message={} details={:?}",
                error.code,
                error.message,
                error.details
            );
            Some(format!(
                "autostart: code={} message={}",
                error.code, error.message
            ))
        }
    }
}

fn rollback_runtime_preference_effects<R: Runtime>(
    app: &AppHandle<R>,
    hotkey_service: &HotkeyService,
    previous_preferences: &AppPreferences,
    launch_at_login_changed: bool,
) -> Vec<String> {
    let mut failures = Vec::new();
    if let Err(error) = hotkey_service.apply_preferences(app, previous_preferences) {
        log::error!(
            target: "bexo::service::preferences",
            "hotkey rollback failed after preferences persistence error code={} message={} details={:?}",
            error.code,
            error.message,
            error.details
        );
        failures.push(format!(
            "hotkey: code={} message={}",
            error.code, error.message
        ));
    }
    if let Some(failure) =
        rollback_autostart_launch_at_login(app, previous_preferences, launch_at_login_changed)
    {
        failures.push(failure);
    }
    failures
}

fn rollback_store_preferences<R: Runtime>(
    service: &PreferencesService,
    store: &Store<R>,
    previous_preferences: &AppPreferences,
) -> Option<String> {
    match service.write_store(store, previous_preferences) {
        Ok(()) => None,
        Err(error) => {
            log::error!(
                target: "bexo::service::preferences",
                "preferences store rollback failed code={} message={} details={:?}",
                error.code,
                error.message,
                error.details
            );
            Some(format!(
                "store: code={} message={}",
                error.code, error.message
            ))
        }
    }
}

fn attach_preference_rollback_failures(
    mut original_error: AppError,
    rollback_failures: Vec<String>,
) -> AppError {
    if !rollback_failures.is_empty() {
        original_error = original_error
            .with_detail("rollbackFailure", rollback_failures.join("; "))
            .with_detail("stateConsistency", "unknown");
    }
    original_error
}

#[cfg(windows)]
mod windows_autostart {
    use std::{
        ffi::{c_void, OsStr},
        os::windows::ffi::OsStrExt,
        path::{Path, PathBuf},
        ptr,
    };

    use crate::error::{AppError, AppResult};

    type HKey = isize;

    const HKEY_CURRENT_USER: HKey = -2147483647i32 as HKey;
    const ERROR_SUCCESS: i32 = 0;
    const ERROR_FILE_NOT_FOUND: i32 = 2;
    const ERROR_MORE_DATA: i32 = 234;
    const REG_SZ: u32 = 1;
    const REG_EXPAND_SZ: u32 = 2;
    const REG_BINARY: u32 = 3;
    const KEY_QUERY_VALUE: u32 = 0x0001;
    const KEY_SET_VALUE: u32 = 0x0002;
    const REG_OPTION_NON_VOLATILE: u32 = 0;
    const RUN_KEY: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run";
    const STARTUP_APPROVED_RUN_KEY: &str =
        "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run";
    const AUTOSTART_ARGS: [&str; 1] = ["--autostart"];
    const MAX_RUN_COMMAND_UTF16_UNITS: usize = 260;
    const MAX_REGISTRY_READ_ATTEMPTS: usize = 2;
    const MAX_REGISTRY_VALUE_BYTES: u32 = 65_536;
    const TASK_MANAGER_ENABLED_VALUE: [u8; 12] = [
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RegistryValueSnapshot {
        value_type: u32,
        bytes: Vec<u8>,
    }

    struct RegistryRollbackPlan<'a> {
        run_before: Option<&'a RegistryValueSnapshot>,
        approval_before: Option<&'a RegistryValueSnapshot>,
        expected_run_value: &'a RegistryValueSnapshot,
        expected_approval_value: &'a RegistryValueSnapshot,
        run_was_modified: bool,
        approval_was_modified: bool,
    }

    #[link(name = "Advapi32")]
    extern "system" {
        fn RegCreateKeyExW(
            hkey: HKey,
            lp_sub_key: *const u16,
            reserved: u32,
            lp_class: *const u16,
            dw_options: u32,
            sam_desired: u32,
            lp_security_attributes: *const c_void,
            phk_result: *mut HKey,
            lpdw_disposition: *mut u32,
        ) -> i32;
        fn RegOpenKeyExW(
            hkey: HKey,
            lp_sub_key: *const u16,
            ul_options: u32,
            sam_desired: u32,
            phk_result: *mut HKey,
        ) -> i32;
        fn RegSetValueExW(
            hkey: HKey,
            lp_value_name: *const u16,
            reserved: u32,
            dw_type: u32,
            lp_data: *const u8,
            cb_data: u32,
        ) -> i32;
        fn RegQueryValueExW(
            hkey: HKey,
            lp_value_name: *const u16,
            lp_reserved: *mut u32,
            lp_type: *mut u32,
            lp_data: *mut u8,
            lpcb_data: *mut u32,
        ) -> i32;
        fn RegDeleteValueW(hkey: HKey, lp_value_name: *const u16) -> i32;
        fn RegCloseKey(hkey: HKey) -> i32;
    }

    pub fn sync_launch_at_login(app_name: &str, should_enable: bool) -> AppResult<()> {
        let app_name = app_name.trim();
        if app_name.is_empty() {
            return Err(AppError::new(
                "AUTOSTART_APP_NAME_INVALID",
                "开机启动应用名称无效",
            ));
        }

        if should_enable {
            sync_enabled(app_name)
        } else {
            sync_disabled(app_name)
        }
    }

    fn sync_enabled(app_name: &str) -> AppResult<()> {
        let current_executable = std::env::current_exe().map_err(|error| {
            AppError::new("AUTOSTART_PATH_RESOLVE_FAILED", "解析开机启动路径失败")
                .with_detail("reason", error.to_string())
        })?;
        if !current_executable.is_file() {
            return Err(
                AppError::new("AUTOSTART_PATH_INVALID", "开机启动程序路径不存在")
                    .with_detail("path", current_executable.display().to_string()),
            );
        }

        let run_before = get_registry_raw_value_if_exists(RUN_KEY, app_name)?;
        let (approval_before, approval_was_read) = match get_registry_raw_value_if_exists(
            STARTUP_APPROVED_RUN_KEY,
            app_name,
        ) {
            Ok(value) => (value, true),
            Err(error) => {
                log::warn!(
                    target: "bexo::service::preferences",
                    "autostart StartupApproved state unavailable; continuing with documented Run registration code={} message={} details={:?}",
                    error.code,
                    error.message,
                    error.details
                );
                (None, false)
            }
        };
        let existing_command_line = run_before.as_ref().and_then(decode_registry_string_value);
        let registration_executable =
            resolve_registration_executable(&current_executable, existing_command_line.as_deref());
        let command_line =
            build_registry_run_command_line(&registration_executable, &AUTOSTART_ARGS);
        validate_registry_run_command_line(&command_line)?;
        let expected_run_value = registry_string_value_snapshot(&command_line);
        let expected_approval_value = RegistryValueSnapshot {
            value_type: REG_BINARY,
            bytes: TASK_MANAGER_ENABLED_VALUE.to_vec(),
        };

        let run_matches = run_before.as_ref().is_some_and(|value| {
            value.value_type == REG_SZ
                && decode_registry_string_value(value).as_deref() == Some(command_line.as_str())
        });
        let approval_needs_repair = approval_before
            .as_ref()
            .is_some_and(|value| !startup_approved_value_is_enabled(value));

        if run_matches && !approval_needs_repair {
            log::info!(
                target: "bexo::service::preferences",
                "autostart sync verified existing registration app_name={} current_exe={} registered_exe={} approval_checked={} approval_present={}",
                app_name,
                current_executable.display(),
                registration_executable.display(),
                approval_was_read,
                approval_before.is_some()
            );
            return Ok(());
        }

        let mut run_was_modified = false;
        let mut approval_was_modified = false;
        let update_result = (|| {
            if !run_matches {
                set_registry_string_value(RUN_KEY, app_name, &command_line)?;
                run_was_modified = true;
            }
            if approval_needs_repair {
                set_startup_approved_enabled_if_present(app_name)?;
                approval_was_modified = true;
            }
            verify_enabled_registration(app_name, &command_line, approval_was_read)
        })();

        if let Err(error) = update_result {
            return Err(rollback_registry_snapshots(
                app_name,
                RegistryRollbackPlan {
                    run_before: run_before.as_ref(),
                    approval_before: approval_before.as_ref(),
                    expected_run_value: &expected_run_value,
                    expected_approval_value: &expected_approval_value,
                    run_was_modified,
                    approval_was_modified,
                },
                error,
            ));
        }

        log::info!(
            target: "bexo::service::preferences",
            "autostart sync repaired registration app_name={} current_exe={} registered_exe={} run_updated={} approval_updated={}",
            app_name,
            current_executable.display(),
            registration_executable.display(),
            !run_matches,
            approval_needs_repair
        );
        Ok(())
    }

    fn sync_disabled(app_name: &str) -> AppResult<()> {
        let Some(run_before) = get_registry_raw_value_if_exists(RUN_KEY, app_name)? else {
            log::info!(
                target: "bexo::service::preferences",
                "autostart sync verified registration absent app_name={}",
                app_name
            );
            return Ok(());
        };

        delete_registry_value_if_exists(RUN_KEY, app_name)?;
        if let Err(error) = verify_disabled_registration(app_name) {
            let rollback_error = restore_registry_value_if_absent(RUN_KEY, app_name, &run_before);
            return Err(attach_rollback_result(error, rollback_error));
        }

        log::info!(
            target: "bexo::service::preferences",
            "autostart sync removed run command app_name={}",
            app_name
        );
        Ok(())
    }

    fn set_startup_approved_enabled_if_present(app_name: &str) -> AppResult<()> {
        let key = match open_registry_key_with_access(STARTUP_APPROVED_RUN_KEY, KEY_SET_VALUE) {
            Ok(key) => key,
            Err(status) if status == ERROR_FILE_NOT_FOUND => return Ok(()),
            Err(status) => {
                return Err(AppError::new(
                    "AUTOSTART_APPROVAL_OPEN_FAILED",
                    "打开开机启动任务管理器状态失败",
                )
                .with_detail("reason", format!("RegOpenKeyExW returned {status}")));
            }
        };
        set_registry_raw_value(&key, app_name, REG_BINARY, &TASK_MANAGER_ENABLED_VALUE).map_err(
            |error| {
                AppError::new(
                    "AUTOSTART_APPROVAL_WRITE_FAILED",
                    "更新开机启动任务管理器状态失败",
                )
                .with_detail("reason", error)
            },
        )
    }

    fn validate_registry_run_command_line(command_line: &str) -> AppResult<()> {
        let utf16_units = command_line.encode_utf16().count();
        if utf16_units > MAX_RUN_COMMAND_UTF16_UNITS {
            return Err(AppError::new(
                "AUTOSTART_COMMAND_TOO_LONG",
                "开机启动命令长度超过 Windows 限制",
            )
            .with_detail("utf16Units", utf16_units.to_string())
            .with_detail("limit", MAX_RUN_COMMAND_UTF16_UNITS.to_string()));
        }
        Ok(())
    }

    fn verify_enabled_registration(
        app_name: &str,
        expected_command: &str,
        verify_startup_approval: bool,
    ) -> AppResult<()> {
        let run_value = get_registry_raw_value_if_exists(RUN_KEY, app_name)?.ok_or_else(|| {
            AppError::new("AUTOSTART_VERIFY_FAILED", "开机启动注册项写入后不存在")
        })?;
        let actual_command = decode_registry_string_value(&run_value).ok_or_else(|| {
            AppError::new("AUTOSTART_VERIFY_FAILED", "开机启动注册项格式无效")
                .with_detail("registryType", run_value.value_type.to_string())
        })?;
        if run_value.value_type != REG_SZ || actual_command != expected_command {
            return Err(AppError::new(
                "AUTOSTART_VERIFY_FAILED",
                "开机启动注册项写入后与预期不一致",
            )
            .with_detail("registryType", run_value.value_type.to_string()));
        }

        if verify_startup_approval {
            if let Some(approval) =
                get_registry_raw_value_if_exists(STARTUP_APPROVED_RUN_KEY, app_name)?
            {
                if !startup_approved_value_is_enabled(&approval) {
                    return Err(AppError::new(
                        "AUTOSTART_APPROVAL_VERIFY_FAILED",
                        "开机启动仍被 Windows 启动项管理禁用",
                    )
                    .with_detail("registryType", approval.value_type.to_string()));
                }
            }
        }
        Ok(())
    }

    fn verify_disabled_registration(app_name: &str) -> AppResult<()> {
        if get_registry_raw_value_if_exists(RUN_KEY, app_name)?.is_some() {
            return Err(AppError::new(
                "AUTOSTART_VERIFY_FAILED",
                "关闭开机启动后注册项仍然存在",
            ));
        }
        Ok(())
    }

    fn startup_approved_value_is_enabled(value: &RegistryValueSnapshot) -> bool {
        value.value_type == REG_BINARY
            && value.bytes.len() >= 8
            && value.bytes.iter().rev().take(8).all(|byte| *byte == 0)
    }

    fn rollback_registry_snapshots(
        app_name: &str,
        plan: RegistryRollbackPlan<'_>,
        mut original_error: AppError,
    ) -> AppError {
        let mut rollback_failures = Vec::new();
        if plan.run_was_modified {
            if let Err(error) = restore_registry_value_if_unchanged(
                RUN_KEY,
                app_name,
                plan.expected_run_value,
                plan.run_before,
            ) {
                rollback_failures.push(format!("Run: {error}"));
            }
        }
        if plan.approval_was_modified {
            if let Err(error) = restore_registry_value_if_unchanged(
                STARTUP_APPROVED_RUN_KEY,
                app_name,
                plan.expected_approval_value,
                plan.approval_before,
            ) {
                rollback_failures.push(format!("StartupApproved: {error}"));
            }
        }
        if !rollback_failures.is_empty() {
            original_error = original_error
                .with_detail("rollbackFailure", rollback_failures.join("; "))
                .with_detail("stateConsistency", "unknown");
        }
        original_error
    }

    fn restore_registry_value_if_unchanged(
        key_path: &str,
        value_name: &str,
        expected_current: &RegistryValueSnapshot,
        snapshot: Option<&RegistryValueSnapshot>,
    ) -> Result<(), String> {
        let current = get_registry_raw_value_if_exists(key_path, value_name)
            .map_err(|error| error.to_string())?;
        if current.as_ref() != Some(expected_current) {
            return Err("current value changed concurrently; rollback skipped".to_string());
        }
        restore_registry_value(key_path, value_name, snapshot)
    }

    fn restore_registry_value_if_absent(
        key_path: &str,
        value_name: &str,
        snapshot: &RegistryValueSnapshot,
    ) -> Result<(), String> {
        let current = get_registry_raw_value_if_exists(key_path, value_name)
            .map_err(|error| error.to_string())?;
        if current.is_some() {
            return Err("current value was recreated concurrently; rollback skipped".to_string());
        }
        restore_registry_value(key_path, value_name, Some(snapshot))
    }

    fn attach_rollback_result(
        mut original_error: AppError,
        rollback_result: Result<(), String>,
    ) -> AppError {
        if let Err(error) = rollback_result {
            original_error = original_error
                .with_detail("rollbackFailure", error)
                .with_detail("stateConsistency", "unknown");
        }
        original_error
    }

    fn restore_registry_value(
        key_path: &str,
        value_name: &str,
        snapshot: Option<&RegistryValueSnapshot>,
    ) -> Result<(), String> {
        match snapshot {
            Some(snapshot) => {
                let key = create_registry_key(key_path, "AUTOSTART_ROLLBACK_OPEN_FAILED")
                    .map_err(|error| error.to_string())?;
                set_registry_raw_value(&key, value_name, snapshot.value_type, &snapshot.bytes)
            }
            None => delete_registry_value_if_exists(key_path, value_name)
                .map_err(|error| error.to_string()),
        }
    }

    fn set_registry_string_value(key_path: &str, value_name: &str, value: &str) -> AppResult<()> {
        let key = create_registry_key(key_path, "AUTOSTART_REGISTRY_OPEN_FAILED")?;
        let snapshot = registry_string_value_snapshot(value);

        set_registry_raw_value(&key, value_name, snapshot.value_type, &snapshot.bytes).map_err(
            |error| {
                AppError::new("AUTOSTART_REGISTRY_WRITE_FAILED", "写入开机启动注册表失败")
                    .with_detail("reason", error)
                    .with_detail("commandLine", value.to_string())
            },
        )
    }

    fn registry_string_value_snapshot(value: &str) -> RegistryValueSnapshot {
        let bytes = value
            .encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect();
        RegistryValueSnapshot {
            value_type: REG_SZ,
            bytes,
        }
    }

    fn set_registry_raw_value(
        key: &RegistryKey,
        value_name: &str,
        value_type: u32,
        value: &[u8],
    ) -> Result<(), String> {
        let value_len = u32::try_from(value.len())
            .map_err(|_| "registry value length exceeds u32".to_string())?;
        if value_len > MAX_REGISTRY_VALUE_BYTES {
            return Err(format!(
                "registry value length {value_len} exceeds limit {MAX_REGISTRY_VALUE_BYTES}"
            ));
        }
        let wide_name = to_wide_null(value_name);
        let status = unsafe {
            RegSetValueExW(
                key.0,
                wide_name.as_ptr(),
                0,
                value_type,
                value.as_ptr(),
                value_len,
            )
        };
        if status == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(format!("RegSetValueExW returned {status}"))
        }
    }

    fn delete_registry_value_if_exists(key_path: &str, value_name: &str) -> AppResult<()> {
        let key = match open_registry_key(key_path) {
            Ok(key) => key,
            Err(status) if status == ERROR_FILE_NOT_FOUND => return Ok(()),
            Err(status) => {
                return Err(AppError::new(
                    "AUTOSTART_REGISTRY_OPEN_FAILED",
                    "打开开机启动注册表失败",
                )
                .with_detail("reason", format!("RegOpenKeyExW returned {status}")));
            }
        };

        let wide_name = to_wide_null(value_name);
        let status = unsafe { RegDeleteValueW(key.0, wide_name.as_ptr()) };
        if status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND {
            Ok(())
        } else {
            Err(
                AppError::new("AUTOSTART_REGISTRY_DELETE_FAILED", "删除开机启动注册表失败")
                    .with_detail("reason", format!("RegDeleteValueW returned {status}")),
            )
        }
    }

    fn get_registry_raw_value_if_exists(
        key_path: &str,
        value_name: &str,
    ) -> AppResult<Option<RegistryValueSnapshot>> {
        let key = match open_registry_key_with_access(key_path, KEY_QUERY_VALUE) {
            Ok(key) => key,
            Err(status) if status == ERROR_FILE_NOT_FOUND => return Ok(None),
            Err(status) => {
                return Err(AppError::new(
                    "AUTOSTART_REGISTRY_OPEN_FAILED",
                    "打开开机启动注册表失败",
                )
                .with_detail("reason", format!("RegOpenKeyExW returned {status}")));
            }
        };

        let wide_name = to_wide_null(value_name);
        for attempt in 0..MAX_REGISTRY_READ_ATTEMPTS {
            let mut value_type = 0u32;
            let mut bytes_len = 0u32;
            let status = unsafe {
                RegQueryValueExW(
                    key.0,
                    wide_name.as_ptr(),
                    ptr::null_mut(),
                    &mut value_type,
                    ptr::null_mut(),
                    &mut bytes_len,
                )
            };

            if status == ERROR_FILE_NOT_FOUND {
                return Ok(None);
            }
            if status != ERROR_SUCCESS {
                return Err(AppError::new(
                    "AUTOSTART_REGISTRY_READ_FAILED",
                    "读取开机启动注册表失败",
                )
                .with_detail("reason", format!("RegQueryValueExW returned {status}")));
            }
            if bytes_len == 0 {
                return Ok(Some(RegistryValueSnapshot {
                    value_type,
                    bytes: Vec::new(),
                }));
            }
            validate_registry_value_size(bytes_len)?;

            let mut buffer = vec![0u8; bytes_len as usize];
            let mut actual_len = bytes_len;
            let status = unsafe {
                RegQueryValueExW(
                    key.0,
                    wide_name.as_ptr(),
                    ptr::null_mut(),
                    &mut value_type,
                    buffer.as_mut_ptr(),
                    &mut actual_len,
                )
            };
            if status == ERROR_FILE_NOT_FOUND {
                return Ok(None);
            }
            if status == ERROR_MORE_DATA && attempt + 1 < MAX_REGISTRY_READ_ATTEMPTS {
                continue;
            }
            if status != ERROR_SUCCESS {
                return Err(AppError::new(
                    "AUTOSTART_REGISTRY_READ_FAILED",
                    "读取开机启动注册表失败",
                )
                .with_detail("reason", format!("RegQueryValueExW returned {status}")));
            }
            if actual_len > bytes_len {
                return Err(AppError::new(
                    "AUTOSTART_REGISTRY_READ_FAILED",
                    "读取开机启动注册表时返回了无效长度",
                )
                .with_detail("bufferLength", bytes_len.to_string())
                .with_detail("actualLength", actual_len.to_string()));
            }
            buffer.truncate(actual_len as usize);
            return Ok(Some(RegistryValueSnapshot {
                value_type,
                bytes: buffer,
            }));
        }

        Err(AppError::new(
            "AUTOSTART_REGISTRY_READ_FAILED",
            "读取开机启动注册表时数据持续变化",
        )
        .retryable(true))
    }

    fn validate_registry_value_size(bytes_len: u32) -> AppResult<()> {
        if bytes_len > MAX_REGISTRY_VALUE_BYTES {
            return Err(AppError::new(
                "AUTOSTART_REGISTRY_VALUE_TOO_LARGE",
                "开机启动注册表值异常过大",
            )
            .with_detail("bytes", bytes_len.to_string())
            .with_detail("limit", MAX_REGISTRY_VALUE_BYTES.to_string()));
        }
        Ok(())
    }

    fn decode_registry_string_value(value: &RegistryValueSnapshot) -> Option<String> {
        if value.value_type != REG_SZ && value.value_type != REG_EXPAND_SZ {
            return None;
        }
        let mut chunks = value.bytes.chunks_exact(2);
        let mut wide = chunks
            .by_ref()
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect::<Vec<_>>();
        if !chunks.remainder().is_empty() {
            return None;
        }
        while wide.last().is_some_and(|unit| *unit == 0) {
            wide.pop();
        }
        let text = String::from_utf16(&wide).ok()?.trim().to_string();
        (!text.is_empty()).then_some(text)
    }

    fn create_registry_key(key_path: &str, error_code: &str) -> AppResult<RegistryKey> {
        let wide_path = to_wide_null(key_path);
        let mut key: HKey = 0;
        let status = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                wide_path.as_ptr(),
                0,
                ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                ptr::null(),
                &mut key,
                ptr::null_mut(),
            )
        };

        if status == ERROR_SUCCESS {
            Ok(RegistryKey(key))
        } else {
            Err(AppError::new(error_code, "打开开机启动注册表失败")
                .with_detail("reason", format!("RegCreateKeyExW returned {status}")))
        }
    }

    fn open_registry_key(key_path: &str) -> Result<RegistryKey, i32> {
        open_registry_key_with_access(key_path, KEY_SET_VALUE)
    }

    fn open_registry_key_with_access(key_path: &str, access: u32) -> Result<RegistryKey, i32> {
        let wide_path = to_wide_null(key_path);
        let mut key: HKey = 0;
        let status =
            unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, wide_path.as_ptr(), 0, access, &mut key) };

        if status == ERROR_SUCCESS {
            Ok(RegistryKey(key))
        } else {
            Err(status)
        }
    }

    fn build_registry_run_command_line(app_path: &Path, args: &[&str]) -> String {
        let mut parts = vec![quote_windows_command_arg(&app_path.display().to_string())];
        parts.extend(args.iter().map(|arg| quote_windows_command_arg(arg)));
        parts.join(" ")
    }

    fn resolve_registration_executable(
        current_exe: &Path,
        existing_command_line: Option<&str>,
    ) -> PathBuf {
        if should_keep_existing_release_command(current_exe, existing_command_line) {
            if let Some(existing_executable) =
                existing_command_line.and_then(extract_windows_command_executable)
            {
                return PathBuf::from(existing_executable);
            }
        }
        current_exe.to_path_buf()
    }

    fn should_keep_existing_release_command(
        current_exe: &Path,
        existing_command_line: Option<&str>,
    ) -> bool {
        if !is_development_executable(current_exe) {
            return false;
        }

        let Some(existing_exe) = existing_command_line.and_then(extract_windows_command_executable)
        else {
            return false;
        };

        let existing_exe = Path::new(&existing_exe);
        let file_names_match = current_exe
            .file_name()
            .and_then(|name| name.to_str())
            .zip(existing_exe.file_name().and_then(|name| name.to_str()))
            .is_some_and(|(current, existing)| current.eq_ignore_ascii_case(existing));

        file_names_match && !is_development_executable(existing_exe) && existing_exe.is_file()
    }

    fn is_development_executable(path: &Path) -> bool {
        let normalized = path.display().to_string().replace('/', "\\").to_lowercase();
        normalized.contains("\\src-tauri\\target\\debug\\")
            || normalized.contains("\\src-tauri\\target\\release\\")
    }

    fn extract_windows_command_executable(command_line: &str) -> Option<String> {
        let trimmed = command_line.trim();
        if trimmed.is_empty() {
            return None;
        }

        if let Some(rest) = trimmed.strip_prefix('"') {
            let mut escaped = false;
            let mut executable = String::new();
            for character in rest.chars() {
                if escaped {
                    executable.push(character);
                    escaped = false;
                    continue;
                }
                match character {
                    '\\' => {
                        escaped = true;
                        executable.push(character);
                    }
                    '"' => return Some(executable),
                    _ => executable.push(character),
                }
            }
            return None;
        }

        let lower = trimmed.to_ascii_lowercase();
        for (index, _) in lower.match_indices(".exe") {
            let end = index + 4;
            let is_boundary = trimmed
                .get(end..)
                .and_then(|suffix| suffix.chars().next())
                .is_none_or(char::is_whitespace);
            if is_boundary {
                return Some(trimmed[..end].trim().to_string());
            }
        }

        trimmed.split_whitespace().next().map(str::to_string)
    }

    fn quote_windows_command_arg(value: &str) -> String {
        let mut quoted = String::with_capacity(value.len() + 2);
        quoted.push('"');

        let mut backslash_count = 0usize;
        for character in value.chars() {
            match character {
                '\\' => {
                    backslash_count += 1;
                }
                '"' => {
                    quoted.extend(std::iter::repeat_n('\\', backslash_count * 2 + 1));
                    quoted.push('"');
                    backslash_count = 0;
                }
                _ => {
                    quoted.extend(std::iter::repeat_n('\\', backslash_count));
                    quoted.push(character);
                    backslash_count = 0;
                }
            }
        }

        quoted.extend(std::iter::repeat_n('\\', backslash_count * 2));
        quoted.push('"');
        quoted
    }

    fn to_wide_null(value: &str) -> Vec<u16> {
        OsStr::new(value).encode_wide().chain([0]).collect()
    }

    struct RegistryKey(HKey);

    impl Drop for RegistryKey {
        fn drop(&mut self) {
            unsafe {
                RegCloseKey(self.0);
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use std::path::Path;

        use super::{
            build_registry_run_command_line, decode_registry_string_value,
            extract_windows_command_executable, is_development_executable,
            quote_windows_command_arg, resolve_registration_executable,
            should_keep_existing_release_command, startup_approved_value_is_enabled,
            validate_registry_run_command_line, validate_registry_value_size,
            RegistryValueSnapshot, MAX_REGISTRY_VALUE_BYTES, REG_BINARY, REG_EXPAND_SZ, REG_SZ,
            TASK_MANAGER_ENABLED_VALUE,
        };

        fn registry_string_snapshot(value_type: u32, value: &str) -> RegistryValueSnapshot {
            let bytes = value
                .encode_utf16()
                .chain([0])
                .flat_map(u16::to_le_bytes)
                .collect();
            RegistryValueSnapshot { value_type, bytes }
        }

        #[test]
        fn registry_run_command_quotes_executable_path_and_autostart_arg() {
            let command = build_registry_run_command_line(
                Path::new(r"C:\Users\aka86\AppData\Local\Bexo Studio\Bexo Studio.exe"),
                &["--autostart"],
            );

            assert_eq!(
                command,
                r#""C:\Users\aka86\AppData\Local\Bexo Studio\Bexo Studio.exe" "--autostart""#
            );
        }

        #[test]
        fn command_arg_quote_escapes_embedded_quotes_and_trailing_slash() {
            assert_eq!(
                quote_windows_command_arg(r#"C:\Path With "Quote"\"#),
                r#""C:\Path With \"Quote\"\\""#
            );
        }

        #[test]
        fn command_executable_extracts_quoted_path() {
            assert_eq!(
                extract_windows_command_executable(
                    r#""C:\Users\aka86\AppData\Local\Bexo Studio\bexo-studio.exe" "--autostart""#
                )
                .as_deref(),
                Some(r"C:\Users\aka86\AppData\Local\Bexo Studio\bexo-studio.exe")
            );
        }

        #[test]
        fn command_executable_repairs_legacy_unquoted_path_with_spaces() {
            assert_eq!(
                extract_windows_command_executable(
                    r#"C:\Users\aka86\AppData\Local\Bexo Studio\bexo-studio.exe --autostart"#
                )
                .as_deref(),
                Some(r"C:\Users\aka86\AppData\Local\Bexo Studio\bexo-studio.exe")
            );
        }

        #[test]
        fn registry_string_decode_supports_sz_and_expand_sz_without_pointer_casts() {
            let expected = r#""C:\Program Files\Bexo Studio\bexo-studio.exe" "--autostart""#;
            assert_eq!(
                decode_registry_string_value(&registry_string_snapshot(REG_SZ, expected))
                    .as_deref(),
                Some(expected)
            );
            assert_eq!(
                decode_registry_string_value(&registry_string_snapshot(REG_EXPAND_SZ, expected))
                    .as_deref(),
                Some(expected)
            );
            assert!(decode_registry_string_value(&RegistryValueSnapshot {
                value_type: REG_SZ,
                bytes: vec![0x41],
            })
            .is_none());
        }

        #[test]
        fn startup_approval_matches_locked_auto_launch_compatibility_rules() {
            for state in [0x00, 0x02, 0x06] {
                let mut enabled = TASK_MANAGER_ENABLED_VALUE.to_vec();
                enabled[0] = state;
                assert!(startup_approved_value_is_enabled(&RegistryValueSnapshot {
                    value_type: REG_BINARY,
                    bytes: enabled,
                }));
            }

            let mut disabled = TASK_MANAGER_ENABLED_VALUE.to_vec();
            disabled[0] = 0x03;
            disabled[4] = 0x01;
            assert!(!startup_approved_value_is_enabled(&RegistryValueSnapshot {
                value_type: REG_BINARY,
                bytes: disabled,
            }));
            assert!(!startup_approved_value_is_enabled(
                &registry_string_snapshot(REG_SZ, "enabled")
            ));
        }

        #[test]
        fn registry_run_command_rejects_windows_limit_overflow() {
            assert!(validate_registry_run_command_line(&"x".repeat(260)).is_ok());
            let error = validate_registry_run_command_line(&"x".repeat(261))
                .expect_err("command longer than the documented limit must fail");
            assert_eq!(error.code, "AUTOSTART_COMMAND_TOO_LONG");
        }

        #[test]
        fn registry_value_size_is_bounded_before_allocation() {
            assert!(validate_registry_value_size(MAX_REGISTRY_VALUE_BYTES).is_ok());
            let error = validate_registry_value_size(MAX_REGISTRY_VALUE_BYTES + 1)
                .expect_err("oversized registry value must be rejected");
            assert_eq!(error.code, "AUTOSTART_REGISTRY_VALUE_TOO_LARGE");
        }

        #[test]
        fn development_executable_detects_tauri_target_paths() {
            assert!(is_development_executable(Path::new(
                r"D:\Desktop\rust\BexoStudio\src-tauri\target\debug\bexo-studio.exe"
            )));
            assert!(!is_development_executable(Path::new(
                r"C:\Users\aka86\AppData\Local\Bexo Studio\bexo-studio.exe"
            )));
        }

        #[test]
        fn development_build_keeps_existing_installed_autostart_command() {
            let fake_install_root = std::env::temp_dir().join(format!(
                "bexo-autostart-test-{}-{}",
                std::process::id(),
                uuid::Uuid::new_v4()
            ));
            let fake_install_dir = fake_install_root.join("Bexo Studio");
            std::fs::create_dir_all(&fake_install_dir).expect("create temp install dir");
            let fake_install_exe = fake_install_dir.join("bexo-studio.exe");
            std::fs::write(&fake_install_exe, []).expect("create fake installed exe");

            let current =
                Path::new(r"D:\Desktop\rust\BexoStudio\src-tauri\target\debug\bexo-studio.exe");
            let existing = format!("{} --autostart", fake_install_exe.display());

            assert!(should_keep_existing_release_command(
                current,
                Some(existing.as_str())
            ));
            assert_eq!(
                resolve_registration_executable(current, Some(existing.as_str())),
                fake_install_exe
            );

            let _ = std::fs::remove_dir_all(&fake_install_root);
        }
    }
}

fn validate_preferences(input: AppPreferences) -> AppResult<AppPreferences> {
    Ok(AppPreferences {
        terminal: validate_terminal_preferences(input.terminal)?,
        ide: crate::domain::IdePreferences {
            vscode_path: validate_tool_path(
                "ide.vscodePath",
                input.ide.vscode_path,
                &[
                    "code.cmd",
                    "code.exe",
                    "code.bat",
                    "code-insiders.cmd",
                    "code-insiders.exe",
                    "code-insiders.bat",
                    "codium.cmd",
                    "codium.exe",
                    "codium.bat",
                ],
                "VSCODE_PATH_INVALID",
                "VS Code",
            )?,
            jetbrains_path: validate_tool_path(
                "ide.jetbrainsPath",
                input.ide.jetbrains_path,
                &[
                    "idea64.exe",
                    "idea.exe",
                    "idea.cmd",
                    "idea.bat",
                    "goland64.exe",
                    "goland.exe",
                    "goland.cmd",
                    "goland.bat",
                    "pycharm64.exe",
                    "pycharm.exe",
                    "pycharm.cmd",
                    "pycharm.bat",
                    "webstorm64.exe",
                    "webstorm.exe",
                    "webstorm.cmd",
                    "webstorm.bat",
                    "phpstorm64.exe",
                    "phpstorm.exe",
                    "phpstorm.cmd",
                    "phpstorm.bat",
                    "clion64.exe",
                    "clion.exe",
                    "clion.cmd",
                    "clion.bat",
                    "rider64.exe",
                    "rider.exe",
                    "rider.cmd",
                    "rider.bat",
                    "datagrip64.exe",
                    "datagrip.exe",
                    "datagrip.cmd",
                    "datagrip.bat",
                    "rustrover64.exe",
                    "rustrover.exe",
                    "rustrover.cmd",
                    "rustrover.bat",
                    "studio64.exe",
                    "studio.exe",
                    "studio.cmd",
                    "studio.bat",
                ],
                "JETBRAINS_PATH_INVALID",
                "JetBrains IDE",
            )?,
            custom_editors: validate_custom_editors(input.ide.custom_editors)?,
        },
        workspace: validate_workspace_preferences(input.workspace)?,
        startup: validate_startup_preferences(input.startup)?,
        hotkey: validate_hotkey_preferences(input.hotkey)?,
        tray: input.tray,
        diagnostics: input.diagnostics,
        codex_history: validate_codex_history_view_preferences(input.codex_history)?,
        codex_auth: validate_codex_auth_preferences(input.codex_auth)?,
    })
}

fn validate_terminal_preferences(input: TerminalPreferences) -> AppResult<TerminalPreferences> {
    Ok(TerminalPreferences {
        windows_terminal_path: validate_tool_path(
            "terminal.windowsTerminalPath",
            input.windows_terminal_path,
            &["wt.exe", "wt.cmd", "wt.bat"],
            "WINDOWS_TERMINAL_PATH_INVALID",
            "Windows Terminal",
        )?,
        codex_cli_path: validate_tool_path(
            "terminal.codexCliPath",
            input.codex_cli_path,
            &["codex.exe", "codex.cmd", "codex.bat"],
            "CODEX_PATH_INVALID",
            "Codex CLI",
        )?,
        command_shell: validate_terminal_command_shell(input.command_shell),
        command_templates: validate_command_templates(input.command_templates)?,
    })
}

fn validate_terminal_command_shell(input: TerminalCommandShell) -> TerminalCommandShell {
    input
}

fn validate_startup_preferences(
    input: crate::domain::StartupPreferences,
) -> AppResult<crate::domain::StartupPreferences> {
    Ok(input)
}

fn validate_codex_history_view_preferences(
    input: CodexHistoryViewPreferences,
) -> AppResult<CodexHistoryViewPreferences> {
    Ok(CodexHistoryViewPreferences {
        message_font_family: validate_codex_history_font_family(input.message_font_family)?,
        message_font_size: validate_codex_history_message_font_size(input.message_font_size)?,
    })
}

fn validate_codex_history_font_family(input: String) -> AppResult<String> {
    let trimmed = input.trim();
    if trimmed.chars().count() > CODEX_HISTORY_MAX_FONT_FAMILY_LENGTH {
        return Err(
            AppError::validation("Codex 历史字体名称不能超过 160 个字符")
                .with_detail("field", "codexHistory.messageFontFamily"),
        );
    }
    if trimmed.chars().any(char::is_control) {
        return Err(
            AppError::validation("Codex 历史字体名称不能包含换行或控制字符")
                .with_detail("field", "codexHistory.messageFontFamily"),
        );
    }
    Ok(trimmed.to_string())
}

fn validate_codex_history_message_font_size(input: i32) -> AppResult<i32> {
    if !(CODEX_HISTORY_MIN_MESSAGE_FONT_SIZE..=CODEX_HISTORY_MAX_MESSAGE_FONT_SIZE).contains(&input)
    {
        return Err(
            AppError::validation("Codex 历史字号必须在 10px 到 24px 之间")
                .with_detail("field", "codexHistory.messageFontSize"),
        );
    }
    Ok(input)
}

fn validate_codex_auth_preferences(input: CodexAuthPreferences) -> AppResult<CodexAuthPreferences> {
    Ok(CodexAuthPreferences {
        quota_refresh_interval_seconds: validate_codex_auth_quota_refresh_interval_seconds(
            input.quota_refresh_interval_seconds,
        )?,
        proxy: validate_codex_auth_proxy_preferences(input.proxy)?,
    })
}

fn validate_codex_auth_quota_refresh_interval_seconds(input: i32) -> AppResult<i32> {
    if !(CODEX_AUTH_MIN_QUOTA_REFRESH_INTERVAL_SECONDS
        ..=CODEX_AUTH_MAX_QUOTA_REFRESH_INTERVAL_SECONDS)
        .contains(&input)
    {
        return Err(
            AppError::validation("Codex Auth 额度刷新间隔必须在 10 秒到 3600 秒之间")
                .with_detail("field", "codexAuth.quotaRefreshIntervalSeconds"),
        );
    }
    Ok(input)
}

fn validate_codex_auth_proxy_preferences(
    input: CodexAuthProxyPreferences,
) -> AppResult<CodexAuthProxyPreferences> {
    let mode = input.mode.trim().to_ascii_lowercase();
    if !matches!(mode.as_str(), "system" | "manual" | "disabled") {
        return Err(AppError::validation("Codex Auth 额度代理模式无效")
            .with_detail("field", "codexAuth.proxy.mode")
            .with_detail("allowed", "system, manual, disabled"));
    }

    let manual_proxy_url = input.manual_proxy_url.trim().to_string();
    if manual_proxy_url.chars().count() > CODEX_AUTH_MAX_MANUAL_PROXY_URL_LENGTH {
        return Err(
            AppError::validation("Codex Auth 手动代理地址不能超过 512 个字符")
                .with_detail("field", "codexAuth.proxy.manualProxyUrl"),
        );
    }
    if manual_proxy_url.chars().any(char::is_control) {
        return Err(
            AppError::validation("Codex Auth 手动代理地址不能包含换行或控制字符")
                .with_detail("field", "codexAuth.proxy.manualProxyUrl"),
        );
    }

    if mode == "manual" && manual_proxy_url.is_empty() {
        return Err(AppError::validation("手动代理模式必须填写代理地址")
            .with_detail("field", "codexAuth.proxy.manualProxyUrl"));
    }
    if !manual_proxy_url.is_empty() {
        let lower_proxy_url = manual_proxy_url.to_ascii_lowercase();
        if !lower_proxy_url.starts_with("http://")
            && !lower_proxy_url.starts_with("https://")
            && !lower_proxy_url.starts_with("socks5://")
            && !lower_proxy_url.starts_with("socks5h://")
        {
            return Err(AppError::validation(
                "Codex Auth 手动代理只支持 http、https、socks5、socks5h",
            )
            .with_detail("field", "codexAuth.proxy.manualProxyUrl"));
        }
    }

    Ok(CodexAuthProxyPreferences {
        mode: if mode.is_empty() {
            DEFAULT_CODEX_AUTH_PROXY_MODE.to_string()
        } else {
            mode
        },
        manual_proxy_url,
    })
}

fn validate_workspace_preferences(
    input: crate::domain::WorkspacePreferences,
) -> AppResult<crate::domain::WorkspacePreferences> {
    let selected_workspace_ids = validate_workspace_id_preferences(
        "workspace.selectedWorkspaceIds",
        input.selected_workspace_ids,
    )?;
    let pinned_workspace_ids = validate_workspace_id_preferences(
        "workspace.pinnedWorkspaceIds",
        input.pinned_workspace_ids,
    )?;

    Ok(crate::domain::WorkspacePreferences {
        selected_workspace_ids,
        pinned_workspace_ids,
    })
}

fn validate_workspace_id_preferences(
    field_prefix: &str,
    values: Vec<String>,
) -> AppResult<Vec<String>> {
    if values.len() > 500 {
        return Err(AppError::validation(format!(
            "{field_prefix} cannot exceed 500 entries"
        )));
    }

    let mut seen_ids = HashSet::with_capacity(values.len());
    let mut normalized_ids = Vec::with_capacity(values.len());

    for (index, workspace_id) in values.into_iter().enumerate() {
        let trimmed = workspace_id.trim();
        let field = format!("{field_prefix}[{index}]");

        if trimmed.is_empty() {
            continue;
        }

        if trimmed.len() > 64 {
            return Err(
                AppError::validation("工作区 ID 不能超过 64 个字符").with_detail("field", field)
            );
        }

        if seen_ids.insert(trimmed.to_string()) {
            normalized_ids.push(trimmed.to_string());
        }
    }

    Ok(normalized_ids)
}

fn validate_hotkey_preferences(input: HotkeyPreferences) -> AppResult<HotkeyPreferences> {
    let screenshot_capture = validate_hotkey_shortcut(
        HotkeyAction::ScreenshotCapture,
        input.screenshot_capture,
        true,
    )?
    .unwrap_or_else(|| HotkeyPreferences::default().screenshot_capture);
    let voice_input_toggle =
        validate_hotkey_shortcut_option(HotkeyAction::VoiceInputToggle, input.voice_input_toggle)?;
    let voice_input_hold =
        validate_hotkey_shortcut_option(HotkeyAction::VoiceInputHold, input.voice_input_hold)?;
    let prompt_quick_paste_slots =
        validate_prompt_quick_paste_slots(input.prompt_quick_paste_slots)?;

    let mut seen_shortcuts: HashSet<String> = HashSet::new();
    let mut ensure_unique = |action: HotkeyAction, shortcut: Option<&str>| -> AppResult<()> {
        let Some(shortcut) = shortcut else {
            return Ok(());
        };

        let normalized = shortcut.to_ascii_lowercase();
        if !seen_shortcuts.insert(normalized) {
            return Err(AppError::new(
                "HOTKEY_DUPLICATE_SHORTCUT",
                "热键冲突：不同动作不能使用同一组合键",
            )
            .with_detail("field", action.preference_field().to_string())
            .with_detail("shortcut", shortcut.to_string()));
        }

        Ok(())
    };

    ensure_unique(
        HotkeyAction::ScreenshotCapture,
        Some(screenshot_capture.as_str()),
    )?;
    ensure_unique(
        HotkeyAction::VoiceInputToggle,
        voice_input_toggle.as_deref(),
    )?;
    ensure_unique(HotkeyAction::VoiceInputHold, voice_input_hold.as_deref())?;
    for slot in &prompt_quick_paste_slots {
        if slot.enabled {
            let action = HotkeyAction::from_prompt_quick_paste_slot(slot.slot)
                .expect("validated prompt quick paste slot must map to an action");
            ensure_unique(action, Some(slot.shortcut.as_str()))?;
        }
    }

    Ok(HotkeyPreferences {
        screenshot_capture,
        voice_input_toggle,
        voice_input_hold,
        prompt_quick_paste_slots,
    })
}

fn validate_prompt_quick_paste_slots(
    mut input: Vec<PromptQuickPasteHotkeySlot>,
) -> AppResult<Vec<PromptQuickPasteHotkeySlot>> {
    if input.len() != PROMPT_QUICK_PASTE_SLOT_COUNT {
        return Err(AppError::new(
            "PROMPT_QUICK_PASTE_SLOTS_INVALID",
            "Prompt 快速粘贴必须包含 5 个配置槽位",
        )
        .with_detail("field", "hotkey.promptQuickPasteSlots")
        .with_detail("count", input.len().to_string()));
    }

    input.sort_by_key(|slot| slot.slot);
    let mut validated = Vec::with_capacity(PROMPT_QUICK_PASTE_SLOT_COUNT);
    for (index, slot) in input.into_iter().enumerate() {
        let expected_slot = (index + 1) as u8;
        if slot.slot != expected_slot {
            return Err(AppError::new(
                "PROMPT_QUICK_PASTE_SLOTS_INVALID",
                "Prompt 快速粘贴槽位编号必须为 1 到 5 且不得重复",
            )
            .with_detail("field", "hotkey.promptQuickPasteSlots")
            .with_detail("slot", slot.slot.to_string()));
        }
        let action = HotkeyAction::from_prompt_quick_paste_slot(slot.slot)
            .expect("validated prompt quick paste slot must map to an action");
        let shortcut = validate_hotkey_shortcut(action, slot.shortcut, true)?
            .expect("required hotkey validation must return a shortcut");
        let prompt_id = match slot.prompt_id {
            Some(raw) if !raw.trim().is_empty() => Some(
                uuid::Uuid::parse_str(raw.trim())
                    .map_err(|_| {
                        AppError::new(
                            "PROMPT_QUICK_PASTE_PROMPT_ID_INVALID",
                            "绑定的 Prompt 标识无效",
                        )
                        .with_detail("field", action.preference_field())
                        .with_detail("slot", slot.slot.to_string())
                    })?
                    .to_string(),
            ),
            _ => None,
        };
        if slot.enabled && prompt_id.is_none() {
            return Err(AppError::new(
                "PROMPT_QUICK_PASTE_PROMPT_REQUIRED",
                "启用快速粘贴前必须选择 Prompt",
            )
            .with_detail("field", action.preference_field())
            .with_detail("slot", slot.slot.to_string()));
        }
        validated.push(PromptQuickPasteHotkeySlot {
            slot: slot.slot,
            enabled: slot.enabled,
            prompt_id,
            shortcut,
        });
    }
    Ok(validated)
}

fn validate_hotkey_shortcut_option(
    action: HotkeyAction,
    value: Option<String>,
) -> AppResult<Option<String>> {
    let Some(raw) = value else {
        return Ok(None);
    };

    validate_hotkey_shortcut(action, raw, false)
}

fn validate_hotkey_shortcut(
    action: HotkeyAction,
    value: String,
    required: bool,
) -> AppResult<Option<String>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        if required {
            return Err(
                AppError::new("HOTKEY_SHORTCUT_REQUIRED", "截图热键不能为空")
                    .with_detail("field", action.preference_field().to_string()),
            );
        }

        return Ok(None);
    }

    if trimmed.len() > 64 {
        return Err(
            AppError::new("HOTKEY_SHORTCUT_TOO_LONG", "热键长度不能超过 64 个字符")
                .with_detail("field", action.preference_field().to_string()),
        );
    }

    classify_supported_shortcut(trimmed).map_err(|error| {
        AppError::new("HOTKEY_SHORTCUT_INVALID", "热键格式无效")
            .with_detail("field", action.preference_field().to_string())
            .with_detail("shortcut", trimmed.to_string())
            .with_detail("reason", error)
    })?;

    validate_hotkey_shortcut_semantics(action, trimmed).map_err(|reason| {
        AppError::new("HOTKEY_SHORTCUT_INVALID", "热键格式无效")
            .with_detail("field", action.preference_field().to_string())
            .with_detail("shortcut", trimmed.to_string())
            .with_detail("reason", reason)
    })?;

    Ok(Some(trimmed.to_string()))
}

fn validate_hotkey_shortcut_semantics(action: HotkeyAction, value: &str) -> Result<(), String> {
    let tokens = split_hotkey_shortcut_tokens(value);
    if tokens.is_empty() {
        return Err("热键不能为空".to_string());
    }

    let has_non_modifier = tokens.iter().any(|token| !is_hotkey_modifier_token(token));
    if action == HotkeyAction::ScreenshotCapture && !has_non_modifier {
        return Err("截图热键至少包含一个非修饰键，例如 Ctrl+Shift+X".to_string());
    }

    if matches!(
        action,
        HotkeyAction::PromptQuickPaste1
            | HotkeyAction::PromptQuickPaste2
            | HotkeyAction::PromptQuickPaste3
            | HotkeyAction::PromptQuickPaste4
            | HotkeyAction::PromptQuickPaste5
    ) && !has_non_modifier
    {
        return Err("Prompt 快速粘贴热键至少包含一个非修饰键，例如 Ctrl+Alt+Shift+1".to_string());
    }

    if matches!(
        action,
        HotkeyAction::VoiceInputToggle | HotkeyAction::VoiceInputHold
    ) && !has_non_modifier
        && !tokens
            .iter()
            .all(|token| is_side_specific_hotkey_modifier_token(token))
    {
        return Err(
            "语音输入热键如果只使用修饰键，必须使用单侧修饰键，例如 RAlt / LWin+LAlt".to_string(),
        );
    }

    Ok(())
}

fn split_hotkey_shortcut_tokens(value: &str) -> Vec<String> {
    value
        .split('+')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

fn is_hotkey_modifier_token(token: &str) -> bool {
    matches!(
        token,
        "ctrl"
            | "control"
            | "lctrl"
            | "leftctrl"
            | "leftcontrol"
            | "rctrl"
            | "rightctrl"
            | "rightcontrol"
            | "alt"
            | "altgraph"
            | "lalt"
            | "leftalt"
            | "ralt"
            | "rightalt"
            | "shift"
            | "lshift"
            | "leftshift"
            | "rshift"
            | "rightshift"
            | "super"
            | "meta"
            | "win"
            | "windows"
            | "lwin"
            | "leftwin"
            | "leftwindows"
            | "rwin"
            | "rightwin"
            | "rightwindows"
            | "command"
            | "commandorcontrol"
            | "cmd"
            | "cmdorctrl"
    )
}

fn is_side_specific_hotkey_modifier_token(token: &str) -> bool {
    matches!(
        token,
        "lctrl"
            | "leftctrl"
            | "leftcontrol"
            | "rctrl"
            | "rightctrl"
            | "rightcontrol"
            | "lalt"
            | "leftalt"
            | "ralt"
            | "rightalt"
            | "lshift"
            | "leftshift"
            | "rshift"
            | "rightshift"
            | "lwin"
            | "leftwin"
            | "leftwindows"
            | "rwin"
            | "rightwin"
            | "rightwindows"
    )
}

fn validate_command_templates(
    templates: Vec<crate::domain::TerminalCommandTemplate>,
) -> AppResult<Vec<crate::domain::TerminalCommandTemplate>> {
    if templates.len() > 100 {
        return Err(AppError::validation(
            "terminal.commandTemplates cannot exceed 100 entries",
        ));
    }

    let mut seen_ids = HashSet::with_capacity(templates.len());
    let mut validated = Vec::with_capacity(templates.len());

    for (index, template) in templates.into_iter().enumerate() {
        let field_prefix = format!("terminal.commandTemplates[{index}]");
        let id = validate_template_text_field(
            &format!("{field_prefix}.id"),
            template.id,
            64,
            "模板 ID",
        )?;

        if !seen_ids.insert(id.clone()) {
            return Err(
                AppError::validation("terminal command template ids must be unique")
                    .with_detail("field", format!("{field_prefix}.id"))
                    .with_detail("id", id),
            );
        }

        let name = validate_template_text_field(
            &format!("{field_prefix}.name"),
            template.name,
            80,
            "模板名称",
        )?;
        let command_line = validate_template_command_line(
            &format!("{field_prefix}.commandLine"),
            template.command_line,
        )?;
        let sort_order = validate_template_sort_order(
            &format!("{field_prefix}.sortOrder"),
            template.sort_order,
        )?;

        validated.push(crate::domain::TerminalCommandTemplate {
            id,
            name,
            command_line,
            sort_order,
        });
    }

    validated.sort_by(|left, right| left.sort_order.cmp(&right.sort_order));
    for (index, template) in validated.iter_mut().enumerate() {
        template.sort_order = index as i64;
    }

    Ok(validated)
}

fn validate_custom_editors(
    editors: Vec<crate::domain::CustomEditorPreference>,
) -> AppResult<Vec<crate::domain::CustomEditorPreference>> {
    if editors.len() > 100 {
        return Err(AppError::validation(
            "ide.customEditors cannot exceed 100 entries",
        ));
    }

    let mut seen_ids = HashSet::with_capacity(editors.len());
    let mut validated = Vec::with_capacity(editors.len());

    for (index, editor) in editors.into_iter().enumerate() {
        let field_prefix = format!("ide.customEditors[{index}]");
        let id = validate_template_text_field(
            &format!("{field_prefix}.id"),
            editor.id,
            64,
            "编辑器 ID",
        )?;

        if !seen_ids.insert(id.clone()) {
            return Err(AppError::validation("custom editor ids must be unique")
                .with_detail("field", format!("{field_prefix}.id"))
                .with_detail("id", id));
        }

        let name = validate_template_text_field(
            &format!("{field_prefix}.name"),
            editor.name,
            80,
            "编辑器名称",
        )?;
        let command =
            validate_custom_editor_command(&format!("{field_prefix}.command"), editor.command)?;

        validated.push(crate::domain::CustomEditorPreference { id, name, command });
    }

    Ok(validated)
}

fn validate_custom_editor_command(field: &str, value: String) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(
            AppError::validation("编辑器命令不能为空").with_detail("field", field.to_string())
        );
    }

    if trimmed.len() > 320 {
        return Err(AppError::validation("编辑器命令不能超过 320 个字符")
            .with_detail("field", field.to_string()));
    }

    if trimmed.contains('\n') || trimmed.contains('\r') {
        return Err(
            AppError::validation("编辑器命令必须是单行").with_detail("field", field.to_string())
        );
    }

    let command_path = PathBuf::from(trimmed);
    if command_path.is_absolute() {
        let metadata = fs::metadata(&command_path).map_err(|error| {
            AppError::validation("编辑器命令路径不存在")
                .with_detail("field", field.to_string())
                .with_detail("command", trimmed.to_string())
                .with_detail("reason", error.to_string())
        })?;

        if !metadata.is_file() {
            return Err(AppError::validation("编辑器命令必须指向可执行文件")
                .with_detail("field", field.to_string())
                .with_detail("command", trimmed.to_string()));
        }

        return Ok(trimmed.to_string());
    }

    if trimmed.split_whitespace().count() > 1 {
        return Err(
            AppError::validation("编辑器命令不能包含参数，请只填写命令名")
                .with_detail("field", field.to_string()),
        );
    }

    Ok(trimmed.to_string())
}

fn validate_tool_path(
    field: &str,
    value: Option<String>,
    candidates: &[&str],
    error_code: &str,
    label: &str,
) -> AppResult<Option<String>> {
    let Some(raw) = value else {
        return Ok(None);
    };

    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.len() > 320 {
        return Err(AppError::validation(format!(
            "{field} exceeds 320 characters"
        )));
    }

    match resolve_configured_executable(trimmed, candidates, error_code, label) {
        Ok(_) => Ok(Some(trimmed.to_string())),
        Err(error) => Err(error.with_detail("field", field.to_string())),
    }
}

fn validate_template_text_field(
    field: &str,
    value: String,
    max_length: usize,
    label: &str,
) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation(format!("{label}不能为空"))
            .with_detail("field", field.to_string()));
    }

    if trimmed.len() > max_length {
        return Err(
            AppError::validation(format!("{label}不能超过 {max_length} 个字符"))
                .with_detail("field", field.to_string()),
        );
    }

    Ok(trimmed.to_string())
}

fn validate_template_command_line(field: &str, value: String) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(
            AppError::validation("终端命令不能为空").with_detail("field", field.to_string())
        );
    }

    if trimmed.len() > 512 {
        return Err(AppError::validation("终端命令不能超过 512 个字符")
            .with_detail("field", field.to_string()));
    }

    if trimmed.contains('\n') || trimmed.contains('\r') {
        return Err(
            AppError::validation("终端命令必须是单行").with_detail("field", field.to_string())
        );
    }

    validate_balanced_terminal_quotes(trimmed)
        .map_err(|error| error.with_detail("field", field.to_string()))?;

    Ok(trimmed.to_string())
}

fn validate_template_sort_order(field: &str, value: i64) -> AppResult<i64> {
    if value < 0 {
        return Err(
            AppError::validation("模板排序不能小于 0").with_detail("field", field.to_string())
        );
    }

    Ok(value)
}

fn validate_balanced_terminal_quotes(command_line: &str) -> AppResult<()> {
    let mut quote: Option<char> = None;
    let mut escaped = false;

    for character in command_line.chars() {
        if escaped {
            escaped = false;
            continue;
        }

        if character == '\\' {
            escaped = true;
            continue;
        }

        if let Some(active_quote) = quote {
            if character == active_quote {
                quote = None;
            }
            continue;
        }

        if character == '"' || character == '\'' {
            quote = Some(character);
        }
    }

    if quote.is_some() {
        return Err(AppError::validation("终端命令包含未闭合的引号"));
    }

    Ok(())
}

fn build_codex_home_directory_info(
    path: PathBuf,
    source: &str,
) -> crate::domain::CodexHomeDirectoryInfo {
    crate::domain::CodexHomeDirectoryInfo {
        path: Some(path.display().to_string()),
        source: source.to_string(),
        exists: path.is_dir(),
    }
}

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use super::{
        apply_preferences_patch, attach_preference_rollback_failures,
        build_codex_home_directory_info, migrate_legacy_preferences,
        sanitize_codex_auth_preferences, sanitize_codex_history_view_preferences,
        sanitize_hotkey_preferences, validate_codex_auth_proxy_preferences,
        validate_codex_auth_quota_refresh_interval_seconds,
        validate_codex_history_message_font_size, validate_hotkey_preferences,
        validate_hotkey_shortcut,
    };
    use crate::domain::{
        AppPreferences, AppPreferencesPatch, CodexAuthPreferences, CodexAuthProxyPreferences,
        CodexHistoryViewPreferences, HotkeyAction, PromptQuickPasteHotkeySlot,
        WorkspacePreferencesPatch, DEFAULT_CODEX_AUTH_PROXY_MODE,
        DEFAULT_CODEX_AUTH_QUOTA_REFRESH_INTERVAL_SECONDS, DEFAULT_CODEX_HISTORY_MESSAGE_FONT_SIZE,
        DEFAULT_PROMPT_QUICK_PASTE_HOTKEYS, DEFAULT_SCREENSHOT_CAPTURE_HOTKEY,
        EARLIER_DEFAULT_SCREENSHOT_CAPTURE_HOTKEY, LEGACY_SCREENSHOT_CAPTURE_HOTKEY,
        PREVIOUS_DEFAULT_SCREENSHOT_CAPTURE_HOTKEY, PROMPT_QUICK_PASTE_SLOT_COUNT,
    };
    use crate::error::AppError;

    #[test]
    fn preferences_patch_merges_against_latest_workspace_state() {
        let mut preferences = AppPreferences::default();
        preferences.workspace.selected_workspace_ids = vec!["selected-a".to_string()];
        preferences.workspace.pinned_workspace_ids = vec!["pinned-a".to_string()];

        let preferences = apply_preferences_patch(
            preferences,
            AppPreferencesPatch {
                workspace: Some(WorkspacePreferencesPatch {
                    selected_workspace_ids: Some(vec!["selected-b".to_string()]),
                    pinned_workspace_ids: None,
                }),
                ..AppPreferencesPatch::default()
            },
        );
        let preferences = apply_preferences_patch(
            preferences,
            AppPreferencesPatch {
                workspace: Some(WorkspacePreferencesPatch {
                    selected_workspace_ids: None,
                    pinned_workspace_ids: Some(vec!["pinned-b".to_string()]),
                }),
                ..AppPreferencesPatch::default()
            },
        );

        assert_eq!(
            preferences.workspace.selected_workspace_ids,
            vec!["selected-b".to_string()]
        );
        assert_eq!(
            preferences.workspace.pinned_workspace_ids,
            vec!["pinned-b".to_string()]
        );
    }

    #[test]
    fn preferences_patch_does_not_replace_unrelated_domains() {
        let mut preferences = AppPreferences::default();
        preferences.workspace.pinned_workspace_ids = vec!["pinned".to_string()];

        let patched = apply_preferences_patch(
            preferences,
            AppPreferencesPatch {
                startup: Some(crate::domain::StartupPreferences {
                    launch_at_login: true,
                    start_silently: true,
                }),
                ..AppPreferencesPatch::default()
            },
        );

        assert!(patched.startup.launch_at_login);
        assert!(patched.startup.start_silently);
        assert_eq!(
            patched.workspace.pinned_workspace_ids,
            vec!["pinned".to_string()]
        );
    }

    #[test]
    fn preference_rollback_failures_preserve_error_and_mark_unknown_consistency() {
        let error = AppError::new("PREFERENCES_STORE_WRITE_FAILED", "保存设置失败");
        let error =
            attach_preference_rollback_failures(error, vec!["store: write failed".to_string()]);

        assert_eq!(error.code, "PREFERENCES_STORE_WRITE_FAILED");
        assert_eq!(error.message, "保存设置失败");
        assert_eq!(
            error
                .details
                .as_ref()
                .and_then(|details| details.get("rollbackFailure"))
                .map(String::as_str),
            Some("store: write failed")
        );
        assert_eq!(
            error
                .details
                .as_ref()
                .and_then(|details| details.get("stateConsistency"))
                .map(String::as_str),
            Some("unknown")
        );
    }

    #[test]
    fn codex_home_directory_info_marks_existing_directory() {
        let directory = env::temp_dir().join(format!("bexo-codex-home-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&directory).expect("create codex home directory");
        let directory_path = directory.display().to_string();

        let resolved = build_codex_home_directory_info(directory.clone(), "env");
        assert_eq!(resolved.path.as_deref(), Some(directory_path.as_str()));
        assert_eq!(resolved.source, "env");
        assert!(resolved.exists);
    }

    #[test]
    fn codex_home_directory_info_marks_missing_directory() {
        let directory =
            env::temp_dir().join(format!("bexo-codex-home-missing-{}", uuid::Uuid::new_v4()));
        let directory_path = directory.display().to_string();

        let resolved = build_codex_home_directory_info(directory.clone(), "default");
        assert_eq!(resolved.path.as_deref(), Some(directory_path.as_str()));
        assert_eq!(resolved.source, "default");
        assert!(!resolved.exists);
    }

    #[test]
    fn migrate_legacy_preferences_updates_legacy_screenshot_hotkey() {
        let mut preferences = AppPreferences::default();
        preferences.hotkey.screenshot_capture = LEGACY_SCREENSHOT_CAPTURE_HOTKEY.to_string();

        let migrated = migrate_legacy_preferences(preferences);
        assert!(migrated.changed);
        assert_eq!(
            migrated.preferences.hotkey.screenshot_capture,
            DEFAULT_SCREENSHOT_CAPTURE_HOTKEY
        );
    }

    #[test]
    fn migrate_legacy_preferences_preserves_custom_screenshot_hotkey() {
        let mut preferences = AppPreferences::default();
        preferences.hotkey.screenshot_capture = "Ctrl+Shift+Q".to_string();

        let migrated = migrate_legacy_preferences(preferences);
        assert!(!migrated.changed);
        assert_eq!(
            migrated.preferences.hotkey.screenshot_capture,
            "Ctrl+Shift+Q"
        );
    }

    #[test]
    fn codex_history_font_size_default_is_twelve_pixels() {
        let preferences = CodexHistoryViewPreferences::default();
        assert_eq!(
            preferences.message_font_size,
            DEFAULT_CODEX_HISTORY_MESSAGE_FONT_SIZE
        );
        assert_eq!(preferences.message_font_size, 12);
    }

    #[test]
    fn codex_history_font_size_validation_rejects_out_of_range_values() {
        let error = validate_codex_history_message_font_size(9)
            .expect_err("font size below range should be rejected");
        assert_eq!(error.code, "VALIDATION_ERROR");
        assert_eq!(
            error
                .details
                .as_ref()
                .and_then(|details| details.get("field"))
                .map(String::as_str),
            Some("codexHistory.messageFontSize")
        );
    }

    #[test]
    fn codex_history_preferences_repair_restores_invalid_size() {
        let repaired = sanitize_codex_history_view_preferences(CodexHistoryViewPreferences {
            message_font_family: "  Microsoft YaHei  ".to_string(),
            message_font_size: -2,
        });

        assert!(repaired.changed);
        assert_eq!(repaired.preferences.message_font_family, "Microsoft YaHei");
        assert_eq!(
            repaired.preferences.message_font_size,
            DEFAULT_CODEX_HISTORY_MESSAGE_FONT_SIZE
        );
    }

    #[test]
    fn codex_auth_quota_refresh_interval_default_is_sixty_seconds() {
        let preferences = CodexAuthPreferences::default();
        assert_eq!(
            preferences.quota_refresh_interval_seconds,
            DEFAULT_CODEX_AUTH_QUOTA_REFRESH_INTERVAL_SECONDS
        );
        assert_eq!(preferences.quota_refresh_interval_seconds, 60);
    }

    #[test]
    fn codex_auth_quota_refresh_interval_validation_rejects_out_of_range_values() {
        let error = validate_codex_auth_quota_refresh_interval_seconds(9)
            .expect_err("refresh interval below range should be rejected");
        assert_eq!(error.code, "VALIDATION_ERROR");
        assert_eq!(
            error
                .details
                .as_ref()
                .and_then(|details| details.get("field"))
                .map(String::as_str),
            Some("codexAuth.quotaRefreshIntervalSeconds")
        );
    }

    #[test]
    fn codex_auth_preferences_repair_restores_invalid_interval() {
        let repaired = sanitize_codex_auth_preferences(CodexAuthPreferences {
            quota_refresh_interval_seconds: 0,
            proxy: CodexAuthProxyPreferences::default(),
        });

        assert!(repaired.changed);
        assert_eq!(
            repaired.preferences.quota_refresh_interval_seconds,
            DEFAULT_CODEX_AUTH_QUOTA_REFRESH_INTERVAL_SECONDS
        );
    }

    #[test]
    fn codex_auth_proxy_default_uses_system_proxy() {
        let preferences = CodexAuthProxyPreferences::default();
        assert_eq!(preferences.mode, DEFAULT_CODEX_AUTH_PROXY_MODE);
        assert_eq!(preferences.mode, "system");
        assert!(preferences.manual_proxy_url.is_empty());
    }

    #[test]
    fn codex_auth_proxy_validation_accepts_manual_http_proxy() {
        let validated = validate_codex_auth_proxy_preferences(CodexAuthProxyPreferences {
            mode: "manual".to_string(),
            manual_proxy_url: "  http://127.0.0.1:7890  ".to_string(),
        })
        .expect("manual HTTP proxy should be valid");

        assert_eq!(validated.mode, "manual");
        assert_eq!(validated.manual_proxy_url, "http://127.0.0.1:7890");
    }

    #[test]
    fn codex_auth_proxy_validation_rejects_manual_proxy_without_supported_scheme() {
        let error = validate_codex_auth_proxy_preferences(CodexAuthProxyPreferences {
            mode: "manual".to_string(),
            manual_proxy_url: "127.0.0.1:7890".to_string(),
        })
        .expect_err("manual proxy without scheme should be rejected");

        assert_eq!(error.code, "VALIDATION_ERROR");
        assert_eq!(
            error
                .details
                .as_ref()
                .and_then(|details| details.get("field"))
                .map(String::as_str),
            Some("codexAuth.proxy.manualProxyUrl")
        );
    }

    #[test]
    fn migrate_legacy_preferences_updates_previous_default_screenshot_hotkey() {
        let mut preferences = AppPreferences::default();
        preferences.hotkey.screenshot_capture =
            PREVIOUS_DEFAULT_SCREENSHOT_CAPTURE_HOTKEY.to_string();

        let migrated = migrate_legacy_preferences(preferences);
        assert!(migrated.changed);
        assert_eq!(
            migrated.preferences.hotkey.screenshot_capture,
            DEFAULT_SCREENSHOT_CAPTURE_HOTKEY
        );
    }

    #[test]
    fn validate_hotkey_shortcut_rejects_modifier_only_screenshot_shortcut() {
        let error = validate_hotkey_shortcut(
            HotkeyAction::ScreenshotCapture,
            "LCtrl+LShift".to_string(),
            true,
        )
        .expect_err("modifier-only screenshot shortcut should be rejected");
        assert_eq!(error.code, "HOTKEY_SHORTCUT_INVALID");
        assert_eq!(
            error
                .details
                .as_ref()
                .and_then(|details| details.get("reason"))
                .map(String::as_str),
            Some("截图热键至少包含一个非修饰键，例如 Ctrl+Shift+X")
        );
    }

    #[test]
    fn validate_hotkey_shortcut_accepts_ralt_for_voice_toggle() {
        let validated =
            validate_hotkey_shortcut(HotkeyAction::VoiceInputToggle, "RAlt".to_string(), false)
                .expect("voice toggle should allow side-specific modifier-only shortcut");
        assert_eq!(validated.as_deref(), Some("RAlt"));
    }

    #[test]
    fn sanitize_hotkey_preferences_repairs_invalid_capture_shortcut() {
        let mut preferences = AppPreferences::default();
        preferences.hotkey.screenshot_capture = "LCtrl+LShift".to_string();

        let repaired = sanitize_hotkey_preferences(preferences.hotkey);
        assert!(repaired.changed);
        assert_eq!(
            repaired.preferences.screenshot_capture,
            DEFAULT_SCREENSHOT_CAPTURE_HOTKEY
        );
    }

    #[test]
    fn old_preferences_receive_five_disabled_prompt_quick_paste_slots() {
        let preferences: AppPreferences = serde_json::from_value(serde_json::json!({
            "hotkey": {
                "screenshotCapture": DEFAULT_SCREENSHOT_CAPTURE_HOTKEY
            }
        }))
        .expect("legacy preferences should deserialize with defaults");

        assert_eq!(
            preferences.hotkey.prompt_quick_paste_slots.len(),
            PROMPT_QUICK_PASTE_SLOT_COUNT
        );
        for (index, slot) in preferences
            .hotkey
            .prompt_quick_paste_slots
            .iter()
            .enumerate()
        {
            assert_eq!(slot.slot, (index + 1) as u8);
            assert!(!slot.enabled);
            assert!(slot.prompt_id.is_none());
            assert_eq!(slot.shortcut, DEFAULT_PROMPT_QUICK_PASTE_HOTKEYS[index]);
        }
    }

    #[test]
    fn validate_prompt_quick_paste_requires_prompt_before_enable() {
        let mut preferences = AppPreferences::default();
        preferences.hotkey.prompt_quick_paste_slots[0].enabled = true;

        let error = validate_hotkey_preferences(preferences.hotkey)
            .expect_err("enabled slot without a prompt must fail");
        assert_eq!(error.code, "PROMPT_QUICK_PASTE_PROMPT_REQUIRED");
    }

    #[test]
    fn validate_prompt_quick_paste_rejects_active_shortcut_conflicts() {
        let mut preferences = AppPreferences::default();
        let slot = &mut preferences.hotkey.prompt_quick_paste_slots[0];
        slot.enabled = true;
        slot.prompt_id = Some(uuid::Uuid::new_v4().to_string());
        slot.shortcut = DEFAULT_SCREENSHOT_CAPTURE_HOTKEY.to_string();

        let error = validate_hotkey_preferences(preferences.hotkey)
            .expect_err("active slots must not conflict with screenshot hotkey");
        assert_eq!(error.code, "HOTKEY_DUPLICATE_SHORTCUT");
    }

    #[test]
    fn sanitize_prompt_quick_paste_repairs_invalid_slot_without_losing_defaults() {
        let mut preferences = AppPreferences::default();
        preferences.hotkey.prompt_quick_paste_slots[0] = PromptQuickPasteHotkeySlot {
            slot: 1,
            enabled: true,
            prompt_id: Some("not-a-uuid".to_string()),
            shortcut: "modifier-only".to_string(),
        };

        let repaired = sanitize_hotkey_preferences(preferences.hotkey);
        assert!(repaired.changed);
        assert_eq!(
            repaired.preferences.prompt_quick_paste_slots.len(),
            PROMPT_QUICK_PASTE_SLOT_COUNT
        );
        assert!(!repaired.preferences.prompt_quick_paste_slots[0].enabled);
        assert!(repaired.preferences.prompt_quick_paste_slots[0]
            .prompt_id
            .is_none());
        assert_eq!(
            repaired.preferences.prompt_quick_paste_slots[0].shortcut,
            DEFAULT_PROMPT_QUICK_PASTE_HOTKEYS[0]
        );
    }

    #[test]
    fn migrate_legacy_preferences_updates_earlier_default_screenshot_hotkey() {
        let mut preferences = AppPreferences::default();
        preferences.hotkey.screenshot_capture =
            EARLIER_DEFAULT_SCREENSHOT_CAPTURE_HOTKEY.to_string();

        let migrated = migrate_legacy_preferences(preferences);
        assert!(migrated.changed);
        assert_eq!(
            migrated.preferences.hotkey.screenshot_capture,
            DEFAULT_SCREENSHOT_CAPTURE_HOTKEY
        );
    }
}
