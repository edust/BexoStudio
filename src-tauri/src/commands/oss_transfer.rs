use tauri::{AppHandle, State};

use crate::{
    domain::{
        OssOperationInput, OssTransferStart, OssTransferTaskView, StartOssDownloadInput,
        StartOssUploadInput,
    },
    error::{AppError, CommandResponse},
    services::OssTransferService,
};

#[tauri::command]
pub async fn list_oss_transfer_tasks(
    oss_transfer_service: State<'_, OssTransferService>,
) -> Result<CommandResponse<Vec<OssTransferTaskView>>, AppError> {
    respond(
        "list_oss_transfer_tasks",
        oss_transfer_service.list_tasks().await,
    )
}

#[tauri::command]
pub async fn start_oss_upload(
    oss_transfer_service: State<'_, OssTransferService>,
    app_handle: AppHandle,
    input: StartOssUploadInput,
) -> Result<CommandResponse<OssTransferStart>, AppError> {
    respond(
        "start_oss_upload",
        oss_transfer_service.start_upload(input, app_handle).await,
    )
}

#[tauri::command]
pub async fn start_oss_download(
    oss_transfer_service: State<'_, OssTransferService>,
    app_handle: AppHandle,
    input: StartOssDownloadInput,
) -> Result<CommandResponse<OssTransferStart>, AppError> {
    respond(
        "start_oss_download",
        oss_transfer_service.start_download(input, app_handle).await,
    )
}

#[tauri::command(rename_all = "camelCase")]
pub async fn resume_oss_transfer(
    oss_transfer_service: State<'_, OssTransferService>,
    app_handle: AppHandle,
    input: OssOperationInput,
) -> Result<CommandResponse<OssTransferStart>, AppError> {
    respond(
        "resume_oss_transfer",
        oss_transfer_service.resume(input, app_handle).await,
    )
}

#[tauri::command(rename_all = "camelCase")]
pub async fn cancel_oss_transfer(
    oss_transfer_service: State<'_, OssTransferService>,
    app_handle: AppHandle,
    input: OssOperationInput,
) -> Result<CommandResponse<OssTransferTaskView>, AppError> {
    respond(
        "cancel_oss_transfer",
        oss_transfer_service.cancel(input, Some(app_handle)).await,
    )
}

#[tauri::command(rename_all = "camelCase")]
pub async fn confirm_oss_transfer(
    oss_transfer_service: State<'_, OssTransferService>,
    app_handle: AppHandle,
    input: OssOperationInput,
) -> Result<CommandResponse<OssTransferTaskView>, AppError> {
    respond(
        "confirm_oss_transfer",
        oss_transfer_service
            .confirm_completion(input, Some(app_handle))
            .await,
    )
}

fn respond<T>(
    operation: &'static str,
    result: Result<T, AppError>,
) -> Result<CommandResponse<T>, AppError> {
    match result {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::oss-transfer",
                "{} failed code={} message={} details={:?}",
                operation,
                error.code,
                error.message,
                error.details
            );
            Ok(CommandResponse::failure(error))
        }
    }
}
