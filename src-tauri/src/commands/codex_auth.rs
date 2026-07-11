use tauri::State;

use crate::{
    domain::{
        CodexAuthProfileDetail, CodexAuthProfileSummary, CodexAuthQuotaRefreshBatchResult,
        CodexAuthSwitchResult, DeleteResult, UpsertCodexAuthProfileInput,
    },
    error::{AppError, CommandResponse},
    services::{CodexAuthService, PreferencesService},
};

#[tauri::command(rename_all = "camelCase")]
pub async fn list_codex_auth_profiles(
    codex_auth_service: State<'_, CodexAuthService>,
) -> Result<CommandResponse<Vec<CodexAuthProfileSummary>>, AppError> {
    match codex_auth_service.list_profiles().await {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::codex_auth",
                "list_codex_auth_profiles failed: {}",
                error
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_codex_auth_profile_detail(
    codex_auth_service: State<'_, CodexAuthService>,
    id: String,
) -> Result<CommandResponse<CodexAuthProfileDetail>, AppError> {
    match codex_auth_service.get_profile_detail(id).await {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::codex_auth",
                "get_codex_auth_profile_detail failed code={} message={}",
                error.code.as_str(),
                error.message.as_str()
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn import_current_codex_auth_profile(
    app_handle: tauri::AppHandle,
    codex_auth_service: State<'_, CodexAuthService>,
    preferences_service: State<'_, PreferencesService>,
) -> Result<CommandResponse<CodexAuthProfileSummary>, AppError> {
    match codex_auth_service
        .import_current_profile(&app_handle, preferences_service.inner())
        .await
    {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::codex_auth",
                "import_current_codex_auth_profile failed code={} message={}",
                error.code.as_str(),
                error.message.as_str()
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn upsert_codex_auth_profile(
    codex_auth_service: State<'_, CodexAuthService>,
    input: UpsertCodexAuthProfileInput,
) -> Result<CommandResponse<CodexAuthProfileSummary>, AppError> {
    match codex_auth_service.upsert_profile(input).await {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::codex_auth",
                "upsert_codex_auth_profile failed code={} message={}",
                error.code.as_str(),
                error.message.as_str()
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn delete_codex_auth_profile(
    codex_auth_service: State<'_, CodexAuthService>,
    id: String,
) -> Result<CommandResponse<DeleteResult>, AppError> {
    match codex_auth_service.delete_profile(id).await {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::codex_auth",
                "delete_codex_auth_profile failed code={} message={}",
                error.code.as_str(),
                error.message.as_str()
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn switch_codex_auth_profile(
    codex_auth_service: State<'_, CodexAuthService>,
    id: String,
) -> Result<CommandResponse<CodexAuthSwitchResult>, AppError> {
    match codex_auth_service.switch_profile(id).await {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::codex_auth",
                "switch_codex_auth_profile failed code={} message={}",
                error.code.as_str(),
                error.message.as_str()
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn query_codex_auth_quota(
    codex_auth_service: State<'_, CodexAuthService>,
    id: String,
) -> Result<CommandResponse<CodexAuthProfileSummary>, AppError> {
    match codex_auth_service.query_quota(id).await {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::codex_auth",
                "query_codex_auth_quota failed code={} message={}",
                error.code.as_str(),
                error.message.as_str()
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn refresh_all_codex_auth_quotas(
    codex_auth_service: State<'_, CodexAuthService>,
) -> Result<CommandResponse<CodexAuthQuotaRefreshBatchResult>, AppError> {
    match codex_auth_service.refresh_all_quotas().await {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::codex_auth",
                "refresh_all_codex_auth_quotas failed code={} message={}",
                error.code.as_str(),
                error.message.as_str()
            );
            Ok(CommandResponse::failure(error))
        }
    }
}
