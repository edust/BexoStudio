use tauri::State;

use crate::{
    domain::{
        ApplyWorkspaceListImportInput, ExportWorkspaceListInput, ExportWorkspaceListResult,
        PreviewWorkspaceListImportInput, WorkspaceImportApplyResult, WorkspaceImportPreview,
    },
    error::{AppError, CommandResponse},
    services::{
        HotkeyService, PreferencesService, WorkspaceImportApplyOutcome, WorkspaceTransferService,
    },
};

#[tauri::command(rename_all = "camelCase")]
pub async fn export_workspace_list(
    workspace_transfer_service: State<'_, WorkspaceTransferService>,
    preferences_service: State<'_, PreferencesService>,
    input: ExportWorkspaceListInput,
) -> Result<CommandResponse<ExportWorkspaceListResult>, AppError> {
    let result = preferences_service
        .get_preferences()
        .map(|preferences| preferences.workspace.pinned_workspace_ids);
    let result = match result {
        Ok(pinned_workspace_ids) => {
            workspace_transfer_service
                .export_workspace_list(input, pinned_workspace_ids)
                .await
        }
        Err(error) => Err(error),
    };
    match result {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::workspace_transfer",
                "export_workspace_list failed code={} message={}",
                error.code,
                error.message
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn preview_workspace_list_import(
    workspace_transfer_service: State<'_, WorkspaceTransferService>,
    preferences_service: State<'_, PreferencesService>,
    input: PreviewWorkspaceListImportInput,
) -> Result<CommandResponse<WorkspaceImportPreview>, AppError> {
    let result = match current_custom_editor_ids(preferences_service.inner()) {
        Ok(custom_editor_ids) => {
            workspace_transfer_service
                .preview_workspace_list_import(input, custom_editor_ids)
                .await
        }
        Err(error) => Err(error),
    };
    match result {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::workspace_transfer",
                "preview_workspace_list_import failed code={} message={}",
                error.code,
                error.message
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn apply_workspace_list_import(
    app_handle: tauri::AppHandle,
    workspace_transfer_service: State<'_, WorkspaceTransferService>,
    preferences_service: State<'_, PreferencesService>,
    hotkey_service: State<'_, HotkeyService>,
    input: ApplyWorkspaceListImportInput,
) -> Result<CommandResponse<WorkspaceImportApplyResult>, AppError> {
    let result = match current_custom_editor_ids(preferences_service.inner()) {
        Ok(custom_editor_ids) => {
            workspace_transfer_service
                .apply_workspace_list_import(input, custom_editor_ids)
                .await
        }
        Err(error) => Err(error),
    };
    match result {
        Ok(outcome) => {
            let mut result = outcome.result.clone();
            if let Err(error) = apply_imported_pinned_preferences(
                &app_handle,
                preferences_service.inner(),
                hotkey_service.inner(),
                &outcome,
            ) {
                log::error!(
                    target: "bexo::command::workspace_transfer",
                    "workspace import committed but pinned preferences update failed code={} message={}",
                    error.code,
                    error.message
                );
                result.warnings.push(format!(
                    "工作区已导入，但置顶状态保存失败（{}）：{}。可在列表中手动重新置顶。",
                    error.code, error.message
                ));
            }
            if let Err(error) = crate::app::refresh_tray_menu(&app_handle).await {
                log::error!(
                    target: "bexo::command::workspace_transfer",
                    "refresh_tray_menu after workspace import failed code={} message={}",
                    error.code,
                    error.message
                );
                result.warnings.push(
                    "工作区已导入，但托盘最近工作区刷新失败；重启 Bexo Studio 后会恢复。"
                        .to_string(),
                );
            }
            Ok(CommandResponse::success(result))
        }
        Err(error) => {
            log::error!(
                target: "bexo::command::workspace_transfer",
                "apply_workspace_list_import failed code={} message={}",
                error.code,
                error.message
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

fn apply_imported_pinned_preferences(
    app_handle: &tauri::AppHandle,
    preferences_service: &PreferencesService,
    hotkey_service: &HotkeyService,
    outcome: &WorkspaceImportApplyOutcome,
) -> Result<(), AppError> {
    if outcome.pinned_workspace_ids_to_add.is_empty()
        && outcome.pinned_workspace_ids_to_remove.is_empty()
    {
        return Ok(());
    }
    preferences_service.merge_workspace_pinned_ids(
        app_handle,
        hotkey_service,
        &outcome.pinned_workspace_ids_to_add,
        &outcome.pinned_workspace_ids_to_remove,
    )?;
    Ok(())
}

fn current_custom_editor_ids(
    preferences_service: &PreferencesService,
) -> Result<Vec<String>, AppError> {
    Ok(preferences_service
        .get_preferences()?
        .ide
        .custom_editors
        .into_iter()
        .map(|editor| editor.id)
        .collect())
}
