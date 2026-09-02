use tauri::State;

use crate::{
    domain::{
        CodexHistoryGlobalSessionsPage, CodexHistoryMessagesInput, CodexHistoryMessagesPage,
        CodexHistorySessionsResponse, ListCodexHistorySessionsInput,
        ListCodexHistorySessionsPageInput, OpenCodexHistoryWindowResult,
    },
    error::{AppError, CommandResponse},
    services::CodexHistoryService,
};

#[tauri::command(rename_all = "camelCase")]
pub async fn open_codex_history_window(
    app_handle: tauri::AppHandle,
    codex_history_service: State<'_, CodexHistoryService>,
    workspace_id: String,
) -> Result<CommandResponse<OpenCodexHistoryWindowResult>, AppError> {
    match codex_history_service
        .open_history_window(&app_handle, workspace_id)
        .await
    {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::codex_history",
                "open_codex_history_window failed code={} message={} details={:?}",
                error.code.as_str(),
                error.message.as_str(),
                error.details.as_ref()
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn list_codex_history_sessions(
    app_handle: tauri::AppHandle,
    codex_history_service: State<'_, CodexHistoryService>,
    input: ListCodexHistorySessionsInput,
) -> Result<CommandResponse<CodexHistorySessionsResponse>, AppError> {
    match codex_history_service
        .list_sessions(&app_handle, input)
        .await
    {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::codex_history",
                "list_codex_history_sessions failed code={} message={} details={:?}",
                error.code.as_str(),
                error.message.as_str(),
                error.details.as_ref()
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn list_all_codex_history_sessions(
    app_handle: tauri::AppHandle,
    codex_history_service: State<'_, CodexHistoryService>,
    input: ListCodexHistorySessionsPageInput,
) -> Result<CommandResponse<CodexHistoryGlobalSessionsPage>, AppError> {
    match codex_history_service
        .list_all_sessions_page(&app_handle, input)
        .await
    {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::codex_history",
                "list_all_codex_history_sessions failed code={} message={} details={:?}",
                error.code.as_str(),
                error.message.as_str(),
                error.details.as_ref()
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_codex_history_messages(
    app_handle: tauri::AppHandle,
    codex_history_service: State<'_, CodexHistoryService>,
    input: CodexHistoryMessagesInput,
) -> Result<CommandResponse<CodexHistoryMessagesPage>, AppError> {
    match codex_history_service.get_messages(&app_handle, input).await {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::codex_history",
                "get_codex_history_messages failed code={} message={} details={:?}",
                error.code.as_str(),
                error.message.as_str(),
                error.details.as_ref()
            );
            Ok(CommandResponse::failure(error))
        }
    }
}
