use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use chrono::Utc;
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use uuid::Uuid;

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

use crate::{
    domain::{
        build_prompt_import_preview, validate_prompt_transfer_document, ApplyPromptListImportInput,
        ExportPromptListInput, ExportPromptListResult, PreviewPromptListImportInput,
        PromptImportApplyResult, PromptImportPreview, PromptTransferDocument, PromptTransferPrompt,
        PROMPT_TRANSFER_FORMAT, PROMPT_TRANSFER_MAX_FILE_BYTES, PROMPT_TRANSFER_SCHEMA_VERSION,
    },
    error::{AppError, AppResult},
    persistence::{apply_prompt_import, list_prompts, Database},
};

const PROMPT_TRANSFER_FILE_IO_TIMEOUT: Duration = Duration::from_secs(10);
const PROMPT_TRANSFER_ANALYSIS_TIMEOUT: Duration = Duration::from_secs(10);

#[cfg(windows)]
const MOVE_FILE_REPLACE_EXISTING: u32 = 0x0000_0001;
#[cfg(windows)]
const MOVE_FILE_WRITE_THROUGH: u32 = 0x0000_0008;

#[cfg(windows)]
#[link(name = "Kernel32")]
unsafe extern "system" {
    fn MoveFileExW(existing_file_name: *const u16, new_file_name: *const u16, flags: u32) -> i32;
}

#[derive(Debug, Clone)]
pub struct PromptTransferService {
    database: Database,
    import_lock: Arc<Mutex<()>>,
}

#[derive(Debug)]
struct LoadedPromptTransferFile {
    document: PromptTransferDocument,
    file_hash: String,
}

impl PromptTransferService {
    pub fn new(database: Database) -> Self {
        Self {
            database,
            import_lock: Arc::new(Mutex::new(())),
        }
    }

    pub async fn export_prompt_list(
        &self,
        input: ExportPromptListInput,
    ) -> AppResult<ExportPromptListResult> {
        let destination_path = validate_destination_path(&input.destination_path)?;
        let prompts = self
            .database
            .read("collect_prompt_transfer_export", list_prompts)
            .await?;
        let prompt_count = prompts.len();
        let document = PromptTransferDocument {
            format: PROMPT_TRANSFER_FORMAT.to_string(),
            schema_version: PROMPT_TRANSFER_SCHEMA_VERSION,
            exported_at: Utc::now().to_rfc3339(),
            prompts: prompts
                .into_iter()
                .enumerate()
                .map(|(index, prompt)| PromptTransferPrompt {
                    id: prompt.id,
                    title: prompt.title,
                    content: prompt.content,
                    sort_order: index as i64,
                })
                .collect(),
        };

        run_blocking_with_timeout(
            "export_prompt_list",
            PROMPT_TRANSFER_FILE_IO_TIMEOUT,
            move |cancelled| {
                check_cancelled(&cancelled)?;
                let document = validate_prompt_transfer_document(document)?;
                let mut bytes = serde_json::to_vec_pretty(&document).map_err(|error| {
                    AppError::new("PROMPT_TRANSFER_WRITE_FAILED", "生成 Prompts JSON 失败")
                        .with_detail("reason", error.to_string())
                })?;
                bytes.push(b'\n');
                if bytes.len() as u64 > PROMPT_TRANSFER_MAX_FILE_BYTES {
                    return Err(file_too_large_error(bytes.len() as u64));
                }
                let file_hash = sha256_hex(&bytes);
                let file_size_bytes = bytes.len() as u64;
                write_prompt_transfer_file(&destination_path, &bytes, &cancelled)?;
                Ok(ExportPromptListResult {
                    prompt_count,
                    file_size_bytes,
                    file_hash,
                })
            },
        )
        .await
    }

    pub async fn preview_prompt_list_import(
        &self,
        input: PreviewPromptListImportInput,
    ) -> AppResult<PromptImportPreview> {
        let source_path = validate_source_path(&input.source_path)?;
        let source_path_text = source_path.display().to_string();
        let loaded = load_prompt_transfer_file(source_path).await?;
        let existing_prompts = self
            .database
            .read("load_prompt_import_existing_state", list_prompts)
            .await?;
        run_blocking_with_timeout(
            "analyze_prompt_list_import",
            PROMPT_TRANSFER_ANALYSIS_TIMEOUT,
            move |cancelled| {
                check_cancelled(&cancelled)?;
                Ok(build_prompt_import_preview(
                    source_path_text,
                    loaded.file_hash,
                    &loaded.document,
                    &existing_prompts,
                ))
            },
        )
        .await
    }

    pub async fn apply_prompt_list_import(
        &self,
        input: ApplyPromptListImportInput,
    ) -> AppResult<PromptImportApplyResult> {
        let _guard = self.import_lock.lock().await;
        validate_expected_hash(&input.expected_file_hash)?;
        let source_path = validate_source_path(&input.source_path)?;
        let loaded = load_prompt_transfer_file(source_path).await?;
        let expected_hash = input.expected_file_hash.to_ascii_lowercase();
        if loaded.file_hash != expected_hash {
            return Err(AppError::new(
                "PROMPT_TRANSFER_FILE_CHANGED",
                "Prompts 文件在预检后已发生变化，请重新预检",
            )
            .with_detail("expectedHash", expected_hash)
            .with_detail("actualHash", loaded.file_hash));
        }

        let prompts = loaded.document.prompts;
        let selections = input.selections;
        self.database
            .write("apply_prompt_list_import", move |connection| {
                apply_prompt_import(connection, prompts, selections)
            })
            .await
    }
}

async fn load_prompt_transfer_file(path: PathBuf) -> AppResult<LoadedPromptTransferFile> {
    run_blocking_with_timeout(
        "read_prompt_transfer_file",
        PROMPT_TRANSFER_FILE_IO_TIMEOUT,
        move |cancelled| {
            let bytes = read_prompt_transfer_file(&path, &cancelled)?;
            check_cancelled(&cancelled)?;
            let file_hash = sha256_hex(&bytes);
            let document = parse_prompt_transfer_document(&bytes)?;
            Ok(LoadedPromptTransferFile {
                document,
                file_hash,
            })
        },
    )
    .await
}

fn parse_prompt_transfer_document(bytes: &[u8]) -> AppResult<PromptTransferDocument> {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let document = serde_json::from_slice::<PromptTransferDocument>(bytes).map_err(|error| {
        AppError::new("PROMPT_TRANSFER_PARSE_FAILED", "无法解析 Prompts JSON")
            .with_detail("line", error.line().to_string())
            .with_detail("column", error.column().to_string())
            .with_detail("reason", error.to_string())
    })?;
    validate_prompt_transfer_document(document)
}

fn validate_source_path(value: &str) -> AppResult<PathBuf> {
    validate_absolute_path(value)
}

fn validate_destination_path(value: &str) -> AppResult<PathBuf> {
    let path = validate_absolute_path(value)?;
    if path.parent().is_none() {
        return Err(AppError::new(
            "PROMPT_TRANSFER_PATH_INVALID",
            "导出文件路径缺少父目录",
        ));
    }
    if path.file_name().is_none() {
        return Err(AppError::new(
            "PROMPT_TRANSFER_PATH_INVALID",
            "导出文件名不能为空",
        ));
    }
    Ok(path)
}

fn validate_absolute_path(value: &str) -> AppResult<PathBuf> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::new(
            "PROMPT_TRANSFER_PATH_INVALID",
            "文件路径不能为空",
        ));
    }
    let path = PathBuf::from(trimmed);
    if !path.is_absolute() {
        return Err(
            AppError::new("PROMPT_TRANSFER_PATH_INVALID", "文件路径必须是绝对路径")
                .with_detail("path", trimmed.to_string()),
        );
    }
    Ok(path)
}

fn validate_expected_hash(value: &str) -> AppResult<()> {
    let trimmed = value.trim();
    if trimmed.len() != 64
        || !trimmed
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(AppError::new(
            "PROMPT_TRANSFER_SELECTION_INVALID",
            "预检文件哈希无效，请重新预检",
        ));
    }
    Ok(())
}

fn read_prompt_transfer_file(path: &Path, cancelled: &AtomicBool) -> AppResult<Vec<u8>> {
    check_cancelled(cancelled)?;
    let metadata = fs::metadata(path).map_err(|error| {
        AppError::new("PROMPT_TRANSFER_PATH_INVALID", "导入文件不存在或无法读取")
            .with_detail("path", path.display().to_string())
            .with_detail("reason", error.to_string())
    })?;
    if !metadata.is_file() {
        return Err(
            AppError::new("PROMPT_TRANSFER_PATH_INVALID", "导入路径必须指向文件")
                .with_detail("path", path.display().to_string()),
        );
    }
    if metadata.len() > PROMPT_TRANSFER_MAX_FILE_BYTES {
        return Err(file_too_large_error(metadata.len()));
    }

    let mut file = File::open(path).map_err(|error| {
        AppError::new("PROMPT_TRANSFER_READ_FAILED", "打开 Prompts 文件失败")
            .with_detail("path", path.display().to_string())
            .with_detail("reason", error.to_string())
    })?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        check_cancelled(cancelled)?;
        let read_count = file.read(&mut buffer).map_err(|error| {
            AppError::new("PROMPT_TRANSFER_READ_FAILED", "读取 Prompts 文件失败")
                .with_detail("path", path.display().to_string())
                .with_detail("reason", error.to_string())
        })?;
        if read_count == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read_count]);
        if bytes.len() as u64 > PROMPT_TRANSFER_MAX_FILE_BYTES {
            return Err(file_too_large_error(bytes.len() as u64));
        }
    }
    Ok(bytes)
}

fn write_prompt_transfer_file(
    destination_path: &Path,
    bytes: &[u8],
    cancelled: &AtomicBool,
) -> AppResult<()> {
    check_cancelled(cancelled)?;
    let parent = destination_path
        .parent()
        .ok_or_else(|| AppError::new("PROMPT_TRANSFER_PATH_INVALID", "导出文件路径缺少父目录"))?;
    let parent_metadata = fs::metadata(parent).map_err(|error| {
        AppError::new(
            "PROMPT_TRANSFER_PATH_INVALID",
            "导出文件父目录不存在或无法访问",
        )
        .with_detail("path", parent.display().to_string())
        .with_detail("reason", error.to_string())
    })?;
    if !parent_metadata.is_dir() {
        return Err(
            AppError::new("PROMPT_TRANSFER_PATH_INVALID", "导出文件父路径不是目录")
                .with_detail("path", parent.display().to_string()),
        );
    }
    if fs::metadata(destination_path)
        .map(|metadata| metadata.is_dir())
        .unwrap_or(false)
    {
        return Err(
            AppError::new("PROMPT_TRANSFER_PATH_INVALID", "导出路径不能指向目录")
                .with_detail("path", destination_path.display().to_string()),
        );
    }

    let file_name = destination_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("bexo-prompts.json");
    let temp_path = parent.join(format!(".{file_name}.tmp.{}", Uuid::new_v4()));
    let write_result = (|| -> AppResult<()> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .map_err(|error| {
                AppError::new(
                    "PROMPT_TRANSFER_WRITE_FAILED",
                    "创建 Prompts 导出临时文件失败",
                )
                .with_detail("path", temp_path.display().to_string())
                .with_detail("reason", error.to_string())
            })?;
        for chunk in bytes.chunks(64 * 1024) {
            check_cancelled(cancelled)?;
            file.write_all(chunk).map_err(|error| {
                AppError::new(
                    "PROMPT_TRANSFER_WRITE_FAILED",
                    "写入 Prompts 导出临时文件失败",
                )
                .with_detail("path", temp_path.display().to_string())
                .with_detail("reason", error.to_string())
            })?;
        }
        file.flush().map_err(|error| {
            AppError::new(
                "PROMPT_TRANSFER_WRITE_FAILED",
                "刷新 Prompts 导出临时文件失败",
            )
            .with_detail("reason", error.to_string())
        })?;
        file.sync_all().map_err(|error| {
            AppError::new(
                "PROMPT_TRANSFER_WRITE_FAILED",
                "持久化 Prompts 导出临时文件失败",
            )
            .with_detail("reason", error.to_string())
        })?;
        drop(file);
        check_cancelled(cancelled)?;
        replace_file_from_temp(&temp_path, destination_path).map_err(|error| {
            AppError::new(
                "PROMPT_TRANSFER_WRITE_FAILED",
                "原子替换 Prompts 导出文件失败",
            )
            .with_detail("path", destination_path.display().to_string())
            .with_detail("reason", error.to_string())
        })?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    write_result
}

#[cfg(windows)]
fn replace_file_from_temp(temp_path: &Path, destination_path: &Path) -> std::io::Result<()> {
    let temp_wide = temp_path
        .as_os_str()
        .encode_wide()
        .chain([0])
        .collect::<Vec<_>>();
    let destination_wide = destination_path
        .as_os_str()
        .encode_wide()
        .chain([0])
        .collect::<Vec<_>>();
    let result = unsafe {
        MoveFileExW(
            temp_wide.as_ptr(),
            destination_wide.as_ptr(),
            MOVE_FILE_REPLACE_EXISTING | MOVE_FILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file_from_temp(temp_path: &Path, destination_path: &Path) -> std::io::Result<()> {
    fs::rename(temp_path, destination_path)
}

async fn run_blocking_with_timeout<T, F>(
    operation_name: &'static str,
    timeout: Duration,
    operation: F,
) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce(Arc<AtomicBool>) -> AppResult<T> + Send + 'static,
{
    let cancelled = Arc::new(AtomicBool::new(false));
    let worker_cancelled = Arc::clone(&cancelled);
    let mut handle = tauri::async_runtime::spawn_blocking(move || operation(worker_cancelled));
    match tokio::time::timeout(timeout, &mut handle).await {
        Ok(joined) => joined.map_err(|error| {
            AppError::new("PROMPT_TRANSFER_TIMEOUT", "Prompts 本地文件操作异常终止")
                .with_detail("operation", operation_name)
                .with_detail("reason", error.to_string())
        })?,
        Err(_) => {
            cancelled.store(true, Ordering::Release);
            match handle.await {
                Ok(Ok(value)) => Ok(value),
                Ok(Err(error)) if error.code != "PROMPT_TRANSFER_CANCELLED" => Err(error),
                Ok(Err(_)) => Err(AppError::new(
                    "PROMPT_TRANSFER_TIMEOUT",
                    "Prompts 本地文件操作超时",
                )
                .with_detail("operation", operation_name)
                .retryable(true)),
                Err(error) => Err(AppError::new(
                    "PROMPT_TRANSFER_TIMEOUT",
                    "Prompts 本地文件操作超时且回收失败",
                )
                .with_detail("operation", operation_name)
                .with_detail("reason", error.to_string())),
            }
        }
    }
}

fn check_cancelled(cancelled: &AtomicBool) -> AppResult<()> {
    if cancelled.load(Ordering::Acquire) {
        return Err(AppError::new(
            "PROMPT_TRANSFER_CANCELLED",
            "Prompts 文件操作已取消",
        ));
    }
    Ok(())
}

fn file_too_large_error(size: u64) -> AppError {
    AppError::new("PROMPT_TRANSFER_TOO_LARGE", "Prompts 文件超过 64 MiB 限制")
        .with_detail("sizeBytes", size.to_string())
        .with_detail("limitBytes", PROMPT_TRANSFER_MAX_FILE_BYTES.to_string())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_accepts_utf8_bom_and_rejects_unknown_fields() {
        let id = Uuid::new_v4();
        let json = format!(
            "{{\"format\":\"{}\",\"schemaVersion\":1,\"exportedAt\":\"2026-09-02T00:00:00Z\",\"prompts\":[{{\"id\":\"{}\",\"title\":\"标题\",\"content\":\"正文\",\"sortOrder\":0}}]}}",
            PROMPT_TRANSFER_FORMAT, id
        );
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(json.as_bytes());
        let document = parse_prompt_transfer_document(&bytes).expect("parse BOM document");
        assert_eq!(document.prompts[0].id, id.to_string());

        let unknown = json.replace("\"sortOrder\":0", "\"sortOrder\":0,\"secret\":true");
        let error = parse_prompt_transfer_document(unknown.as_bytes())
            .expect_err("unknown fields must be rejected");
        assert_eq!(error.code, "PROMPT_TRANSFER_PARSE_FAILED");
    }

    #[test]
    fn atomic_writer_round_trips_and_leaves_no_temp_file() {
        let directory = std::env::temp_dir().join(format!("bexo-prompts-{}", Uuid::new_v4()));
        fs::create_dir_all(&directory).expect("create temp directory");
        let destination = directory.join("prompts.json");
        let cancelled = AtomicBool::new(false);
        write_prompt_transfer_file(&destination, b"first", &cancelled).expect("write first");
        write_prompt_transfer_file(&destination, b"second", &cancelled).expect("replace second");
        assert_eq!(fs::read(&destination).expect("read export"), b"second");
        let entries = fs::read_dir(&directory)
            .expect("read temp directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect entries");
        assert_eq!(entries.len(), 1);
        fs::remove_dir_all(directory).expect("remove temp directory");
    }
}
