use tauri::{AppHandle, State};
use tauri_plugin_fs::FsExt;

use crate::{
    domain::{WorkspaceResourceEntry, WorkspaceResourceGitStatusResponse},
    error::{AppError, CommandResponse},
    services::ResourceBrowserService,
};

#[tauri::command(rename_all = "camelCase")]
pub async fn list_workspace_resource_children(
    resource_browser_service: State<'_, ResourceBrowserService>,
    workspace_id: String,
    target_path: Option<String>,
) -> Result<CommandResponse<Vec<WorkspaceResourceEntry>>, AppError> {
    match resource_browser_service
        .list_workspace_resource_children(workspace_id, target_path)
        .await
    {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::resource_browser",
                "list_workspace_resource_children failed: {}",
                error
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn allow_workspace_resource_scope(
    app: AppHandle,
    resource_browser_service: State<'_, ResourceBrowserService>,
    workspace_id: String,
) -> Result<CommandResponse<String>, AppError> {
    match resource_browser_service
        .resolve_workspace_root_path(workspace_id)
        .await
    {
        Ok(root_path) => {
            let fs_scope = app.fs_scope();
            match fs_scope.allow_directory(&root_path, true) {
                Ok(()) => Ok(CommandResponse::success(root_path)),
                Err(error) => {
                    let app_error = AppError::new(
                        "FS_SCOPE_ALLOW_FAILED",
                        "failed to allow workspace directory for filesystem watch",
                    )
                    .with_detail("path", root_path)
                    .with_detail("reason", error.to_string());
                    log::error!(
                        target: "bexo::command::resource_browser",
                        "allow_workspace_resource_scope failed: {}",
                        app_error
                    );
                    Ok(CommandResponse::failure(app_error))
                }
            }
        }
        Err(error) => {
            log::error!(
                target: "bexo::command::resource_browser",
                "allow_workspace_resource_scope failed: {}",
                error
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_workspace_resource_git_statuses(
    resource_browser_service: State<'_, ResourceBrowserService>,
    workspace_id: String,
) -> Result<CommandResponse<WorkspaceResourceGitStatusResponse>, AppError> {
    match resource_browser_service
        .get_workspace_resource_git_statuses(workspace_id)
        .await
    {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::resource_browser",
                "get_workspace_resource_git_statuses failed: {}",
                error
            );
            Ok(CommandResponse::failure(error))
        }
    }
}
