use tauri::State;

use crate::{
    domain::{DeleteResult, PromptRecord, ReorderPromptsInput, UpsertPromptInput},
    error::{AppError, CommandResponse},
    services::PromptService,
};

#[tauri::command]
pub async fn list_prompts(
    prompt_service: State<'_, PromptService>,
) -> Result<CommandResponse<Vec<PromptRecord>>, AppError> {
    match prompt_service.list_prompts().await {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(target: "bexo::command::prompt", "list_prompts failed: {}", error);
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn upsert_prompt(
    prompt_service: State<'_, PromptService>,
    input: UpsertPromptInput,
) -> Result<CommandResponse<PromptRecord>, AppError> {
    match prompt_service.upsert_prompt(input).await {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(target: "bexo::command::prompt", "upsert_prompt failed: {}", error);
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn delete_prompt(
    prompt_service: State<'_, PromptService>,
    id: String,
) -> Result<CommandResponse<DeleteResult>, AppError> {
    match prompt_service.delete_prompt(id).await {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(target: "bexo::command::prompt", "delete_prompt failed: {}", error);
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn reorder_prompts(
    prompt_service: State<'_, PromptService>,
    input: ReorderPromptsInput,
) -> Result<CommandResponse<Vec<PromptRecord>>, AppError> {
    match prompt_service.reorder_prompts(input).await {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(target: "bexo::command::prompt", "reorder_prompts failed: {}", error);
            Ok(CommandResponse::failure(error))
        }
    }
}
