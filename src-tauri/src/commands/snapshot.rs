use tauri::State;

use crate::{
    domain::{CreateSnapshotInput, SnapshotRecord, UpdateSnapshotInput},
    error::{AppError, CommandResponse},
    services::PlannerService,
};

#[tauri::command(rename_all = "camelCase")]
pub async fn list_snapshots(
    planner_service: State<'_, PlannerService>,
    workspace_id: Option<String>,
) -> Result<CommandResponse<Vec<SnapshotRecord>>, AppError> {
    match planner_service.list_snapshots(workspace_id).await {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(target: "bexo::command::snapshot", "list_snapshots failed: {}", error);
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn create_snapshot(
    app_handle: tauri::AppHandle,
    planner_service: State<'_, PlannerService>,
    input: CreateSnapshotInput,
) -> Result<CommandResponse<SnapshotRecord>, AppError> {
    match planner_service.create_snapshot(input).await {
        Ok(data) => {
            if let Err(error) = crate::app::refresh_tray_menu(&app_handle).await {
                log::error!(
                    target: "bexo::command::snapshot",
                    "refresh_tray_menu after create_snapshot failed: {}",
                    error
                );
            }
            Ok(CommandResponse::success(data))
        }
        Err(error) => {
            log::error!(target: "bexo::command::snapshot", "create_snapshot failed: {}", error);
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn update_snapshot(
    app_handle: tauri::AppHandle,
    planner_service: State<'_, PlannerService>,
    input: UpdateSnapshotInput,
) -> Result<CommandResponse<SnapshotRecord>, AppError> {
    match planner_service.update_snapshot(input).await {
        Ok(data) => {
            if let Err(error) = crate::app::refresh_tray_menu(&app_handle).await {
                log::error!(
                    target: "bexo::command::snapshot",
                    "refresh_tray_menu after update_snapshot failed: {}",
                    error
                );
            }
            Ok(CommandResponse::success(data))
        }
        Err(error) => {
            log::error!(target: "bexo::command::snapshot", "update_snapshot failed: {}", error);
            Ok(CommandResponse::failure(error))
        }
    }
}
