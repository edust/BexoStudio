use tauri::State;

use crate::{
    domain::{
        DeleteResult, OpenWorkspaceInEditorResult, OpenWorkspaceTerminalResult, ProjectRecord,
        RunWorkspaceTerminalCommandResult, RunWorkspaceTerminalCommandsResult, UpsertProjectInput,
        UpsertWorkspaceInput, WorkspaceRecord,
    },
    error::{AppError, CommandResponse},
    services::{PreferencesService, WorkspaceService},
};

#[tauri::command]
pub async fn list_workspaces(
    workspace_service: State<'_, WorkspaceService>,
) -> Result<CommandResponse<Vec<WorkspaceRecord>>, AppError> {
    match workspace_service.list_workspaces().await {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(target: "bexo::command::workspace", "list_workspaces failed: {}", error);
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command]
pub async fn upsert_workspace(
    workspace_service: State<'_, WorkspaceService>,
    input: UpsertWorkspaceInput,
) -> Result<CommandResponse<WorkspaceRecord>, AppError> {
    match workspace_service.upsert_workspace(input).await {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(target: "bexo::command::workspace", "upsert_workspace failed: {}", error);
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command]
pub async fn delete_workspace(
    workspace_service: State<'_, WorkspaceService>,
    id: String,
) -> Result<CommandResponse<DeleteResult>, AppError> {
    match workspace_service.delete_workspace(id).await {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(target: "bexo::command::workspace", "delete_workspace failed: {}", error);
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command]
pub async fn register_workspace_folder(
    workspace_service: State<'_, WorkspaceService>,
    path: String,
) -> Result<CommandResponse<WorkspaceRecord>, AppError> {
    match workspace_service.register_workspace_folder(path).await {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::workspace",
                "register_workspace_folder failed: {}",
                error
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command]
pub async fn remove_workspace_registration(
    workspace_service: State<'_, WorkspaceService>,
    id: String,
) -> Result<CommandResponse<DeleteResult>, AppError> {
    match workspace_service.remove_workspace_registration(id).await {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::workspace",
                "remove_workspace_registration failed: {}",
                error
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn open_workspace_terminal(
    workspace_service: State<'_, WorkspaceService>,
    preferences_service: State<'_, PreferencesService>,
    workspace_id: String,
) -> Result<CommandResponse<OpenWorkspaceTerminalResult>, AppError> {
    match workspace_service
        .open_workspace_terminal(workspace_id, &preferences_service)
        .await
    {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::workspace",
                "open_workspace_terminal failed: {}",
                error
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn open_workspace_terminal_at_path(
    workspace_service: State<'_, WorkspaceService>,
    preferences_service: State<'_, PreferencesService>,
    workspace_id: String,
    target_path: Option<String>,
) -> Result<CommandResponse<OpenWorkspaceTerminalResult>, AppError> {
    match workspace_service
        .open_workspace_terminal_at_path(workspace_id, target_path, &preferences_service)
        .await
    {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::workspace",
                "open_workspace_terminal_at_path failed: {}",
                error
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn open_workspace_in_editor(
    workspace_service: State<'_, WorkspaceService>,
    preferences_service: State<'_, PreferencesService>,
    workspace_id: String,
    editor_key: String,
) -> Result<CommandResponse<OpenWorkspaceInEditorResult>, AppError> {
    match workspace_service
        .open_workspace_in_editor(workspace_id, editor_key, &preferences_service)
        .await
    {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::workspace",
                "open_workspace_in_editor failed: {}",
                error
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn run_workspace_terminal_command(
    workspace_service: State<'_, WorkspaceService>,
    preferences_service: State<'_, PreferencesService>,
    workspace_id: String,
    launch_task_id: String,
) -> Result<CommandResponse<RunWorkspaceTerminalCommandResult>, AppError> {
    match workspace_service
        .run_workspace_terminal_command(workspace_id, launch_task_id, &preferences_service)
        .await
    {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::workspace",
                "run_workspace_terminal_command failed: {}",
                error
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn run_workspace_terminal_commands(
    workspace_service: State<'_, WorkspaceService>,
    preferences_service: State<'_, PreferencesService>,
    workspace_id: String,
) -> Result<CommandResponse<RunWorkspaceTerminalCommandsResult>, AppError> {
    match workspace_service
        .run_workspace_terminal_commands(workspace_id, &preferences_service)
        .await
    {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::workspace",
                "run_workspace_terminal_commands failed: {}",
                error
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command]
pub async fn upsert_project(
    workspace_service: State<'_, WorkspaceService>,
    input: UpsertProjectInput,
) -> Result<CommandResponse<ProjectRecord>, AppError> {
    match workspace_service.upsert_project(input).await {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(target: "bexo::command::workspace", "upsert_project failed: {}", error);
            Ok(CommandResponse::failure(error))
        }
    }
}
