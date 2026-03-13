use tauri::State;

use crate::{
    domain::{AppPreferences, CodexHomeDirectoryInfo, EditorPathDetectionResult},
    error::{AppError, CommandResponse},
    services::{HotkeyService, PreferencesService},
};

#[tauri::command(rename_all = "camelCase")]
pub async fn get_app_preferences(
    preferences_service: State<'_, PreferencesService>,
) -> Result<CommandResponse<AppPreferences>, AppError> {
    match preferences_service.get_preferences() {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::preferences",
                "get_app_preferences failed: {}",
                error
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn update_app_preferences(
    app_handle: tauri::AppHandle,
    preferences_service: State<'_, PreferencesService>,
    hotkey_service: State<'_, HotkeyService>,
    input: AppPreferences,
) -> Result<CommandResponse<AppPreferences>, AppError> {
    match preferences_service.update_preferences(&app_handle, hotkey_service.inner(), input) {
        Ok(data) => {
            if let Err(error) = crate::app::refresh_tray_menu(&app_handle).await {
                log::error!(
                    target: "bexo::command::preferences",
                    "refresh_tray_menu after preferences update failed: {}",
                    error
                );
            }
            Ok(CommandResponse::success(data))
        }
        Err(error) => {
            log::error!(
                target: "bexo::command::preferences",
                "update_app_preferences failed: {}",
                error
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_codex_home_directory(
    app_handle: tauri::AppHandle,
    preferences_service: State<'_, PreferencesService>,
) -> Result<CommandResponse<CodexHomeDirectoryInfo>, AppError> {
    match preferences_service.get_codex_home_directory(&app_handle) {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::preferences",
                "get_codex_home_directory failed: {}",
                error
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn detect_editors_from_path(
    preferences_service: State<'_, PreferencesService>,
) -> Result<CommandResponse<EditorPathDetectionResult>, AppError> {
    match preferences_service.detect_editors_from_path() {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::preferences",
                "detect_editors_from_path failed: {}",
                error
            );
            Ok(CommandResponse::failure(error))
        }
    }
}
