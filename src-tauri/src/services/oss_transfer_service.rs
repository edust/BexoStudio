use std::{
    collections::HashMap,
    future::Future,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, UNIX_EPOCH},
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::{
    fs::{self, File, OpenOptions},
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, SeekFrom},
    time::{sleep, timeout},
};

use crate::{
    adapters::{
        AlibabaOssAdapter, ObjectStorageAdapter, OssMultipartAbortRequest,
        OssMultipartCompletePart, OssMultipartCompleteRequest, OssMultipartInitiateRequest,
        OssMultipartUploadPartRequest, OssObjectGetRequest, OssObjectHeadRequest,
        OssObjectPutRequest,
    },
    domain::{
        normalize_object_key, validate_operation_id, validate_target_id, OssCredential,
        OssOperationInput, OssTransferStart, OssTransferTaskRecord, OssTransferTaskView,
        StartOssDownloadInput, StartOssUploadInput, OSS_MAX_TRANSFER_RETRIES,
        OSS_TRANSFER_PROGRESS_EVENT_NAME, OSS_TRANSFER_STATE_EVENT_NAME,
    },
    error::{AppError, AppResult},
    persistence::{
        get_oss_account, get_oss_target, get_oss_transfer_task, insert_oss_transfer_task,
        list_oss_transfer_tasks, list_resumable_oss_transfer_tasks, update_oss_transfer_task,
        Database,
    },
};

use super::OssCredentialService;

const FILE_IO_TIMEOUT: Duration = Duration::from_secs(30);
const STREAM_CHUNK_TIMEOUT: Duration = Duration::from_secs(120);
const DEFAULT_PART_SIZE: u64 = 4 * 1024 * 1024;
const MAX_PART_COUNT: u64 = 10_000;

#[derive(Debug, Clone)]
pub struct OssTransferService {
    database: Database,
    credential_service: OssCredentialService,
    adapter: Arc<AlibabaOssAdapter>,
    active_tasks: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UploadCheckpoint {
    kind: String,
    file_size: u64,
    modified_millis: u64,
    part_size: u64,
    overwrite: bool,
    parts: Vec<UploadPartCheckpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UploadPartCheckpoint {
    part_number: u32,
    size: u64,
    etag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DownloadCheckpoint {
    kind: String,
    object_size: u64,
    etag: Option<String>,
    bytes_completed: u64,
    overwrite: bool,
}

#[derive(Debug, Clone, Copy)]
struct FileSnapshot {
    size: u64,
    modified_millis: u64,
}

impl OssTransferService {
    pub fn new(database: Database) -> AppResult<Self> {
        Ok(Self {
            database,
            credential_service: OssCredentialService::new(),
            adapter: Arc::new(AlibabaOssAdapter::new()?),
            active_tasks: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub async fn list_tasks(&self) -> AppResult<Vec<OssTransferTaskView>> {
        let tasks = self
            .database
            .read("list_oss_transfer_tasks", list_oss_transfer_tasks)
            .await?;
        let mut views = Vec::with_capacity(tasks.len());
        for task in tasks {
            views.push(self.task_view(&task).await?);
        }
        Ok(views)
    }

    pub async fn recover_interrupted_tasks(&self) -> AppResult<Vec<String>> {
        let tasks = self
            .database
            .read(
                "list_resumable_oss_transfer_tasks",
                list_resumable_oss_transfer_tasks,
            )
            .await?;
        let mut recovered = Vec::new();
        for mut task in tasks {
            if matches!(task.status.as_str(), "running" | "queued" | "cancelling") {
                task.status = "paused".to_string();
                task.last_error_code = Some("OSS_TRANSFER_INTERRUPTED".to_string());
                task.last_error_message =
                    Some("应用退出时传输尚未完成，可在传输队列中恢复。".to_string());
                self.update_task(&mut task, None).await?;
                recovered.push(task.id);
            }
        }
        Ok(recovered)
    }

    pub async fn start_upload(
        &self,
        input: StartOssUploadInput,
        app_handle: AppHandle,
    ) -> AppResult<OssTransferStart> {
        let target_id = validate_target_id(input.target_id)?;
        let object_key = normalize_object_key(input.object_key)?;
        let local_path = validate_local_path(input.local_path, "localPath")?;
        let target = self.load_target(target_id.clone()).await?;
        ensure_key_within_target(&target, &object_key)?;
        let account = self.load_account(target.account_id.clone()).await?;
        self.credential_service
            .read(account.credential_ref.clone())
            .await?;
        let snapshot = file_snapshot(&local_path).await?;
        if let Some(existing) = self
            .find_resumable_task("upload", &target.id, &object_key, &local_path)
            .await?
        {
            return Ok(OssTransferStart {
                operation_id: existing.id.clone(),
                task_id: existing.id,
            });
        }

        let task_id = uuid::Uuid::new_v4().to_string();
        let checkpoint = UploadCheckpoint {
            kind: "upload".to_string(),
            file_size: snapshot.size,
            modified_millis: snapshot.modified_millis,
            part_size: calculate_part_size(snapshot.size),
            overwrite: input.overwrite,
            parts: Vec::new(),
        };
        let task = new_transfer_task(
            task_id.clone(),
            account.id,
            target.id,
            "upload",
            object_key,
            local_path,
            snapshot.size,
            Some(serialize_checkpoint(&checkpoint)?),
        );
        let task = self
            .database
            .write("insert_oss_upload_task", move |connection| {
                insert_oss_transfer_task(connection, task)
            })
            .await?;
        self.spawn_task(task.clone(), app_handle)?;
        Ok(OssTransferStart {
            operation_id: task.id.clone(),
            task_id: task.id,
        })
    }

    pub async fn start_download(
        &self,
        input: StartOssDownloadInput,
        app_handle: AppHandle,
    ) -> AppResult<OssTransferStart> {
        let target_id = validate_target_id(input.target_id)?;
        let object_key = normalize_object_key(input.object_key)?;
        let local_path = validate_local_path(input.local_path, "localPath")?;
        let target = self.load_target(target_id.clone()).await?;
        ensure_key_within_target(&target, &object_key)?;
        let account = self.load_account(target.account_id.clone()).await?;
        let credential = self
            .credential_service
            .read(account.credential_ref.clone())
            .await?;
        if !input.overwrite && path_exists(&local_path).await? {
            return Err(AppError::new(
                "OSS_DOWNLOAD_CONFLICT",
                "the local destination already exists",
            )
            .with_detail("localPath", local_path.display().to_string()));
        }
        let metadata = self
            .adapter
            .head_object(OssObjectHeadRequest {
                target: target.clone(),
                credential,
                object_key: object_key.clone(),
            })
            .await?;
        let object_size = metadata.size.ok_or_else(|| {
            AppError::new(
                "OSS_RESPONSE_INVALID",
                "OSS object metadata did not contain a size",
            )
        })?;
        if let Some(existing) = self
            .find_resumable_task("download", &target.id, &object_key, &local_path)
            .await?
        {
            return Ok(OssTransferStart {
                operation_id: existing.id.clone(),
                task_id: existing.id,
            });
        }

        let task_id = uuid::Uuid::new_v4().to_string();
        let checkpoint = DownloadCheckpoint {
            kind: "download".to_string(),
            object_size,
            etag: metadata.etag,
            bytes_completed: 0,
            overwrite: input.overwrite,
        };
        let task = new_transfer_task(
            task_id.clone(),
            account.id,
            target.id,
            "download",
            object_key,
            local_path,
            object_size,
            Some(serialize_checkpoint(&checkpoint)?),
        );
        let task = self
            .database
            .write("insert_oss_download_task", move |connection| {
                insert_oss_transfer_task(connection, task)
            })
            .await?;
        self.spawn_task(task.clone(), app_handle)?;
        Ok(OssTransferStart {
            operation_id: task.id.clone(),
            task_id: task.id,
        })
    }

    pub async fn resume(
        &self,
        input: OssOperationInput,
        app_handle: AppHandle,
    ) -> AppResult<OssTransferStart> {
        let operation_id = validate_operation_id(input.operation_id)?;
        let mut task = self.load_task(operation_id.clone()).await?;
        if task.status == "completed" {
            return Err(AppError::new(
                "OSS_TRANSFER_ALREADY_COMPLETED",
                "OSS transfer task is already completed",
            ));
        }
        if task.status == "needs_confirmation" {
            return Err(AppError::new(
                "OSS_TRANSFER_NEEDS_CONFIRMATION",
                "the previous completion result is unknown; confirm the remote object before resuming",
            ));
        }
        if self.is_active(&operation_id)? {
            return Ok(OssTransferStart {
                operation_id: task.id.clone(),
                task_id: task.id,
            });
        }
        if !matches!(
            task.status.as_str(),
            "paused" | "failed" | "queued" | "running"
        ) {
            return Err(AppError::new(
                "OSS_TRANSFER_NOT_RESUMABLE",
                "OSS transfer task cannot be resumed from its current state",
            )
            .with_detail("status", task.status));
        }
        task.status = "queued".to_string();
        task.last_error_code = None;
        task.last_error_message = None;
        self.update_task(&mut task, None).await?;
        self.spawn_task(task.clone(), app_handle)?;
        Ok(OssTransferStart {
            operation_id: task.id.clone(),
            task_id: task.id,
        })
    }

    pub async fn cancel(
        &self,
        input: OssOperationInput,
        app_handle: Option<AppHandle>,
    ) -> AppResult<OssTransferTaskView> {
        let operation_id = validate_operation_id(input.operation_id)?;
        let mut task = self.load_task(operation_id.clone()).await?;
        if matches!(task.status.as_str(), "completed" | "cancelled") {
            return self.task_view(&task).await;
        }
        if task.status == "needs_confirmation" {
            return Err(AppError::new(
                "OSS_TRANSFER_NEEDS_CONFIRMATION",
                "confirm the remote object before cancelling this transfer",
            ));
        }
        if let Some(cancel_token) = self.active_token(&operation_id)? {
            cancel_token.store(true, Ordering::SeqCst);
            task.status = "cancelling".to_string();
            self.update_task(&mut task, app_handle.as_ref()).await?;
            return self.task_view(&task).await;
        }

        if let Some(upload_id) = task.upload_id.clone() {
            let target = self.load_target(task.target_id.clone()).await?;
            let account = self.load_account(task.account_id.clone()).await?;
            let credential = self.credential_service.read(account.credential_ref).await?;
            if let Err(error) = self
                .adapter
                .abort_multipart_upload(OssMultipartAbortRequest {
                    target,
                    credential,
                    object_key: task.object_key.clone(),
                    upload_id,
                })
                .await
            {
                task.status = "paused".to_string();
                task.last_error_code = Some("OSS_MULTIPART_CLEANUP_REQUIRED".to_string());
                task.last_error_message = Some(error.message.clone());
                self.update_task(&mut task, app_handle.as_ref()).await?;
                return Err(error.with_detail(
                    "cleanup",
                    "multipart upload remains on OSS and requires cleanup",
                ));
            }
            task.upload_id = None;
            task.checkpoint_json = None;
        }
        task.status = "cancelled".to_string();
        task.last_error_code = Some("OSS_TRANSFER_CANCELLED".to_string());
        task.last_error_message = Some("传输已取消".to_string());
        self.update_task(&mut task, app_handle.as_ref()).await?;
        self.task_view(&task).await
    }

    pub async fn confirm_completion(
        &self,
        input: OssOperationInput,
        app_handle: Option<AppHandle>,
    ) -> AppResult<OssTransferTaskView> {
        let operation_id = validate_operation_id(input.operation_id)?;
        let mut task = self.load_task(operation_id).await?;
        if task.status != "needs_confirmation" {
            return Err(AppError::new(
                "OSS_TRANSFER_CONFIRMATION_NOT_REQUIRED",
                "OSS transfer is not waiting for remote completion confirmation",
            )
            .with_detail("status", task.status));
        }
        if task.operation != "upload" {
            return Err(AppError::new(
                "OSS_TRANSFER_CONFIRMATION_UNSUPPORTED",
                "remote completion confirmation is only supported for uploads",
            ));
        }

        let target = self.load_target(task.target_id.clone()).await?;
        let account = self.load_account(task.account_id.clone()).await?;
        let credential = self.credential_service.read(account.credential_ref).await?;
        let metadata = match self
            .adapter
            .head_object(OssObjectHeadRequest {
                target,
                credential,
                object_key: task.object_key.clone(),
            })
            .await
        {
            Ok(metadata) => metadata,
            Err(error)
                if matches!(
                    error.code.as_str(),
                    "OSS_OBJECT_NOT_FOUND" | "OSS_OBJECT_OR_BUCKET_NOT_FOUND"
                ) =>
            {
                task.status = "paused".to_string();
                task.last_error_code = Some("OSS_TRANSFER_REMOTE_NOT_FOUND".to_string());
                task.last_error_message =
                    Some("未发现已完成的远端对象，可恢复上传或取消并清理未完成分片。".to_string());
                self.update_task(&mut task, app_handle.as_ref()).await?;
                return self.task_view(&task).await;
            }
            Err(error) => return Err(error),
        };

        let actual_size = metadata.size.ok_or_else(|| {
            AppError::new(
                "OSS_RESPONSE_INVALID",
                "confirmed OSS object metadata did not contain a size",
            )
        })?;
        if actual_size != task.total_bytes {
            return Err(AppError::new(
                "OSS_TRANSFER_REMOTE_MISMATCH",
                "remote object size does not match the transfer",
            )
            .with_detail("expected", task.total_bytes.to_string())
            .with_detail("actual", actual_size.to_string()));
        }

        task.status = "completed".to_string();
        task.bytes_completed = task.total_bytes;
        task.upload_id = None;
        task.checkpoint_json = None;
        task.last_error_code = None;
        task.last_error_message = None;
        self.update_task(&mut task, app_handle.as_ref()).await?;
        self.task_view(&task).await
    }

    fn spawn_task(&self, task: OssTransferTaskRecord, app_handle: AppHandle) -> AppResult<()> {
        let cancel_token = Arc::new(AtomicBool::new(false));
        {
            let mut active = self.active_tasks.lock().map_err(|_| {
                AppError::new(
                    "OSS_TRANSFER_REGISTRY_UNAVAILABLE",
                    "OSS transfer registry is unavailable",
                )
            })?;
            if active.contains_key(&task.id) {
                return Ok(());
            }
            active.insert(task.id.clone(), Arc::clone(&cancel_token));
        }
        let service = self.clone();
        let task_id = task.id.clone();
        tauri::async_runtime::spawn(async move {
            service.run_task(task, cancel_token, app_handle).await;
            if let Ok(mut active) = service.active_tasks.lock() {
                active.remove(&task_id);
            }
        });
        Ok(())
    }

    async fn run_task(
        &self,
        mut task: OssTransferTaskRecord,
        cancel_token: Arc<AtomicBool>,
        app_handle: AppHandle,
    ) {
        task.status = "running".to_string();
        task.last_error_code = None;
        task.last_error_message = None;
        if let Err(error) = self.update_task(&mut task, Some(&app_handle)).await {
            log::error!(
                target: "bexo::service::oss-transfer",
                "failed to mark OSS transfer running task_id={} code={} reason={}",
                task.id,
                error.code,
                error.message
            );
            return;
        }

        let result = match task.operation.as_str() {
            "upload" => self.run_upload(&mut task, &cancel_token, &app_handle).await,
            "download" => {
                self.run_download(&mut task, &cancel_token, &app_handle)
                    .await
            }
            _ => Err(AppError::new(
                "OSS_TRANSFER_OPERATION_INVALID",
                "OSS transfer operation is invalid",
            )),
        };

        match result {
            Ok(()) => {
                task.status = "completed".to_string();
                task.bytes_completed = task.total_bytes;
                task.upload_id = None;
                task.checkpoint_json = None;
                if let Err(error) = self.update_task(&mut task, Some(&app_handle)).await {
                    log::error!(
                        target: "bexo::service::oss-transfer",
                        "failed to persist completed OSS transfer task_id={} code={} reason={}",
                        task.id,
                        error.code,
                        error.message
                    );
                }
            }
            Err(error) => {
                if error.code == "OSS_TRANSFER_CANCELLED" {
                    task.status = "cancelled".to_string();
                    task.last_error_code = None;
                    task.last_error_message = None;
                } else if error.code == "OSS_MULTIPART_CLEANUP_REQUIRED" {
                    task.status = "paused".to_string();
                    task.last_error_code = Some(error.code.clone());
                    task.last_error_message = Some(error.message.clone());
                } else if error.code == "OSS_TRANSFER_NEEDS_CONFIRMATION" {
                    task.status = "needs_confirmation".to_string();
                    task.last_error_code = Some(error.code.clone());
                    task.last_error_message = Some(error.message.clone());
                } else {
                    task.status = "failed".to_string();
                    task.last_error_code = Some(error.code.clone());
                    task.last_error_message = Some(error.message.clone());
                }
                if let Err(persist_error) = self.update_task(&mut task, Some(&app_handle)).await {
                    log::error!(
                        target: "bexo::service::oss-transfer",
                        "failed to persist OSS transfer failure task_id={} code={} reason={}",
                        task.id,
                        persist_error.code,
                        persist_error.message
                    );
                }
                log::error!(
                    target: "bexo::service::oss-transfer",
                    "OSS transfer failed task_id={} code={} message={}",
                    task.id,
                    error.code,
                    error.message
                );
            }
        }
    }

    async fn run_upload(
        &self,
        task: &mut OssTransferTaskRecord,
        cancel_token: &AtomicBool,
        app_handle: &AppHandle,
    ) -> AppResult<()> {
        let result = self.run_upload_inner(task, cancel_token, app_handle).await;
        if result
            .as_ref()
            .err()
            .is_some_and(|error| error.code == "OSS_TRANSFER_CANCELLED")
        {
            if let Some(upload_id) = task.upload_id.clone() {
                let target = self.load_target(task.target_id.clone()).await?;
                let account = self.load_account(task.account_id.clone()).await?;
                let credential = self.credential_service.read(account.credential_ref).await?;
                if let Err(error) = self
                    .adapter
                    .abort_multipart_upload(OssMultipartAbortRequest {
                        target,
                        credential,
                        object_key: task.object_key.clone(),
                        upload_id,
                    })
                    .await
                {
                    return Err(AppError::new(
                        "OSS_MULTIPART_CLEANUP_REQUIRED",
                        "multipart upload was cancelled but OSS cleanup failed",
                    )
                    .with_detail("reason", error.message));
                }
                task.upload_id = None;
                task.checkpoint_json = None;
            }
        }
        result
    }

    async fn run_upload_inner(
        &self,
        task: &mut OssTransferTaskRecord,
        cancel_token: &AtomicBool,
        app_handle: &AppHandle,
    ) -> AppResult<()> {
        let target = self.load_target(task.target_id.clone()).await?;
        ensure_key_within_target(&target, &task.object_key)?;
        let account = self.load_account(task.account_id.clone()).await?;
        let credential = self.credential_service.read(account.credential_ref).await?;
        let local_path = PathBuf::from(&task.local_path);
        let snapshot = file_snapshot(&local_path).await?;
        let mut checkpoint: UploadCheckpoint = parse_checkpoint(task.checkpoint_json.as_deref())?;
        validate_upload_checkpoint(&checkpoint, &snapshot)?;
        if snapshot.size <= checkpoint.part_size && task.upload_id.is_none() {
            check_cancel(cancel_token)?;
            let body = await_cancelable(cancel_token, read_file_bytes(&local_path)).await?;
            let after_read = file_snapshot(&local_path).await?;
            validate_same_file(snapshot, after_read)?;
            let put_result = await_cancelable(
                cancel_token,
                self.adapter.put_object(OssObjectPutRequest {
                    target,
                    credential,
                    object_key: task.object_key.clone(),
                    body,
                    content_type: content_type_for_path(&local_path),
                    overwrite: checkpoint.overwrite,
                }),
            )
            .await;
            let put_result = match put_result {
                Err(error) if error.code == "OSS_TRANSFER_CANCELLED" => {
                    return Err(unknown_upload_cancellation_error());
                }
                result => result,
            };
            if let Err(error) = put_result {
                if is_unknown_remote_result(&error) {
                    return Err(unknown_upload_result_error(error));
                }
                return Err(error);
            }
            task.bytes_completed = snapshot.size;
            self.update_task(task, Some(app_handle)).await?;
            return Ok(());
        }

        if task.upload_id.is_none() {
            check_cancel(cancel_token)?;
            let upload = self
                .adapter
                .initiate_multipart_upload(OssMultipartInitiateRequest {
                    target: target.clone(),
                    credential: credential.clone(),
                    object_key: task.object_key.clone(),
                    content_type: content_type_for_path(&local_path),
                    overwrite: checkpoint.overwrite,
                })
                .await
                .map_err(|error| {
                    if is_unknown_remote_result(&error) {
                        unknown_upload_result_error(error)
                    } else {
                        error
                    }
                })?;
            task.upload_id = Some(upload.upload_id);
            task.checkpoint_json = Some(serialize_checkpoint(&checkpoint)?);
            self.update_task(task, Some(app_handle)).await?;
        }

        let upload_id = task.upload_id.clone().ok_or_else(|| {
            AppError::new(
                "OSS_MULTIPART_INCOMPLETE",
                "multipart upload has no upload ID",
            )
        })?;
        let mut file = timeout(FILE_IO_TIMEOUT, File::open(&local_path))
            .await
            .map_err(|_| {
                AppError::new(
                    "OSS_LOCAL_FILE_TIMEOUT",
                    "opening the upload file timed out",
                )
                .retryable(true)
            })?
            .map_err(|error| {
                AppError::new("OSS_LOCAL_FILE_READ_FAILED", "failed to open upload file")
                    .with_detail("reason", error.to_string())
            })?;
        let part_count = part_count(snapshot.size, checkpoint.part_size)?;
        for part_number in 1..=part_count {
            check_cancel(cancel_token)?;
            let part_number_u32 = u32::try_from(part_number).map_err(|_| {
                AppError::new(
                    "OSS_MULTIPART_INCOMPLETE",
                    "multipart part number is invalid",
                )
            })?;
            let part_size = part_size_for(snapshot.size, checkpoint.part_size, part_number);
            if checkpoint
                .parts
                .iter()
                .any(|part| part.part_number == part_number_u32)
            {
                continue;
            }
            await_cancelable(cancel_token, async {
                timeout(
                    FILE_IO_TIMEOUT,
                    file.seek(SeekFrom::Start((part_number - 1) * checkpoint.part_size)),
                )
                .await
                .map_err(|_| {
                    AppError::new(
                        "OSS_LOCAL_FILE_TIMEOUT",
                        "seeking the upload file timed out",
                    )
                    .retryable(true)
                })?
                .map_err(|error| {
                    AppError::new("OSS_LOCAL_FILE_READ_FAILED", "failed to seek upload file")
                        .with_detail("reason", error.to_string())
                })?;
                Ok::<(), AppError>(())
            })
            .await?;
            let mut body = vec![
                0_u8;
                usize::try_from(part_size).map_err(|_| {
                    AppError::new("OSS_MULTIPART_INCOMPLETE", "multipart part is too large")
                })?
            ];
            await_cancelable(cancel_token, async {
                timeout(FILE_IO_TIMEOUT, file.read_exact(&mut body))
                    .await
                    .map_err(|_| {
                        AppError::new("OSS_LOCAL_FILE_TIMEOUT", "reading an upload part timed out")
                            .retryable(true)
                    })?
                    .map_err(|error| {
                        AppError::new("OSS_LOCAL_FILE_READ_FAILED", "failed to read upload part")
                            .with_detail("reason", error.to_string())
                    })?;
                Ok::<(), AppError>(())
            })
            .await?;
            let uploaded = await_cancelable(
                cancel_token,
                self.adapter
                    .upload_multipart_part(OssMultipartUploadPartRequest {
                        target: target.clone(),
                        credential: credential.clone(),
                        object_key: task.object_key.clone(),
                        upload_id: upload_id.clone(),
                        part_number: part_number_u32,
                        body,
                    }),
            )
            .await?;
            checkpoint.parts.push(UploadPartCheckpoint {
                part_number: uploaded.part_number,
                size: part_size,
                etag: uploaded.etag,
            });
            checkpoint.parts.sort_by_key(|part| part.part_number);
            task.bytes_completed = checkpoint.parts.iter().map(|part| part.size).sum();
            task.checkpoint_json = Some(serialize_checkpoint(&checkpoint)?);
            self.update_task(task, Some(app_handle)).await?;
        }

        check_cancel(cancel_token)?;
        let complete_result = await_cancelable(
            cancel_token,
            self.adapter
                .complete_multipart_upload(OssMultipartCompleteRequest {
                    target,
                    credential,
                    object_key: task.object_key.clone(),
                    upload_id,
                    parts: checkpoint
                        .parts
                        .iter()
                        .map(|part| OssMultipartCompletePart {
                            part_number: part.part_number,
                            etag: part.etag.clone(),
                        })
                        .collect(),
                    overwrite: checkpoint.overwrite,
                }),
        )
        .await;
        let complete_result = match complete_result {
            Err(error) if error.code == "OSS_TRANSFER_CANCELLED" => {
                return Err(unknown_upload_cancellation_error());
            }
            result => result,
        };
        if let Err(error) = complete_result {
            if is_unknown_remote_result(&error) {
                return Err(unknown_upload_result_error(error));
            }
            return Err(error);
        }
        Ok(())
    }

    async fn run_download(
        &self,
        task: &mut OssTransferTaskRecord,
        cancel_token: &AtomicBool,
        app_handle: &AppHandle,
    ) -> AppResult<()> {
        let temp_path = download_temp_path(task)?;
        let result = self
            .run_download_inner(task, cancel_token, app_handle, &temp_path)
            .await;
        if result
            .as_ref()
            .err()
            .is_some_and(|error| error.code == "OSS_TRANSFER_CANCELLED")
        {
            if let Err(error) = remove_file_if_exists(&temp_path).await {
                return Err(AppError::new(
                    "OSS_TRANSFER_CLEANUP_REQUIRED",
                    "download was cancelled but the temporary file could not be removed",
                )
                .with_detail("reason", error.message));
            }
        }
        result
    }

    async fn run_download_inner(
        &self,
        task: &mut OssTransferTaskRecord,
        cancel_token: &AtomicBool,
        app_handle: &AppHandle,
        temp_path: &Path,
    ) -> AppResult<()> {
        let target = self.load_target(task.target_id.clone()).await?;
        ensure_key_within_target(&target, &task.object_key)?;
        let account = self.load_account(task.account_id.clone()).await?;
        let credential = self.credential_service.read(account.credential_ref).await?;
        let metadata = await_cancelable(
            cancel_token,
            self.adapter.head_object(OssObjectHeadRequest {
                target: target.clone(),
                credential: credential.clone(),
                object_key: task.object_key.clone(),
            }),
        )
        .await?;
        let object_size = metadata.size.ok_or_else(|| {
            AppError::new(
                "OSS_RESPONSE_INVALID",
                "OSS object metadata did not contain a size",
            )
        })?;
        let mut checkpoint: DownloadCheckpoint = parse_checkpoint(task.checkpoint_json.as_deref())?;
        if checkpoint.object_size != object_size || checkpoint.etag != metadata.etag {
            return Err(AppError::new(
                "OSS_TRANSFER_SOURCE_CHANGED",
                "remote object changed after the transfer was created",
            ));
        }
        if checkpoint.bytes_completed > object_size {
            return Err(AppError::new(
                "OSS_TRANSFER_CHECKPOINT_INVALID",
                "download checkpoint exceeds the remote object size",
            ));
        }
        task.total_bytes = object_size;
        task.bytes_completed = checkpoint.bytes_completed;
        task.checkpoint_json = Some(serialize_checkpoint(&checkpoint)?);
        self.update_task(task, Some(app_handle)).await?;

        ensure_parent_directory(temp_path).await?;
        let existing_temp_size = file_size(temp_path).await?;
        if existing_temp_size.is_none() && checkpoint.bytes_completed > 0 {
            return Err(AppError::new(
                "OSS_TRANSFER_CHECKPOINT_INVALID",
                "download checkpoint exists but its temporary file is missing",
            ));
        }
        let temp_file_exists = existing_temp_size.is_some();
        let existing_temp_size = existing_temp_size.unwrap_or(0);
        if !temp_file_exists || existing_temp_size != checkpoint.bytes_completed {
            let file = timeout(
                FILE_IO_TIMEOUT,
                OpenOptions::new().create(true).write(true).open(temp_path),
            )
            .await
            .map_err(|_| {
                AppError::new(
                    "OSS_LOCAL_FILE_TIMEOUT",
                    "opening the download temporary file timed out",
                )
                .retryable(true)
            })?
            .map_err(|error| {
                AppError::new(
                    "OSS_LOCAL_FILE_WRITE_FAILED",
                    "failed to open download temporary file",
                )
                .with_detail("reason", error.to_string())
            })?;
            timeout(FILE_IO_TIMEOUT, file.set_len(checkpoint.bytes_completed))
                .await
                .map_err(|_| {
                    AppError::new(
                        "OSS_LOCAL_FILE_TIMEOUT",
                        "aligning the download checkpoint file timed out",
                    )
                    .retryable(true)
                })?
                .map_err(|error| {
                    AppError::new(
                        "OSS_LOCAL_FILE_WRITE_FAILED",
                        "failed to align download checkpoint file",
                    )
                    .with_detail("reason", error.to_string())
                })?;
        }

        while checkpoint.bytes_completed < object_size {
            check_cancel(cancel_token)?;
            let start = checkpoint.bytes_completed;
            let end = (start + DEFAULT_PART_SIZE).min(object_size) - 1;
            let expected = end - start + 1;
            let bytes = self
                .download_range(
                    &target,
                    &credential,
                    &task.object_key,
                    start,
                    end,
                    expected,
                    cancel_token,
                )
                .await?;
            let mut file = timeout(
                FILE_IO_TIMEOUT,
                OpenOptions::new().create(true).append(true).open(temp_path),
            )
            .await
            .map_err(|_| {
                AppError::new(
                    "OSS_LOCAL_FILE_TIMEOUT",
                    "opening the download temporary file timed out",
                )
                .retryable(true)
            })?
            .map_err(|error| {
                AppError::new(
                    "OSS_LOCAL_FILE_WRITE_FAILED",
                    "failed to open download temporary file",
                )
                .with_detail("reason", error.to_string())
            })?;
            timeout(FILE_IO_TIMEOUT, file.write_all(&bytes))
                .await
                .map_err(|_| {
                    AppError::new(
                        "OSS_LOCAL_FILE_TIMEOUT",
                        "writing downloaded data timed out",
                    )
                    .retryable(true)
                })?
                .map_err(|error| {
                    AppError::new(
                        "OSS_LOCAL_FILE_WRITE_FAILED",
                        "failed to write downloaded data",
                    )
                    .with_detail("reason", error.to_string())
                })?;
            timeout(FILE_IO_TIMEOUT, file.flush())
                .await
                .map_err(|_| {
                    AppError::new(
                        "OSS_LOCAL_FILE_TIMEOUT",
                        "flushing downloaded data timed out",
                    )
                    .retryable(true)
                })?
                .map_err(|error| {
                    AppError::new(
                        "OSS_LOCAL_FILE_WRITE_FAILED",
                        "failed to flush downloaded data",
                    )
                    .with_detail("reason", error.to_string())
                })?;
            checkpoint.bytes_completed += bytes.len() as u64;
            task.bytes_completed = checkpoint.bytes_completed;
            task.checkpoint_json = Some(serialize_checkpoint(&checkpoint)?);
            self.update_task(task, Some(app_handle)).await?;
        }

        check_cancel(cancel_token)?;
        finalize_download_file(temp_path, Path::new(&task.local_path), checkpoint.overwrite)
            .await?;
        Ok(())
    }

    async fn download_range(
        &self,
        target: &crate::domain::OssTargetRecord,
        credential: &OssCredential,
        object_key: &str,
        start: u64,
        end: u64,
        expected: u64,
        cancel_token: &AtomicBool,
    ) -> AppResult<Vec<u8>> {
        let mut last_error = None;
        for attempt in 0..OSS_MAX_TRANSFER_RETRIES {
            check_cancel(cancel_token)?;
            let mut response = match await_cancelable(
                cancel_token,
                self.adapter.get_object(OssObjectGetRequest {
                    target: target.clone(),
                    credential: credential.clone(),
                    object_key: object_key.to_string(),
                    range_start: Some(start),
                    range_end: Some(end),
                }),
            )
            .await
            {
                Ok(response) => response,
                Err(error) => {
                    if !error.retryable.unwrap_or(false) || attempt + 1 >= OSS_MAX_TRANSFER_RETRIES
                    {
                        return Err(error);
                    }
                    last_error = Some(error);
                    sleep(transfer_retry_delay(attempt)).await;
                    continue;
                }
            };
            if let Some(content_length) = response.content_length() {
                if content_length != expected {
                    return Err(AppError::new(
                        "OSS_RESPONSE_INVALID",
                        "OSS range response length does not match the requested size",
                    )
                    .with_detail("expected", expected.to_string())
                    .with_detail("actual", content_length.to_string()));
                }
            }
            let mut body = Vec::with_capacity(usize::try_from(expected).map_err(|_| {
                AppError::new("OSS_TRANSFER_RANGE_INVALID", "download range is too large")
            })?);
            let mut stream_error = None;
            loop {
                check_cancel(cancel_token)?;
                let next = match timeout(STREAM_CHUNK_TIMEOUT, response.chunk()).await {
                    Ok(Ok(next)) => next,
                    Ok(Err(error)) => {
                        stream_error = Some(
                            AppError::new("OSS_NETWORK_ERROR", "OSS download stream failed")
                                .with_detail("reason", error.to_string())
                                .retryable(true),
                        );
                        break;
                    }
                    Err(_) => {
                        stream_error = Some(
                            AppError::new("OSS_NETWORK_TIMEOUT", "OSS download stream timed out")
                                .retryable(true),
                        );
                        break;
                    }
                };
                let Some(chunk) = next else {
                    break;
                };
                body.extend_from_slice(&chunk);
                if body.len() as u64 > expected {
                    return Err(AppError::new(
                        "OSS_RESPONSE_INVALID",
                        "OSS range response exceeded the requested size",
                    ));
                }
            }
            if let Some(error) = stream_error {
                if attempt + 1 >= OSS_MAX_TRANSFER_RETRIES {
                    return Err(error);
                }
                last_error = Some(error);
                sleep(transfer_retry_delay(attempt)).await;
                continue;
            }
            if body.len() as u64 == expected {
                return Ok(body);
            }
            last_error = Some(
                AppError::new(
                    "OSS_RESPONSE_INVALID",
                    "OSS range response was shorter than requested",
                )
                .retryable(true),
            );
            if attempt + 1 < OSS_MAX_TRANSFER_RETRIES {
                sleep(transfer_retry_delay(attempt)).await;
            }
        }
        Err(last_error.unwrap_or_else(|| {
            AppError::new(
                "OSS_NETWORK_ERROR",
                "OSS download range exhausted its retry budget",
            )
        }))
    }

    async fn update_task(
        &self,
        task: &mut OssTransferTaskRecord,
        app_handle: Option<&AppHandle>,
    ) -> AppResult<()> {
        let updated = self
            .database
            .write("update_oss_transfer_task", {
                let task = task.clone();
                move |connection| update_oss_transfer_task(connection, task)
            })
            .await?;
        *task = updated;
        if let Some(app_handle) = app_handle {
            let view = self.task_view(task).await?;
            emit_transfer_event(app_handle, &view);
        }
        Ok(())
    }

    async fn load_task(&self, operation_id: String) -> AppResult<OssTransferTaskRecord> {
        self.database
            .read("get_oss_transfer_task", move |connection| {
                get_oss_transfer_task(connection, operation_id)
            })
            .await?
            .ok_or_else(|| {
                AppError::new("OSS_TRANSFER_NOT_FOUND", "OSS transfer task was not found")
            })
    }

    async fn load_account(&self, account_id: String) -> AppResult<crate::domain::OssAccountRecord> {
        let account_id = crate::domain::validate_account_id(account_id)?;
        let query_id = account_id.clone();
        self.database
            .read("get_oss_account_for_transfer", move |connection| {
                get_oss_account(connection, query_id)
            })
            .await?
            .ok_or_else(|| {
                AppError::new("OSS_ACCOUNT_NOT_FOUND", "OSS account was not found")
                    .with_detail("accountId", account_id)
            })
    }

    async fn load_target(&self, target_id: String) -> AppResult<crate::domain::OssTargetRecord> {
        let target_id = validate_target_id(target_id)?;
        let query_id = target_id.clone();
        self.database
            .read("get_oss_target_for_transfer", move |connection| {
                get_oss_target(connection, query_id)
            })
            .await?
            .ok_or_else(|| {
                AppError::new("OSS_TARGET_NOT_FOUND", "OSS target was not found")
                    .with_detail("targetId", target_id)
            })
    }

    async fn task_view(&self, task: &OssTransferTaskRecord) -> AppResult<OssTransferTaskView> {
        let target = self.load_target(task.target_id.clone()).await?;
        let account = self.load_account(task.account_id.clone()).await?;
        Ok(OssTransferTaskView {
            task: task.clone(),
            account_display_name: account.display_name,
            target_display_name: target.display_name,
            bucket: target.bucket,
            region: target.region,
        })
    }

    async fn find_resumable_task(
        &self,
        operation: &str,
        target_id: &str,
        object_key: &str,
        local_path: &Path,
    ) -> AppResult<Option<OssTransferTaskRecord>> {
        let local_path = local_path.display().to_string();
        let tasks = self
            .database
            .read(
                "list_resumable_oss_transfer_tasks_for_dedup",
                list_resumable_oss_transfer_tasks,
            )
            .await?;
        Ok(tasks.into_iter().find(|task| {
            task.target_id == target_id
                && task.operation == operation
                && task.object_key == object_key
                && task.local_path == local_path
        }))
    }

    fn is_active(&self, operation_id: &str) -> AppResult<bool> {
        let active = self.active_tasks.lock().map_err(|_| {
            AppError::new(
                "OSS_TRANSFER_REGISTRY_UNAVAILABLE",
                "OSS transfer registry is unavailable",
            )
        })?;
        Ok(active.contains_key(operation_id))
    }

    fn active_token(&self, operation_id: &str) -> AppResult<Option<Arc<AtomicBool>>> {
        let active = self.active_tasks.lock().map_err(|_| {
            AppError::new(
                "OSS_TRANSFER_REGISTRY_UNAVAILABLE",
                "OSS transfer registry is unavailable",
            )
        })?;
        Ok(active.get(operation_id).cloned())
    }
}

fn new_transfer_task(
    id: String,
    account_id: String,
    target_id: String,
    operation: &str,
    object_key: String,
    local_path: PathBuf,
    total_bytes: u64,
    checkpoint_json: Option<String>,
) -> OssTransferTaskRecord {
    let timestamp = Utc::now().to_rfc3339();
    OssTransferTaskRecord {
        id,
        account_id,
        target_id,
        operation: operation.to_string(),
        object_key,
        local_path: local_path.display().to_string(),
        status: "queued".to_string(),
        bytes_completed: 0,
        total_bytes,
        upload_id: None,
        checkpoint_json,
        last_error_code: None,
        last_error_message: None,
        created_at: timestamp.clone(),
        updated_at: timestamp,
    }
}

fn validate_local_path(value: String, field: &str) -> AppResult<PathBuf> {
    let value = value.trim().to_string();
    if value.is_empty()
        || value.len() > 4 * 1024
        || value
            .chars()
            .any(|character| character == '\0' || character.is_control())
    {
        return Err(AppError::validation(format!("{field} is invalid")).with_detail("field", field));
    }
    let path = PathBuf::from(&value);
    if !path.is_absolute() {
        return Err(
            AppError::validation(format!("{field} must be absolute")).with_detail("field", field)
        );
    }
    Ok(path)
}

async fn file_snapshot(path: &Path) -> AppResult<FileSnapshot> {
    let path = path.to_path_buf();
    let metadata = timeout(FILE_IO_TIMEOUT, fs::metadata(&path))
        .await
        .map_err(|_| {
            AppError::new(
                "OSS_LOCAL_FILE_TIMEOUT",
                "local file metadata lookup timed out",
            )
            .with_detail("path", path.display().to_string())
            .retryable(true)
        })?
        .map_err(|error| {
            AppError::new("OSS_LOCAL_FILE_NOT_FOUND", "local file could not be read")
                .with_detail("path", path.display().to_string())
                .with_detail("reason", error.to_string())
        })?;
    if !metadata.is_file() {
        return Err(
            AppError::new("OSS_LOCAL_FILE_INVALID", "upload source must be a file")
                .with_detail("path", path.display().to_string()),
        );
    }
    let modified_millis = metadata
        .modified()
        .map_err(|error| {
            AppError::new(
                "OSS_LOCAL_FILE_INVALID",
                "failed to read local file modification time",
            )
            .with_detail("reason", error.to_string())
        })?
        .duration_since(UNIX_EPOCH)
        .map_err(|error| {
            AppError::new(
                "OSS_LOCAL_FILE_INVALID",
                "local file modification time is invalid",
            )
            .with_detail("reason", error.to_string())
        })?
        .as_millis();
    Ok(FileSnapshot {
        size: metadata.len(),
        modified_millis: u64::try_from(modified_millis).map_err(|_| {
            AppError::new(
                "OSS_LOCAL_FILE_INVALID",
                "local file modification time is out of range",
            )
        })?,
    })
}

async fn read_file_bytes(path: &Path) -> AppResult<Vec<u8>> {
    let path = path.to_path_buf();
    timeout(FILE_IO_TIMEOUT, fs::read(&path))
        .await
        .map_err(|_| {
            AppError::new("OSS_LOCAL_FILE_TIMEOUT", "local file read timed out")
                .with_detail("path", path.display().to_string())
                .retryable(true)
        })?
        .map_err(|error| {
            AppError::new(
                "OSS_LOCAL_FILE_READ_FAILED",
                "failed to read local upload file",
            )
            .with_detail("reason", error.to_string())
        })
}

async fn path_exists(path: &Path) -> AppResult<bool> {
    match timeout(FILE_IO_TIMEOUT, fs::metadata(path)).await {
        Ok(Ok(_)) => Ok(true),
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Ok(Err(error)) => Err(AppError::new(
            "OSS_LOCAL_FILE_CHECK_FAILED",
            "failed to inspect local path",
        )
        .with_detail("reason", error.to_string())),
        Err(_) => Err(
            AppError::new("OSS_LOCAL_FILE_TIMEOUT", "local path inspection timed out")
                .retryable(true),
        ),
    }
}

async fn file_size(path: &Path) -> AppResult<Option<u64>> {
    match timeout(FILE_IO_TIMEOUT, fs::metadata(path)).await {
        Ok(Ok(metadata)) => Ok(Some(metadata.len())),
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Ok(Err(error)) => Err(AppError::new(
            "OSS_LOCAL_FILE_CHECK_FAILED",
            "failed to inspect temporary download file",
        )
        .with_detail("reason", error.to_string())),
        Err(_) => Err(AppError::new(
            "OSS_LOCAL_FILE_TIMEOUT",
            "temporary download file inspection timed out",
        )
        .retryable(true)),
    }
}

fn validate_same_file(before: FileSnapshot, after: FileSnapshot) -> AppResult<()> {
    if before.size != after.size || before.modified_millis != after.modified_millis {
        return Err(AppError::new(
            "OSS_TRANSFER_SOURCE_CHANGED",
            "local file changed while it was being uploaded",
        ));
    }
    Ok(())
}

fn calculate_part_size(total_bytes: u64) -> u64 {
    DEFAULT_PART_SIZE.max(if total_bytes == 0 {
        DEFAULT_PART_SIZE
    } else {
        total_bytes.div_ceil(MAX_PART_COUNT)
    })
}

fn part_count(total_bytes: u64, part_size: u64) -> AppResult<u64> {
    if part_size == 0 {
        return Err(AppError::new(
            "OSS_MULTIPART_INCOMPLETE",
            "multipart part size is invalid",
        ));
    }
    let count = if total_bytes == 0 {
        0
    } else {
        total_bytes.div_ceil(part_size)
    };
    if count > MAX_PART_COUNT {
        return Err(AppError::new(
            "OSS_MULTIPART_INCOMPLETE",
            "file requires more than 10,000 multipart parts",
        ));
    }
    Ok(count)
}

fn part_size_for(total_bytes: u64, part_size: u64, part_number: u64) -> u64 {
    let offset = (part_number - 1) * part_size;
    (total_bytes - offset).min(part_size)
}

fn validate_upload_checkpoint(
    checkpoint: &UploadCheckpoint,
    snapshot: &FileSnapshot,
) -> AppResult<()> {
    if checkpoint.kind != "upload"
        || checkpoint.file_size != snapshot.size
        || checkpoint.modified_millis != snapshot.modified_millis
        || checkpoint.part_size == 0
    {
        return Err(AppError::new(
            "OSS_TRANSFER_SOURCE_CHANGED",
            "local file or upload checkpoint no longer matches",
        ));
    }
    let expected_count = part_count(snapshot.size, checkpoint.part_size)?;
    let mut seen = std::collections::HashSet::new();
    for part in &checkpoint.parts {
        let part_number = u64::from(part.part_number);
        if part.part_number == 0
            || part_number > expected_count
            || part.etag.is_empty()
            || !seen.insert(part.part_number)
            || part.size != part_size_for(snapshot.size, checkpoint.part_size, part_number)
        {
            return Err(AppError::new(
                "OSS_TRANSFER_CHECKPOINT_INVALID",
                "upload checkpoint contains invalid part metadata",
            ));
        }
    }
    Ok(())
}

fn serialize_checkpoint<T: Serialize>(value: &T) -> AppResult<String> {
    serde_json::to_string(value).map_err(|error| {
        AppError::new(
            "OSS_TRANSFER_CHECKPOINT_INVALID",
            "failed to serialize OSS transfer checkpoint",
        )
        .with_detail("reason", error.to_string())
    })
}

fn parse_checkpoint<T: for<'de> Deserialize<'de>>(value: Option<&str>) -> AppResult<T> {
    let value = value.ok_or_else(|| {
        AppError::new(
            "OSS_TRANSFER_CHECKPOINT_INVALID",
            "OSS transfer checkpoint is missing",
        )
    })?;
    serde_json::from_str(value).map_err(|error| {
        AppError::new(
            "OSS_TRANSFER_CHECKPOINT_INVALID",
            "OSS transfer checkpoint is invalid",
        )
        .with_detail("reason", error.to_string())
    })
}

fn check_cancel(cancel_token: &AtomicBool) -> AppResult<()> {
    if cancel_token.load(Ordering::SeqCst) {
        Err(AppError::new(
            "OSS_TRANSFER_CANCELLED",
            "OSS transfer cancellation requested",
        ))
    } else {
        Ok(())
    }
}

async fn await_cancelable<T, F>(cancel_token: &AtomicBool, operation: F) -> AppResult<T>
where
    F: Future<Output = AppResult<T>>,
{
    tokio::select! {
        result = operation => result,
        _ = wait_for_cancel(cancel_token) => Err(AppError::new(
            "OSS_TRANSFER_CANCELLED",
            "OSS transfer cancellation requested",
        )),
    }
}

async fn wait_for_cancel(cancel_token: &AtomicBool) {
    while !cancel_token.load(Ordering::SeqCst) {
        sleep(Duration::from_millis(100)).await;
    }
}

fn ensure_key_within_target(
    target: &crate::domain::OssTargetRecord,
    object_key: &str,
) -> AppResult<()> {
    if !target.prefix.is_empty() && !object_key.starts_with(&target.prefix) {
        return Err(AppError::new(
            "OSS_OBJECT_OUT_OF_SCOPE",
            "requested object is outside the configured OSS target",
        )
        .with_detail("targetId", target.id.clone()));
    }
    Ok(())
}

async fn ensure_parent_directory(path: &Path) -> AppResult<()> {
    let parent = path.parent().ok_or_else(|| {
        AppError::new(
            "OSS_LOCAL_DIRECTORY_NOT_FOUND",
            "local destination directory does not exist",
        )
    })?;
    let metadata = timeout(FILE_IO_TIMEOUT, fs::metadata(parent))
        .await
        .map_err(|_| {
            AppError::new(
                "OSS_LOCAL_FILE_TIMEOUT",
                "local destination directory lookup timed out",
            )
            .retryable(true)
        })?
        .map_err(|error| {
            AppError::new(
                "OSS_LOCAL_DIRECTORY_NOT_FOUND",
                "local destination directory does not exist",
            )
            .with_detail("reason", error.to_string())
        })?;
    if !metadata.is_dir() {
        return Err(AppError::new(
            "OSS_LOCAL_DIRECTORY_NOT_FOUND",
            "local destination path is not a directory",
        ));
    }
    Ok(())
}

fn download_temp_path(task: &OssTransferTaskRecord) -> AppResult<PathBuf> {
    let destination = Path::new(&task.local_path);
    let parent = destination.parent().ok_or_else(|| {
        AppError::new(
            "OSS_LOCAL_FILE_INVALID",
            "download destination has no parent directory",
        )
    })?;
    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::new(
                "OSS_LOCAL_FILE_INVALID",
                "download destination file name is invalid",
            )
        })?;
    Ok(parent.join(format!(".{file_name}.bexo-{}.part", task.id)))
}

async fn finalize_download_file(
    temp_path: &Path,
    destination: &Path,
    overwrite: bool,
) -> AppResult<()> {
    ensure_parent_directory(destination).await?;
    if path_exists(destination).await? {
        if !overwrite {
            return Err(AppError::new(
                "OSS_DOWNLOAD_CONFLICT",
                "the local destination already exists",
            ));
        }
        timeout(FILE_IO_TIMEOUT, fs::remove_file(destination))
            .await
            .map_err(|_| {
                AppError::new(
                    "OSS_LOCAL_FILE_TIMEOUT",
                    "removing the existing download destination timed out",
                )
                .retryable(true)
            })?
            .map_err(|error| {
                AppError::new(
                    "OSS_LOCAL_FILE_WRITE_FAILED",
                    "failed to replace the existing download destination",
                )
                .with_detail("reason", error.to_string())
            })?;
    }
    timeout(FILE_IO_TIMEOUT, fs::rename(temp_path, destination))
        .await
        .map_err(|_| {
            AppError::new(
                "OSS_LOCAL_FILE_TIMEOUT",
                "finalizing the downloaded file timed out",
            )
            .retryable(true)
        })?
        .map_err(|error| {
            AppError::new(
                "OSS_LOCAL_FILE_WRITE_FAILED",
                "failed to finalize the downloaded file",
            )
            .with_detail("reason", error.to_string())
        })
}

async fn remove_file_if_exists(path: &Path) -> AppResult<()> {
    match timeout(FILE_IO_TIMEOUT, fs::remove_file(path)).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(Err(error)) => Err(AppError::new(
            "OSS_LOCAL_FILE_CLEANUP_FAILED",
            "failed to remove temporary transfer file",
        )
        .with_detail("reason", error.to_string())),
        Err(_) => Err(AppError::new(
            "OSS_LOCAL_FILE_TIMEOUT",
            "temporary transfer file cleanup timed out",
        )
        .retryable(true)),
    }
}

fn content_type_for_path(path: &Path) -> Option<String> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    let content_type = match extension.as_str() {
        "txt" | "log" | "md" => "text/plain",
        "json" => "application/json",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" | "mjs" => "text/javascript",
        "ts" | "tsx" => "text/typescript",
        "xml" => "application/xml",
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "zip" => "application/zip",
        _ => return None,
    };
    Some(content_type.to_string())
}

fn emit_transfer_event(app_handle: &AppHandle, task: &OssTransferTaskView) {
    if let Err(error) = app_handle.emit(OSS_TRANSFER_PROGRESS_EVENT_NAME, task.clone()) {
        log::error!(
            target: "bexo::service::oss-transfer",
            "failed to emit OSS transfer progress task_id={} reason={}",
            task.task.id,
            error
        );
    }
    if let Err(error) = app_handle.emit(OSS_TRANSFER_STATE_EVENT_NAME, task.clone()) {
        log::error!(
            target: "bexo::service::oss-transfer",
            "failed to emit OSS transfer state task_id={} reason={}",
            task.task.id,
            error
        );
    }
}

fn unknown_upload_cancellation_error() -> AppError {
    unknown_upload_result_error(AppError::new(
        "OSS_TRANSFER_CANCELLED",
        "upload cancellation happened while the remote result was still unknown",
    ))
}

fn transfer_retry_delay(attempt: u32) -> Duration {
    Duration::from_millis(250_u64.saturating_mul(1_u64 << attempt.min(4)))
}

fn is_unknown_remote_result(error: &AppError) -> bool {
    matches!(
        error.code.as_str(),
        "OSS_NETWORK_TIMEOUT" | "OSS_NETWORK_ERROR" | "OSS_REMOTE_RETRYABLE"
    ) || error.code.starts_with("OSS_REMOTE_")
}

fn unknown_upload_result_error(error: AppError) -> AppError {
    AppError::new(
        "OSS_TRANSFER_NEEDS_CONFIRMATION",
        "OSS upload result is unknown; confirm the remote object before retrying",
    )
    .with_detail("cause", error.code)
}

#[cfg(test)]
mod tests {
    use super::{
        calculate_part_size, part_count, part_size_for, validate_local_path, FileSnapshot,
        UploadCheckpoint,
    };
    use crate::domain::OssTransferTaskRecord;

    #[test]
    fn multipart_part_size_stays_within_oss_part_limit() {
        assert_eq!(calculate_part_size(0), 4 * 1024 * 1024);
        assert_eq!(part_count(32 * 1024 * 1024, 16 * 1024 * 1024).unwrap(), 2);
        assert_eq!(
            part_size_for(17 * 1024 * 1024, 16 * 1024 * 1024, 2),
            1024 * 1024
        );
    }

    #[test]
    fn local_transfer_paths_must_be_absolute() {
        assert!(validate_local_path("relative.txt".into(), "localPath").is_err());
        assert!(validate_local_path(r"D:\downloads\file.txt".into(), "localPath").is_ok());
    }

    #[test]
    fn upload_checkpoint_serializes_without_secrets() {
        let checkpoint = UploadCheckpoint {
            kind: "upload".into(),
            file_size: 10,
            modified_millis: 20,
            part_size: 16,
            overwrite: false,
            parts: Vec::new(),
        };
        let json = super::serialize_checkpoint(&checkpoint).unwrap();
        assert!(json.contains("\"partSize\""));
        assert!(!json.to_ascii_lowercase().contains("secret"));
        let _snapshot = FileSnapshot {
            size: 10,
            modified_millis: 20,
        };
    }

    #[test]
    fn transfer_events_do_not_serialize_internal_multipart_state() {
        let task = OssTransferTaskRecord {
            id: "550e8400-e29b-41d4-a716-446655440000".into(),
            account_id: "550e8400-e29b-41d4-a716-446655440001".into(),
            target_id: "550e8400-e29b-41d4-a716-446655440002".into(),
            operation: "upload".into(),
            object_key: "assets/file.bin".into(),
            local_path: r"D:\uploads\file.bin".into(),
            status: "running".into(),
            bytes_completed: 16,
            total_bytes: 32,
            upload_id: Some("upload-id-is-internal".into()),
            checkpoint_json: Some("{\"partNumber\":1}".into()),
            last_error_code: None,
            last_error_message: None,
            created_at: "2026-07-12T00:00:00Z".into(),
            updated_at: "2026-07-12T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&task).unwrap();
        assert!(!json.contains("uploadId"));
        assert!(!json.contains("checkpointJson"));
        assert!(!json.contains("upload-id-is-internal"));
    }
}
