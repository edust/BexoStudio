use tauri::State;

use crate::{
    domain::{DeleteResult, LaunchTaskRecord, UpsertLaunchTaskInput},
    error::{AppError, CommandResponse},
    services::WorkspaceService,
};

#[tauri::command(rename_all = "camelCase")]
pub async fn list_launch_tasks(
    workspace_service: State<'_, WorkspaceService>,
    project_id: String,
) -> Result<CommandResponse<Vec<LaunchTaskRecord>>, AppError> {
    match workspace_service.list_launch_tasks(project_id).await {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(target: "bexo::command::launch_task", "list_launch_tasks failed: {}", error);
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn upsert_launch_task(
    workspace_service: State<'_, WorkspaceService>,
    input: UpsertLaunchTaskInput,
) -> Result<CommandResponse<LaunchTaskRecord>, AppError> {
    match workspace_service.upsert_launch_task(input).await {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(target: "bexo::command::launch_task", "upsert_launch_task failed: {}", error);
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn delete_launch_task(
    workspace_service: State<'_, WorkspaceService>,
    id: String,
) -> Result<CommandResponse<DeleteResult>, AppError> {
    match workspace_service.delete_launch_task(id).await {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(target: "bexo::command::launch_task", "delete_launch_task failed: {}", error);
            Ok(CommandResponse::failure(error))
        }
    }
}
