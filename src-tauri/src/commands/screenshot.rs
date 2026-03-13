use tauri::{Manager, State};

use crate::{
    domain::{
        CancelScreenshotSessionResult, CopyScreenshotSelectionResult,
        SaveScreenshotSelectionResult, ScreenshotRenderedImageInput, ScreenshotSelectionInput,
        ScreenshotSessionView, StartScreenshotSessionResult,
    },
    error::{AppError, CommandResponse},
    services::ScreenshotService,
};

#[tauri::command]
pub async fn start_screenshot_session(
    app_handle: tauri::AppHandle,
    screenshot_service: State<'_, ScreenshotService>,
) -> Result<CommandResponse<StartScreenshotSessionResult>, AppError> {
    match screenshot_service.start_session(&app_handle) {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::screenshot",
                "start_screenshot_session failed: {}",
                error
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command]
pub async fn get_screenshot_session(
    screenshot_service: State<'_, ScreenshotService>,
) -> Result<CommandResponse<ScreenshotSessionView>, AppError> {
    match screenshot_service.get_active_session() {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::screenshot",
                "get_screenshot_session failed: {}",
                error
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn copy_screenshot_selection(
    app_handle: tauri::AppHandle,
    screenshot_service: State<'_, ScreenshotService>,
    session_id: String,
    selection: ScreenshotSelectionInput,
    rendered_image: Option<ScreenshotRenderedImageInput>,
) -> Result<CommandResponse<CopyScreenshotSelectionResult>, AppError> {
    match screenshot_service.copy_selection(&session_id, selection, rendered_image) {
        Ok(data) => {
            if let Some(window) =
                app_handle.get_webview_window(crate::domain::SCREENSHOT_OVERLAY_WINDOW_LABEL)
            {
                let _ = window.hide();
            }
            Ok(CommandResponse::success(data))
        }
        Err(error) => {
            log::error!(
                target: "bexo::command::screenshot",
                "copy_screenshot_selection failed: {}",
                error
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn save_screenshot_selection(
    app_handle: tauri::AppHandle,
    screenshot_service: State<'_, ScreenshotService>,
    session_id: String,
    selection: ScreenshotSelectionInput,
    file_path: Option<String>,
    rendered_image: Option<ScreenshotRenderedImageInput>,
) -> Result<CommandResponse<SaveScreenshotSelectionResult>, AppError> {
    match screenshot_service.save_selection(
        &app_handle,
        &session_id,
        selection,
        file_path,
        rendered_image,
    ) {
        Ok(data) => {
            if let Some(window) =
                app_handle.get_webview_window(crate::domain::SCREENSHOT_OVERLAY_WINDOW_LABEL)
            {
                let _ = window.hide();
            }
            Ok(CommandResponse::success(data))
        }
        Err(error) => {
            log::error!(
                target: "bexo::command::screenshot",
                "save_screenshot_selection failed: {}",
                error
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn cancel_screenshot_session(
    app_handle: tauri::AppHandle,
    screenshot_service: State<'_, ScreenshotService>,
    session_id: String,
) -> Result<CommandResponse<CancelScreenshotSessionResult>, AppError> {
    match screenshot_service.cancel_session(&session_id) {
        Ok(data) => {
            if let Some(window) =
                app_handle.get_webview_window(crate::domain::SCREENSHOT_OVERLAY_WINDOW_LABEL)
            {
                let _ = window.hide();
            }
            Ok(CommandResponse::success(data))
        }
        Err(error) => {
            log::error!(
                target: "bexo::command::screenshot",
                "cancel_screenshot_session failed: {}",
                error
            );
            Ok(CommandResponse::failure(error))
        }
    }
}
