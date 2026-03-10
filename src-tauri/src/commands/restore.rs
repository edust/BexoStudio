use tauri::State;

use crate::{
    domain::{
        CancelRestoreActionResult, CancelRestoreRunResult, OpenLogDirectoryResult,
        RecentRestoreTarget, RestoreCapabilities, RestorePreview, RestorePreviewInput,
        RestoreRunDetail, RestoreRunSummary, StartRestoreDryRunInput, StartRestoreRunInput,
    },
    error::{AppError, CommandResponse},
    services::{PlannerService, PreferencesService, RestoreService},
};

#[tauri::command(rename_all = "camelCase")]
pub async fn preview_restore(
    planner_service: State<'_, PlannerService>,
    input: RestorePreviewInput,
) -> Result<CommandResponse<RestorePreview>, AppError> {
    match planner_service.preview_restore(input).await {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(target: "bexo::command::restore", "preview_restore failed: {}", error);
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn start_restore_dry_run(
    app_handle: tauri::AppHandle,
    planner_service: State<'_, PlannerService>,
    input: StartRestoreDryRunInput,
) -> Result<CommandResponse<RestoreRunDetail>, AppError> {
    match planner_service.start_restore_dry_run(input).await {
        Ok(data) => {
            if let Err(error) = crate::app::refresh_tray_menu(&app_handle).await {
                log::error!(
                    target: "bexo::command::restore",
                    "refresh_tray_menu after start_restore_dry_run failed: {}",
                    error
                );
            }
            Ok(CommandResponse::success(data))
        }
        Err(error) => {
            log::error!(target: "bexo::command::restore", "start_restore_dry_run failed: {}", error);
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_restore_capabilities(
    preferences_service: State<'_, PreferencesService>,
    restore_service: State<'_, RestoreService>,
) -> Result<CommandResponse<RestoreCapabilities>, AppError> {
    match restore_service
        .get_restore_capabilities(&preferences_service)
        .await
    {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(target: "bexo::command::restore", "get_restore_capabilities failed: {}", error);
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn start_restore_run(
    app_handle: tauri::AppHandle,
    preferences_service: State<'_, PreferencesService>,
    restore_service: State<'_, RestoreService>,
    input: StartRestoreRunInput,
) -> Result<CommandResponse<RestoreRunDetail>, AppError> {
    match restore_service
        .start_restore_run_with_events(Some(&app_handle), input, &preferences_service)
        .await
    {
        Ok(data) => {
            if let Err(error) = crate::app::refresh_tray_menu(&app_handle).await {
                log::error!(
                    target: "bexo::command::restore",
                    "refresh_tray_menu after start_restore_run failed: {}",
                    error
                );
            }
            Ok(CommandResponse::success(data))
        }
        Err(error) => {
            log::error!(target: "bexo::command::restore", "start_restore_run failed: {}", error);
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn cancel_restore_run(
    app_handle: tauri::AppHandle,
    restore_service: State<'_, RestoreService>,
    run_id: String,
) -> Result<CommandResponse<CancelRestoreRunResult>, AppError> {
    match restore_service
        .cancel_restore_run(Some(&app_handle), run_id)
        .await
    {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(target: "bexo::command::restore", "cancel_restore_run failed: {}", error);
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn cancel_restore_action(
    app_handle: tauri::AppHandle,
    restore_service: State<'_, RestoreService>,
    run_id: String,
    project_task_id: String,
    action_id: String,
) -> Result<CommandResponse<CancelRestoreActionResult>, AppError> {
    match restore_service
        .cancel_restore_action(Some(&app_handle), run_id, project_task_id, action_id)
        .await
    {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(target: "bexo::command::restore", "cancel_restore_action failed: {}", error);
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn list_restore_runs(
    restore_service: State<'_, RestoreService>,
) -> Result<CommandResponse<Vec<RestoreRunSummary>>, AppError> {
    match restore_service.list_restore_runs().await {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(target: "bexo::command::restore", "list_restore_runs failed: {}", error);
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn list_recent_restore_targets(
    restore_service: State<'_, RestoreService>,
) -> Result<CommandResponse<Vec<RecentRestoreTarget>>, AppError> {
    match restore_service.list_recent_restore_targets().await {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::restore",
                "list_recent_restore_targets failed: {}",
                error
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn restore_recent_target(
    app_handle: tauri::AppHandle,
    preferences_service: State<'_, PreferencesService>,
    restore_service: State<'_, RestoreService>,
    id: String,
    mode: Option<String>,
) -> Result<CommandResponse<RestoreRunDetail>, AppError> {
    match restore_service
        .restore_recent_target_with_events(Some(&app_handle), id, mode, &preferences_service)
        .await
    {
        Ok(data) => {
            if let Err(error) = crate::app::refresh_tray_menu(&app_handle).await {
                log::error!(
                    target: "bexo::command::restore",
                    "refresh_tray_menu after restore_recent_target failed: {}",
                    error
                );
            }
            Ok(CommandResponse::success(data))
        }
        Err(error) => {
            log::error!(target: "bexo::command::restore", "restore_recent_target failed: {}", error);
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_restore_run_detail(
    restore_service: State<'_, RestoreService>,
    id: String,
) -> Result<CommandResponse<RestoreRunDetail>, AppError> {
    match restore_service.get_restore_run_detail(id).await {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(target: "bexo::command::restore", "get_restore_run_detail failed: {}", error);
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn open_log_directory(
    restore_service: State<'_, RestoreService>,
) -> Result<CommandResponse<OpenLogDirectoryResult>, AppError> {
    match restore_service.open_log_directory().await {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(target: "bexo::command::restore", "open_log_directory failed: {}", error);
            Ok(CommandResponse::failure(error))
        }
    }
}
