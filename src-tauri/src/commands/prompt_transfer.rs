use tauri::State;

use crate::{
    domain::{
        ApplyPromptListImportInput, ExportPromptListInput, ExportPromptListResult,
        PreviewPromptListImportInput, PromptImportApplyResult, PromptImportPreview,
    },
    error::{AppError, CommandResponse},
    services::PromptTransferService,
};

#[tauri::command(rename_all = "camelCase")]
pub async fn export_prompt_list(
    prompt_transfer_service: State<'_, PromptTransferService>,
    input: ExportPromptListInput,
) -> Result<CommandResponse<ExportPromptListResult>, AppError> {
    match prompt_transfer_service.export_prompt_list(input).await {
        Ok(data) => {
            log::info!(
                target: "bexo::command::prompt_transfer",
                "export_prompt_list completed prompt_count={} file_size_bytes={} file_hash={}",
                data.prompt_count,
                data.file_size_bytes,
                data.file_hash
            );
            Ok(CommandResponse::success(data))
        }
        Err(error) => {
            log::error!(
                target: "bexo::command::prompt_transfer",
                "export_prompt_list failed code={} message={}",
                error.code,
                error.message
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn preview_prompt_list_import(
    prompt_transfer_service: State<'_, PromptTransferService>,
    input: PreviewPromptListImportInput,
) -> Result<CommandResponse<PromptImportPreview>, AppError> {
    match prompt_transfer_service
        .preview_prompt_list_import(input)
        .await
    {
        Ok(data) => {
            log::info!(
                target: "bexo::command::prompt_transfer",
                "preview_prompt_list_import completed total={} ready={} existing={} conflicts={} duplicates={} file_hash={}",
                data.summary.total_prompt_count,
                data.summary.ready_prompt_count,
                data.summary.existing_prompt_count,
                data.summary.conflict_prompt_count,
                data.summary.duplicate_prompt_count,
                data.file_hash
            );
            Ok(CommandResponse::success(data))
        }
        Err(error) => {
            log::error!(
                target: "bexo::command::prompt_transfer",
                "preview_prompt_list_import failed code={} message={}",
                error.code,
                error.message
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn apply_prompt_list_import(
    prompt_transfer_service: State<'_, PromptTransferService>,
    input: ApplyPromptListImportInput,
) -> Result<CommandResponse<PromptImportApplyResult>, AppError> {
    match prompt_transfer_service
        .apply_prompt_list_import(input)
        .await
    {
        Ok(data) => {
            log::info!(
                target: "bexo::command::prompt_transfer",
                "apply_prompt_list_import completed created={} updated={} skipped={} final_count={}",
                data.created_prompt_count,
                data.updated_prompt_count,
                data.skipped_prompt_count,
                data.prompts.len()
            );
            Ok(CommandResponse::success(data))
        }
        Err(error) => {
            log::error!(
                target: "bexo::command::prompt_transfer",
                "apply_prompt_list_import failed code={} message={}",
                error.code,
                error.message
            );
            Ok(CommandResponse::failure(error))
        }
    }
}
