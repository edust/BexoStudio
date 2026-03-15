use std::time::Instant;

use tauri::{ipc::Response, State};

use crate::{
    domain::{
        CancelScreenshotSessionResult, CopyScreenshotSelectionResult,
        SaveScreenshotSelectionResult, ScreenshotRenderedImageInput, ScreenshotSelectionInput,
        ScreenshotSelectionRenderView, ScreenshotSessionView, StartScreenshotSessionResult,
    },
    error::{AppError, CommandResponse},
    services::ScreenshotService,
};

#[tauri::command]
pub async fn start_screenshot_session(
    app_handle: tauri::AppHandle,
    screenshot_service: State<'_, ScreenshotService>,
) -> Result<CommandResponse<StartScreenshotSessionResult>, AppError> {
    let started_at = Instant::now();
    match screenshot_service.start_session(&app_handle) {
        Ok(data) => {
            log::info!(
                target: "bexo::command::screenshot",
                "start_screenshot_session completed session_id={} total_ms={}",
                data.session_id,
                started_at.elapsed().as_millis()
            );
            Ok(CommandResponse::success(data))
        }
        Err(error) => {
            log::error!(
                target: "bexo::command::screenshot",
                "start_screenshot_session failed total_ms={} reason={}",
                started_at.elapsed().as_millis(),
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
    let started_at = Instant::now();
    match screenshot_service.get_active_session() {
        Ok(data) => {
            log::info!(
                target: "bexo::command::screenshot",
                "get_screenshot_session completed session_id={} image_status={:?} image_data_url_bytes={} preview_image_path={} preview_transport={:?} preview_pixels={}x{} monitors={} total_ms={}",
                data.session_id,
                data.image_status,
                data.image_data_url.len(),
                data.preview_image_path.as_deref().unwrap_or(""),
                data.preview_transport,
                data.preview_pixel_width,
                data.preview_pixel_height,
                data.monitors.len(),
                started_at.elapsed().as_millis()
            );
            Ok(CommandResponse::success(data))
        }
        Err(error) => {
            log::error!(
                target: "bexo::command::screenshot",
                "get_screenshot_session failed total_ms={} reason={}",
                started_at.elapsed().as_millis(),
                error
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_screenshot_preview_rgba(
    screenshot_service: State<'_, ScreenshotService>,
    session_id: String,
) -> Result<Response, AppError> {
    let started_at = Instant::now();
    match screenshot_service.get_preview_rgba(&session_id) {
        Ok(bytes) => {
            log::info!(
                target: "bexo::command::screenshot",
                "get_screenshot_preview_rgba completed session_id={} bytes={} total_ms={}",
                session_id,
                bytes.len(),
                started_at.elapsed().as_millis()
            );
            Ok(Response::new(bytes))
        }
        Err(error) => {
            log::error!(
                target: "bexo::command::screenshot",
                "get_screenshot_preview_rgba failed session_id={} total_ms={} reason={}",
                session_id,
                started_at.elapsed().as_millis(),
                error
            );
            Err(error)
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_screenshot_selection_render(
    screenshot_service: State<'_, ScreenshotService>,
    session_id: String,
    selection: ScreenshotSelectionInput,
) -> Result<CommandResponse<ScreenshotSelectionRenderView>, AppError> {
    let started_at = Instant::now();
    match screenshot_service.get_selection_render(&session_id, selection) {
        Ok(data) => {
            log::info!(
                target: "bexo::command::screenshot",
                "get_screenshot_selection_render completed session_id={} mode={:?} width={} height={} image_data_url_bytes={} total_ms={}",
                data.session_id,
                data.render_mode,
                data.width,
                data.height,
                data.image_data_url.len(),
                started_at.elapsed().as_millis()
            );
            Ok(CommandResponse::success(data))
        }
        Err(error) => {
            log::error!(
                target: "bexo::command::screenshot",
                "get_screenshot_selection_render failed session_id={} total_ms={} reason={}",
                session_id,
                started_at.elapsed().as_millis(),
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
            if let Err(error) = screenshot_service.restore_overlay_hot_state(&app_handle) {
                log::warn!(
                    target: "bexo::command::screenshot",
                    "hide overlay window after copy failed: {}",
                    error
                );
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
            if let Err(error) = screenshot_service.restore_overlay_hot_state(&app_handle) {
                log::warn!(
                    target: "bexo::command::screenshot",
                    "hide overlay window after save failed: {}",
                    error
                );
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
            if let Err(error) = screenshot_service.restore_overlay_hot_state(&app_handle) {
                log::warn!(
                    target: "bexo::command::screenshot",
                    "hide overlay window after cancel failed: {}",
                    error
                );
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
