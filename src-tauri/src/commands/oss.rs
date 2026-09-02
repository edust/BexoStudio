use tauri::State;

use crate::{
    domain::{
        CopyOssObjectInput, CopyOssObjectResult, CreateOssFolderInput, CreateOssFolderResult,
        DeleteOssObjectsInput, DeleteOssObjectsResult, DeleteResult, GetOssObjectDownloadUrlInput,
        GetOssObjectDownloadUrlResult, HeadOssObjectInput, ListOssObjectsInput, OssAccountSummary,
        OssObjectMetadata, OssObjectPage, OssProbeResult, OssTargetRecord, TestOssTargetInput,
        UpsertOssAccountInput, UpsertOssTargetInput,
    },
    error::{AppError, CommandResponse},
    services::OssService,
};

#[tauri::command]
pub async fn list_oss_accounts(
    oss_service: State<'_, OssService>,
) -> Result<CommandResponse<Vec<OssAccountSummary>>, AppError> {
    respond("list_oss_accounts", oss_service.list_accounts().await)
}

#[tauri::command]
pub async fn upsert_oss_account(
    oss_service: State<'_, OssService>,
    input: UpsertOssAccountInput,
) -> Result<CommandResponse<OssAccountSummary>, AppError> {
    respond(
        "upsert_oss_account",
        oss_service.upsert_account(input).await,
    )
}

#[tauri::command]
pub async fn delete_oss_account(
    oss_service: State<'_, OssService>,
    id: String,
) -> Result<CommandResponse<DeleteResult>, AppError> {
    respond("delete_oss_account", oss_service.delete_account(id).await)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn list_oss_targets(
    oss_service: State<'_, OssService>,
    account_id: String,
) -> Result<CommandResponse<Vec<OssTargetRecord>>, AppError> {
    respond(
        "list_oss_targets",
        oss_service.list_targets(account_id).await,
    )
}

#[tauri::command]
pub async fn upsert_oss_target(
    oss_service: State<'_, OssService>,
    input: UpsertOssTargetInput,
) -> Result<CommandResponse<OssTargetRecord>, AppError> {
    respond("upsert_oss_target", oss_service.upsert_target(input).await)
}

#[tauri::command]
pub async fn delete_oss_target(
    oss_service: State<'_, OssService>,
    id: String,
) -> Result<CommandResponse<DeleteResult>, AppError> {
    respond("delete_oss_target", oss_service.delete_target(id).await)
}

#[tauri::command]
pub async fn list_oss_objects(
    oss_service: State<'_, OssService>,
    input: ListOssObjectsInput,
) -> Result<CommandResponse<OssObjectPage>, AppError> {
    respond("list_oss_objects", oss_service.list_objects(input).await)
}

#[tauri::command]
pub async fn head_oss_object(
    oss_service: State<'_, OssService>,
    input: HeadOssObjectInput,
) -> Result<CommandResponse<OssObjectMetadata>, AppError> {
    respond("head_oss_object", oss_service.head_object(input).await)
}

#[tauri::command]
pub async fn get_oss_object_download_url(
    oss_service: State<'_, OssService>,
    input: GetOssObjectDownloadUrlInput,
) -> Result<CommandResponse<GetOssObjectDownloadUrlResult>, AppError> {
    respond(
        "get_oss_object_download_url",
        oss_service.get_object_download_url(input).await,
    )
}

#[tauri::command]
pub async fn create_oss_folder(
    oss_service: State<'_, OssService>,
    input: CreateOssFolderInput,
) -> Result<CommandResponse<CreateOssFolderResult>, AppError> {
    respond("create_oss_folder", oss_service.create_folder(input).await)
}

#[tauri::command]
pub async fn test_oss_target(
    oss_service: State<'_, OssService>,
    input: TestOssTargetInput,
) -> Result<CommandResponse<OssProbeResult>, AppError> {
    respond("test_oss_target", oss_service.test_target(input).await)
}

#[tauri::command]
pub async fn delete_oss_objects(
    oss_service: State<'_, OssService>,
    input: DeleteOssObjectsInput,
) -> Result<CommandResponse<DeleteOssObjectsResult>, AppError> {
    respond(
        "delete_oss_objects",
        oss_service.delete_objects(input).await,
    )
}

#[tauri::command]
pub async fn copy_oss_object(
    oss_service: State<'_, OssService>,
    input: CopyOssObjectInput,
) -> Result<CommandResponse<CopyOssObjectResult>, AppError> {
    respond("copy_oss_object", oss_service.copy_object(input).await)
}

fn respond<T>(
    operation: &'static str,
    result: Result<T, AppError>,
) -> Result<CommandResponse<T>, AppError> {
    match result {
        Ok(data) => Ok(CommandResponse::success(data)),
        Err(error) => {
            log::error!(
                target: "bexo::command::oss",
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
