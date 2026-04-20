use std::{
    collections::HashSet,
    env, fs,
    path::PathBuf,
    sync::{Arc, RwLock},
};

use chrono::Utc;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_store::{Store, StoreExt};

use crate::{
    adapters::{resolve_configured_executable, IdeAdapter, JetBrainsAdapter, VSCodeAdapter},
    domain::{
        AppPreferences, EditorPathDetectionResult, HotkeyAction, HotkeyPreferences,
        DEFAULT_SCREENSHOT_CAPTURE_HOTKEY, EARLIER_DEFAULT_SCREENSHOT_CAPTURE_HOTKEY,
        LEGACY_SCREENSHOT_CAPTURE_HOTKEY, PREVIOUS_DEFAULT_SCREENSHOT_CAPTURE_HOTKEY,
    },
    error::{AppError, AppResult},
    services::HotkeyService,
};

use super::windows_hook_hotkey::classify_supported_shortcut;

const PREFERENCES_STORE_PATH: &str = "settings/preferences.json";
const PREFERENCES_STORE_KEY: &str = "appPreferences";

#[derive(Debug, Clone)]
pub struct PreferencesService {
    cache: Arc<RwLock<AppPreferences>>,
}

impl PreferencesService {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(AppPreferences::default())),
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
        input: AppPreferences,
    ) -> AppResult<AppPreferences> {
        let previous_preferences = self.get_preferences()?;
        let validated = validate_preferences(input)?;
        sync_autostart_launch_at_login(app, validated.startup.launch_at_login)?;
        hotkey_service.apply_preferences(app, &validated)?;
        let store = self.open_store(app)?;
        if let Err(error) = self.write_store(&store, &validated) {
            if let Err(rollback_error) =
                hotkey_service.apply_preferences(app, &previous_preferences)
            {
                log::error!(
                    target: "bexo::service::preferences",
                    "hotkey rollback failed after store write error: {}",
                    rollback_error
                );
            }
            return Err(error);
        }
        self.replace_cache(validated.clone())?;
        Ok(validated)
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
    let repaired_hotkeys = sanitize_hotkey_preferences(preferences.hotkey);
    preferences.hotkey = repaired_hotkeys.preferences;

    PreferenceRepairResult {
        preferences,
        changed: repaired_hotkeys.changed,
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

fn sync_autostart_launch_at_login<R: Runtime>(
    app: &AppHandle<R>,
    should_enable: bool,
) -> AppResult<()> {
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

fn validate_preferences(input: AppPreferences) -> AppResult<AppPreferences> {
    Ok(AppPreferences {
        terminal: crate::domain::TerminalPreferences {
            windows_terminal_path: validate_tool_path(
                "terminal.windowsTerminalPath",
                input.terminal.windows_terminal_path,
                &["wt.exe", "wt.cmd", "wt.bat"],
                "WINDOWS_TERMINAL_PATH_INVALID",
                "Windows Terminal",
            )?,
            codex_cli_path: validate_tool_path(
                "terminal.codexCliPath",
                input.terminal.codex_cli_path,
                &["codex.exe", "codex.cmd", "codex.bat"],
                "CODEX_PATH_INVALID",
                "Codex CLI",
            )?,
            command_templates: validate_command_templates(input.terminal.command_templates)?,
        },
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
    })
}

fn validate_startup_preferences(
    input: crate::domain::StartupPreferences,
) -> AppResult<crate::domain::StartupPreferences> {
    Ok(input)
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

    Ok(HotkeyPreferences {
        screenshot_capture,
        voice_input_toggle,
        voice_input_hold,
    })
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
        build_codex_home_directory_info, migrate_legacy_preferences, sanitize_hotkey_preferences,
        validate_hotkey_shortcut,
    };
    use crate::domain::{
        AppPreferences, HotkeyAction, DEFAULT_SCREENSHOT_CAPTURE_HOTKEY,
        EARLIER_DEFAULT_SCREENSHOT_CAPTURE_HOTKEY, LEGACY_SCREENSHOT_CAPTURE_HOTKEY,
        PREVIOUS_DEFAULT_SCREENSHOT_CAPTURE_HOTKEY,
    };

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
