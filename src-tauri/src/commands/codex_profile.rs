use tauri::State;

use crate::{
    domain::{CodexProfileRecord, UpsertCodexProfileInput},
    error::{AppError, CommandResponse},
    services::ProfileService,
};

#[tauri::command]
pub async fn list_codex_profiles(
    profile_service: State<'_, ProfileService>,
) -> Result<CommandResponse<Vec<CodexProfileRecord>>, AppError> {
    match profile_service.list_codex_profiles().await {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(target: "bexo::command::profile", "list_codex_profiles failed: {}", error);
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command]
pub async fn upsert_codex_profile(
    profile_service: State<'_, ProfileService>,
    input: UpsertCodexProfileInput,
) -> Result<CommandResponse<CodexProfileRecord>, AppError> {
    match profile_service.upsert_codex_profile(input).await {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(target: "bexo::command::profile", "upsert_codex_profile failed: {}", error);
            Ok(CommandResponse::failure(error))
        }
    }
}
