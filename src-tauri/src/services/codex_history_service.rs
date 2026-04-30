use std::{
    collections::BTreeSet,
    env,
    fs::{self, File},
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use chrono::{DateTime, TimeZone, Utc};
use serde_json::Value;
use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindowBuilder};

use crate::{
    domain::{
        ensure_absolute_directory, validate_optional_uuid, CodexHistoryGlobalSessionsResponse,
        CodexHistoryMessage, CodexHistoryMessagesInput, CodexHistoryMessagesPage,
        CodexHistorySession, CodexHistorySessionsResponse, ListCodexHistorySessionsInput,
        OpenCodexHistoryWindowResult,
    },
    error::{AppError, AppResult},
    persistence::{get_workspace_primary_project_path, list_codex_profiles, Database},
};

const CODEX_PROVIDER_ID: &str = "codex";
const HISTORY_WINDOW_PREFIX: &str = "codex_history_";
const DEFAULT_MESSAGE_LIMIT: usize = 40;
const MAX_MESSAGE_LIMIT: usize = 120;
const MAX_MESSAGE_CHARS: usize = 12_000;
const TITLE_MAX_CHARS: usize = 80;
const SUMMARY_MAX_CHARS: usize = 160;
const HEAD_LINE_COUNT: usize = 16;
const TAIL_LINE_COUNT: usize = 48;
const TAIL_READ_BYTES: u64 = 64 * 1024;
const MESSAGE_READ_CHUNK_BYTES: u64 = 64 * 1024;
const MESSAGE_MAX_SCAN_BYTES: u64 = 64 * 1024 * 1024;
const MESSAGE_MAX_CARRY_BYTES: usize = 2 * 1024 * 1024;
const SCAN_TIMEOUT: Duration = Duration::from_secs(15);
const MESSAGE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct CodexHistoryService {
    database: Database,
}

#[derive(Debug, Clone)]
struct WorkspaceHistoryContext {
    workspace_id: String,
    workspace_path: String,
    workspace_compare_path: String,
}

#[derive(Debug, Clone)]
struct ParsedSession {
    session: CodexHistorySession,
    sort_ms: i64,
}

#[derive(Debug, Clone)]
struct TimestampValue {
    ms: i64,
    display: String,
}

impl CodexHistoryService {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub async fn open_history_window<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        workspace_id: String,
    ) -> AppResult<OpenCodexHistoryWindowResult> {
        let context = self.resolve_workspace_context(workspace_id).await?;
        let window_label = history_window_label(&context.workspace_id);

        if let Some(window) = app.get_webview_window(&window_label) {
            window.show().map_err(|error| {
                AppError::new(
                    "CODEX_HISTORY_WINDOW_FAILED",
                    "failed to show codex history window",
                )
                .with_detail("workspaceId", context.workspace_id.clone())
                .with_detail("reason", error.to_string())
            })?;
            window.set_focus().map_err(|error| {
                AppError::new(
                    "CODEX_HISTORY_WINDOW_FAILED",
                    "failed to focus codex history window",
                )
                .with_detail("workspaceId", context.workspace_id.clone())
                .with_detail("reason", error.to_string())
            })?;
            return Ok(OpenCodexHistoryWindowResult {
                workspace_id: context.workspace_id,
                window_label,
            });
        }

        let title = format!(
            "Codex History - {}",
            path_basename(&context.workspace_path).unwrap_or_else(|| "Workspace".to_string())
        );
        let url = format!(
            "index.html?window=codex-history&workspaceId={}",
            context.workspace_id
        );

        WebviewWindowBuilder::new(app, &window_label, WebviewUrl::App(url.into()))
            .title(title)
            .inner_size(1180.0, 780.0)
            .min_inner_size(920.0, 620.0)
            .resizable(true)
            .decorations(true)
            .visible(true)
            .build()
            .map_err(|error| {
                AppError::new(
                    "CODEX_HISTORY_WINDOW_FAILED",
                    "failed to create codex history window",
                )
                .with_detail("workspaceId", context.workspace_id.clone())
                .with_detail("reason", error.to_string())
            })?;

        Ok(OpenCodexHistoryWindowResult {
            workspace_id: context.workspace_id,
            window_label,
        })
    }

    pub async fn list_sessions<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        input: ListCodexHistorySessionsInput,
    ) -> AppResult<CodexHistorySessionsResponse> {
        let context = self.resolve_workspace_context(input.workspace_id).await?;
        let roots = self.resolve_existing_codex_session_roots(app).await?;
        let root_labels = roots
            .iter()
            .map(|root| display_path(root.as_path()))
            .collect::<Vec<_>>();
        let scan_context = context.clone();

        let sessions = run_blocking_with_timeout(
            "scan_codex_history_sessions",
            SCAN_TIMEOUT,
            move |cancelled| scan_sessions_for_workspace(&roots, &scan_context, &cancelled),
        )
        .await?;

        Ok(CodexHistorySessionsResponse {
            workspace_id: context.workspace_id,
            workspace_path: context.workspace_path,
            codex_roots: root_labels,
            sessions,
        })
    }

    pub async fn list_all_sessions<R: Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> AppResult<CodexHistoryGlobalSessionsResponse> {
        let roots = self.resolve_existing_codex_session_roots(app).await?;
        let root_labels = roots
            .iter()
            .map(|root| display_path(root.as_path()))
            .collect::<Vec<_>>();

        let sessions = run_blocking_with_timeout(
            "scan_all_codex_history_sessions",
            SCAN_TIMEOUT,
            move |cancelled| scan_all_sessions(&roots, &cancelled),
        )
        .await?;

        Ok(CodexHistoryGlobalSessionsResponse {
            codex_roots: root_labels,
            sessions,
        })
    }

    pub async fn get_messages<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        input: CodexHistoryMessagesInput,
    ) -> AppResult<CodexHistoryMessagesPage> {
        let context = match input.workspace_id {
            Some(workspace_id) if !workspace_id.trim().is_empty() => {
                Some(self.resolve_workspace_context(workspace_id).await?)
            }
            _ => None,
        };
        let roots = self.resolve_existing_codex_session_roots(app).await?;
        let limit = input
            .limit
            .unwrap_or(DEFAULT_MESSAGE_LIMIT)
            .clamp(1, MAX_MESSAGE_LIMIT);
        let source_path = input.source_path;
        let cursor = input.cursor;

        run_blocking_with_timeout(
            "get_codex_history_messages",
            MESSAGE_TIMEOUT,
            move |cancelled| {
                check_cancelled(&cancelled)?;
                let source = validate_source_path(&source_path, &roots)?;
                if let Some(context) = context {
                    let parsed = parse_session(&source)?;
                    let project_dir = parsed.session.project_dir.as_deref().ok_or_else(|| {
                        AppError::new(
                            "CODEX_HISTORY_WORKSPACE_MISMATCH",
                            "codex session has no project directory",
                        )
                        .with_detail("sourcePath", source.display().to_string())
                    })?;
                    if !path_is_under_workspace(project_dir, &context.workspace_compare_path) {
                        return Err(AppError::new(
                            "CODEX_HISTORY_WORKSPACE_MISMATCH",
                            "codex session does not belong to this workspace",
                        )
                        .with_detail("workspaceId", context.workspace_id)
                        .with_detail("workspacePath", context.workspace_path)
                        .with_detail("projectDir", project_dir.to_string()));
                    }
                }

                read_messages_page_from_tail(&source, cursor.as_deref(), limit, &cancelled)
            },
        )
        .await
    }

    async fn resolve_workspace_context(
        &self,
        workspace_id: String,
    ) -> AppResult<WorkspaceHistoryContext> {
        let workspace_id = validate_optional_uuid("workspaceId", Some(workspace_id))?
            .ok_or_else(|| AppError::validation("workspaceId is required"))?;
        let workspace_id_for_query = workspace_id.clone();
        let workspace_path = self
            .database
            .read("get_codex_history_workspace_path", move |connection| {
                get_workspace_primary_project_path(connection, workspace_id_for_query)
            })
            .await?;
        let workspace_path = ensure_absolute_directory(&workspace_path, "INVALID_WORKSPACE_PATH")?;
        let workspace_compare_path = normalize_compare_path(&workspace_path);

        Ok(WorkspaceHistoryContext {
            workspace_id,
            workspace_path,
            workspace_compare_path,
        })
    }

    async fn resolve_existing_codex_session_roots<R: Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> AppResult<Vec<PathBuf>> {
        let profiles = self
            .database
            .read("list_codex_profiles_for_history_roots", list_codex_profiles)
            .await?;

        let mut candidates = Vec::new();
        if let Some(configured_home) = env::var("CODEX_HOME")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        {
            candidates.push(PathBuf::from(configured_home).join("sessions"));
        } else if let Ok(home_dir) = app.path().home_dir() {
            candidates.push(home_dir.join(".codex").join("sessions"));
        }

        for profile in profiles {
            if !profile.codex_home.trim().is_empty() {
                candidates.push(PathBuf::from(profile.codex_home).join("sessions"));
            }
        }

        let mut seen = BTreeSet::new();
        let mut roots = Vec::new();
        for candidate in candidates {
            if !candidate.exists() || !candidate.is_dir() {
                continue;
            }
            let canonical = match candidate.canonicalize() {
                Ok(path) => path,
                Err(error) => {
                    log::warn!(
                        target: "bexo::service::codex_history",
                        "failed to canonicalize codex sessions root path={} reason={}",
                        candidate.display(),
                        error
                    );
                    continue;
                }
            };
            let key = normalize_compare_path(canonical.to_string_lossy().as_ref());
            if seen.insert(key) {
                roots.push(canonical);
            }
        }

        Ok(roots)
    }
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
    let operation_cancelled = Arc::clone(&cancelled);
    let handle = tauri::async_runtime::spawn_blocking(move || operation(operation_cancelled));
    match tokio::time::timeout(timeout, handle).await {
        Ok(joined) => joined.map_err(|error| {
            AppError::new(
                "CODEX_HISTORY_TASK_FAILED",
                "codex history task join failed",
            )
            .with_detail("operation", operation_name)
            .with_detail("reason", error.to_string())
        })?,
        Err(_) => {
            cancelled.store(true, Ordering::Relaxed);
            Err(
                AppError::new("CODEX_HISTORY_TIMEOUT", "codex history operation timed out")
                    .with_detail("operation", operation_name)
                    .retryable(true),
            )
        }
    }
}

fn scan_sessions_for_workspace(
    roots: &[PathBuf],
    context: &WorkspaceHistoryContext,
    cancelled: &AtomicBool,
) -> AppResult<Vec<CodexHistorySession>> {
    let mut files = Vec::new();
    for root in roots {
        check_cancelled(cancelled)?;
        collect_jsonl_files(root, &mut files, cancelled)?;
    }

    let mut sessions = Vec::new();
    for path in files {
        check_cancelled(cancelled)?;
        match parse_session(&path) {
            Ok(parsed) => {
                let Some(project_dir) = parsed.session.project_dir.as_deref() else {
                    continue;
                };
                if path_is_under_workspace(project_dir, &context.workspace_compare_path) {
                    sessions.push(parsed);
                }
            }
            Err(error) => {
                log::warn!(
                    target: "bexo::service::codex_history",
                    "skip codex session parse path={} error={}",
                    path.display(),
                    error
                );
            }
        }
    }

    sessions.sort_by(|left, right| right.sort_ms.cmp(&left.sort_ms));
    Ok(sessions.into_iter().map(|parsed| parsed.session).collect())
}

fn scan_all_sessions(
    roots: &[PathBuf],
    cancelled: &AtomicBool,
) -> AppResult<Vec<CodexHistorySession>> {
    let mut files = Vec::new();
    for root in roots {
        check_cancelled(cancelled)?;
        collect_jsonl_files(root, &mut files, cancelled)?;
    }

    let mut sessions = Vec::new();
    for path in files {
        check_cancelled(cancelled)?;
        match parse_session(&path) {
            Ok(parsed) => sessions.push(parsed),
            Err(error) => {
                log::warn!(
                    target: "bexo::service::codex_history",
                    "skip codex session parse path={} error={}",
                    path.display(),
                    error
                );
            }
        }
    }

    sessions.sort_by(|left, right| right.sort_ms.cmp(&left.sort_ms));
    Ok(sessions.into_iter().map(|parsed| parsed.session).collect())
}

fn collect_jsonl_files(
    root: &Path,
    files: &mut Vec<PathBuf>,
    cancelled: &AtomicBool,
) -> AppResult<()> {
    check_cancelled(cancelled)?;
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) => {
            log::warn!(
            target: "bexo::service::codex_history",
            "failed to read codex sessions directory path={} reason={}",
            root.display(),
                    error
                );
            return Ok(());
        }
    };

    for entry in entries.flatten() {
        check_cancelled(cancelled)?;
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        if file_type.is_dir() {
            collect_jsonl_files(&path, files, cancelled)?;
        } else if file_type.is_file()
            && path.extension().and_then(|value| value.to_str()) == Some("jsonl")
        {
            files.push(path);
        }
    }

    Ok(())
}

fn parse_session(path: &Path) -> AppResult<ParsedSession> {
    let (head, tail) =
        read_head_tail_lines(path, HEAD_LINE_COUNT, TAIL_LINE_COUNT).map_err(|error| {
            AppError::new(
                "CODEX_HISTORY_READ_FAILED",
                "failed to read codex session metadata",
            )
            .with_detail("path", path.display().to_string())
            .with_detail("reason", error.to_string())
        })?;

    let mut session_id = None;
    let mut project_dir = None;
    let mut created_at: Option<TimestampValue> = None;
    let mut first_user_message = None;

    for line in &head {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if created_at.is_none() {
            created_at = value.get("timestamp").and_then(parse_timestamp_value);
        }

        if value.get("type").and_then(Value::as_str) == Some("session_meta") {
            if let Some(payload) = value.get("payload") {
                if is_subagent_source(payload.get("source")) {
                    return Err(AppError::new(
                        "CODEX_HISTORY_SUBAGENT_SESSION",
                        "subagent codex session is skipped",
                    ));
                }
                if session_id.is_none() {
                    session_id = payload
                        .get("id")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                }
                if project_dir.is_none() {
                    project_dir = payload
                        .get("cwd")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                }
                if created_at.is_none() {
                    created_at = payload.get("timestamp").and_then(parse_timestamp_value);
                }
            }
        }

        if first_user_message.is_none()
            && value.get("type").and_then(Value::as_str) == Some("response_item")
        {
            if let Some(payload) = value.get("payload") {
                if payload.get("type").and_then(Value::as_str) == Some("message")
                    && payload.get("role").and_then(Value::as_str) == Some("user")
                {
                    let text = payload.get("content").map(extract_text).unwrap_or_default();
                    let trimmed = text.trim();
                    if !trimmed.is_empty()
                        && !trimmed.starts_with("# AGENTS.md")
                        && !trimmed.starts_with("<environment_context>")
                    {
                        first_user_message = Some(trimmed.to_string());
                    }
                }
            }
        }
    }

    let mut last_active_at = None;
    let mut summary = None;
    for line in tail.iter().rev() {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if last_active_at.is_none() {
            last_active_at = value.get("timestamp").and_then(parse_timestamp_value);
        }
        if summary.is_none() && value.get("type").and_then(Value::as_str) == Some("response_item") {
            if let Some(payload) = value.get("payload") {
                if payload.get("type").and_then(Value::as_str) == Some("message") {
                    let text = payload.get("content").map(extract_text).unwrap_or_default();
                    if !text.trim().is_empty() {
                        summary = Some(truncate_chars(&text, SUMMARY_MAX_CHARS));
                    }
                }
            }
        }
    }

    let session_id = session_id
        .or_else(|| infer_uuid_from_filename(path))
        .ok_or_else(|| {
            AppError::new(
                "CODEX_HISTORY_PARSE_FAILED",
                "failed to resolve codex session id",
            )
            .with_detail("path", path.display().to_string())
        })?;

    let title = first_user_message
        .map(|text| truncate_chars(&text, TITLE_MAX_CHARS))
        .or_else(|| project_dir.as_deref().and_then(path_basename));
    let file_size = path.metadata().map(|metadata| metadata.len()).unwrap_or(0);
    let sort_ms = last_active_at
        .as_ref()
        .or(created_at.as_ref())
        .map(|timestamp| timestamp.ms)
        .unwrap_or(0);

    Ok(ParsedSession {
        session: CodexHistorySession {
            provider_id: CODEX_PROVIDER_ID.to_string(),
            session_id,
            title,
            summary,
            project_dir,
            created_at: created_at.map(|timestamp| timestamp.display),
            last_active_at: last_active_at.map(|timestamp| timestamp.display),
            source_path: display_path(path),
            file_size_bytes: file_size,
        },
        sort_ms,
    })
}

fn read_head_tail_lines(
    path: &Path,
    head_n: usize,
    tail_n: usize,
) -> std::io::Result<(Vec<String>, Vec<String>)> {
    let file = File::open(path)?;
    let file_len = file.metadata()?.len();
    if file_len < TAIL_READ_BYTES {
        let reader = BufReader::new(file);
        let all = reader.lines().map_while(Result::ok).collect::<Vec<_>>();
        let head = all.iter().take(head_n).cloned().collect::<Vec<_>>();
        let skip = all.len().saturating_sub(tail_n);
        let tail = all.into_iter().skip(skip).collect::<Vec<_>>();
        return Ok((head, tail));
    }

    let head_reader = BufReader::new(file);
    let head = head_reader
        .lines()
        .take(head_n)
        .map_while(Result::ok)
        .collect::<Vec<_>>();

    let seek_pos = file_len.saturating_sub(TAIL_READ_BYTES);
    let mut tail_file = File::open(path)?;
    tail_file.seek(SeekFrom::Start(seek_pos))?;
    let tail_reader = BufReader::new(tail_file);
    let tail_lines = tail_reader
        .lines()
        .map_while(Result::ok)
        .skip(if seek_pos > 0 { 1 } else { 0 })
        .collect::<Vec<_>>();
    let skip = tail_lines.len().saturating_sub(tail_n);

    Ok((head, tail_lines.into_iter().skip(skip).collect()))
}

fn read_messages_page_from_tail(
    path: &Path,
    cursor: Option<&str>,
    limit: usize,
    cancelled: &AtomicBool,
) -> AppResult<CodexHistoryMessagesPage> {
    let mut file = File::open(path).map_err(|error| {
        AppError::new("CODEX_HISTORY_READ_FAILED", "failed to open codex session")
            .with_detail("path", path.display().to_string())
            .with_detail("reason", error.to_string())
    })?;
    let file_len = file
        .metadata()
        .map_err(|error| {
            AppError::new(
                "CODEX_HISTORY_READ_FAILED",
                "failed to read codex session metadata",
            )
            .with_detail("path", path.display().to_string())
            .with_detail("reason", error.to_string())
        })?
        .len();
    let mut position = match cursor {
        Some(raw) if !raw.trim().is_empty() => raw.trim().parse::<u64>().map_err(|_| {
            AppError::validation("cursor must be a valid byte offset")
                .with_detail("cursor", raw.to_string())
        })?,
        _ => file_len,
    }
    .min(file_len);

    let mut carry = Vec::<u8>::new();
    let mut scanned_bytes = 0_u64;
    let mut messages = Vec::<(u64, CodexHistoryMessage)>::new();
    let mut earliest_message_offset = position;

    while position > 0 && messages.len() < limit && scanned_bytes < MESSAGE_MAX_SCAN_BYTES {
        check_cancelled(cancelled)?;
        let read_size = position.min(MESSAGE_READ_CHUNK_BYTES);
        let chunk_start = position - read_size;
        file.seek(SeekFrom::Start(chunk_start)).map_err(|error| {
            AppError::new("CODEX_HISTORY_READ_FAILED", "failed to seek codex session")
                .with_detail("path", path.display().to_string())
                .with_detail("reason", error.to_string())
        })?;
        let mut chunk = vec![0_u8; read_size as usize];
        file.read_exact(&mut chunk).map_err(|error| {
            AppError::new(
                "CODEX_HISTORY_READ_FAILED",
                "failed to read codex session chunk",
            )
            .with_detail("path", path.display().to_string())
            .with_detail("reason", error.to_string())
        })?;
        scanned_bytes += read_size;

        let mut combined = chunk;
        combined.extend_from_slice(&carry);
        let mut segment_end = combined.len();

        while let Some(newline_index) = combined[..segment_end]
            .iter()
            .rposition(|byte| *byte == b'\n')
        {
            let line_start = newline_index + 1;
            if segment_end > line_start {
                let byte_offset = chunk_start + line_start as u64;
                if let Some(message) = parse_message_line(&combined[line_start..segment_end]) {
                    earliest_message_offset = byte_offset;
                    messages.push((byte_offset, message));
                    if messages.len() >= limit {
                        break;
                    }
                }
            }
            segment_end = newline_index;
        }

        if messages.len() >= limit {
            break;
        }

        if chunk_start == 0 {
            if segment_end > 0 {
                if let Some(message) = parse_message_line(&combined[..segment_end]) {
                    earliest_message_offset = 0;
                    messages.push((0, message));
                }
            }
            carry.clear();
        } else if segment_end <= MESSAGE_MAX_CARRY_BYTES {
            carry = combined[..segment_end].to_vec();
        } else {
            carry.clear();
        }

        position = chunk_start;
    }

    let reached_start = position == 0 || earliest_message_offset == 0;
    let hit_scan_budget = scanned_bytes >= MESSAGE_MAX_SCAN_BYTES && !reached_start;
    let has_more = !reached_start || hit_scan_budget;
    let older_cursor = if has_more {
        let cursor_offset = if messages.is_empty() {
            position
        } else {
            earliest_message_offset
        };
        (cursor_offset > 0).then(|| cursor_offset.to_string())
    } else {
        None
    };

    messages.reverse();
    Ok(CodexHistoryMessagesPage {
        messages: messages.into_iter().map(|(_, message)| message).collect(),
        older_cursor,
        has_more,
    })
}

fn check_cancelled(cancelled: &AtomicBool) -> AppResult<()> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(AppError::new(
            "CODEX_HISTORY_CANCELLED",
            "codex history operation was cancelled",
        )
        .retryable(true));
    }
    Ok(())
}

fn parse_message_line(line_bytes: &[u8]) -> Option<CodexHistoryMessage> {
    let line = std::str::from_utf8(trim_line_end(line_bytes)).ok()?;
    let value = serde_json::from_str::<Value>(line).ok()?;
    if value.get("type").and_then(Value::as_str) != Some("response_item") {
        return None;
    }
    let payload = value.get("payload")?;
    let item_type = payload.get("type").and_then(Value::as_str).unwrap_or("");
    let timestamp = value
        .get("timestamp")
        .and_then(parse_timestamp_value)
        .map(|timestamp| timestamp.display);

    let (role, content) = match item_type {
        "message" => {
            let role = payload
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            let content = payload.get("content").map(extract_text).unwrap_or_default();
            (role, content)
        }
        "function_call" => {
            let name = payload
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let arguments = payload
                .get("arguments")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            let content = if arguments.is_empty() {
                format!("[Tool: {name}]")
            } else {
                format!("[Tool: {name}]\n{arguments}")
            };
            ("assistant".to_string(), content)
        }
        "function_call_output" => {
            let output = payload
                .get("output")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            ("tool".to_string(), output)
        }
        "web_search_call" => ("assistant".to_string(), "[Web Search]".to_string()),
        "reasoning" => {
            let content = payload
                .get("summary")
                .map(extract_text)
                .filter(|text| !text.trim().is_empty())
                .or_else(|| {
                    payload
                        .get("content")
                        .map(extract_text)
                        .filter(|text| !text.trim().is_empty())
                })
                .unwrap_or_default();
            ("assistant".to_string(), content)
        }
        _ => return None,
    };

    let content = content.trim();
    if content.is_empty() {
        return None;
    }
    let (content, truncated) = truncate_message_content(content, MAX_MESSAGE_CHARS);

    Some(CodexHistoryMessage {
        role,
        content,
        timestamp,
        item_type: item_type.to_string(),
        truncated,
    })
}

fn validate_source_path(source_path: &str, roots: &[PathBuf]) -> AppResult<PathBuf> {
    let trimmed = source_path.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation("sourcePath is required"));
    }
    let source = PathBuf::from(trimmed);
    if !source.is_absolute() {
        return Err(AppError::new(
            "CODEX_HISTORY_SOURCE_INVALID",
            "sourcePath must be absolute",
        )
        .with_detail("sourcePath", trimmed.to_string()));
    }
    if source.extension().and_then(|value| value.to_str()) != Some("jsonl") {
        return Err(AppError::new(
            "CODEX_HISTORY_SOURCE_INVALID",
            "sourcePath must point to a Codex JSONL session",
        )
        .with_detail("sourcePath", trimmed.to_string()));
    }
    let canonical_source = source.canonicalize().map_err(|error| {
        AppError::new(
            "CODEX_HISTORY_SOURCE_INVALID",
            "failed to resolve codex session sourcePath",
        )
        .with_detail("sourcePath", trimmed.to_string())
        .with_detail("reason", error.to_string())
    })?;

    for root in roots {
        let Ok(canonical_root) = root.canonicalize() else {
            continue;
        };
        if canonical_source.starts_with(&canonical_root) {
            return Ok(canonical_source);
        }
    }

    Err(AppError::new(
        "CODEX_HISTORY_SOURCE_OUTSIDE_ROOT",
        "codex session sourcePath is outside known Codex sessions roots",
    )
    .with_detail("sourcePath", trimmed.to_string()))
}

fn trim_line_end(mut bytes: &[u8]) -> &[u8] {
    while matches!(bytes.last(), Some(b'\n' | b'\r')) {
        bytes = &bytes[..bytes.len().saturating_sub(1)];
    }
    bytes
}

fn parse_timestamp_value(value: &Value) -> Option<TimestampValue> {
    let ms = if let Some(value) = value.as_i64() {
        if value > 1_000_000_000_000 {
            value
        } else {
            value * 1000
        }
    } else if let Some(value) = value.as_f64() {
        let value = value as i64;
        if value > 1_000_000_000_000 {
            value
        } else {
            value * 1000
        }
    } else {
        let raw = value.as_str()?;
        let parsed = DateTime::parse_from_rfc3339(raw).ok()?;
        parsed.timestamp_millis()
    };
    let display = Utc.timestamp_millis_opt(ms).single()?.to_rfc3339();
    Some(TimestampValue { ms, display })
}

fn extract_text(content: &Value) -> String {
    match content {
        Value::String(text) => text.to_string(),
        Value::Array(items) => items
            .iter()
            .filter_map(extract_text_from_item)
            .filter(|text| !text.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Object(map) => map
            .get("text")
            .and_then(Value::as_str)
            .or_else(|| map.get("input_text").and_then(Value::as_str))
            .or_else(|| map.get("output_text").and_then(Value::as_str))
            .unwrap_or_default()
            .to_string(),
        _ => String::new(),
    }
}

fn extract_text_from_item(item: &Value) -> Option<String> {
    if let Some(text) = item.get("text").and_then(Value::as_str) {
        return Some(text.to_string());
    }
    if let Some(text) = item.get("input_text").and_then(Value::as_str) {
        return Some(text.to_string());
    }
    if let Some(text) = item.get("output_text").and_then(Value::as_str) {
        return Some(text.to_string());
    }
    if let Some(content) = item.get("content") {
        let text = extract_text(content);
        if !text.trim().is_empty() {
            return Some(text);
        }
    }
    None
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let mut value = trimmed.chars().take(max_chars).collect::<String>();
    value.push_str("...");
    value
}

fn truncate_message_content(text: &str, max_chars: usize) -> (String, bool) {
    if text.chars().count() <= max_chars {
        return (text.to_string(), false);
    }
    let mut value = text.chars().take(max_chars).collect::<String>();
    value.push_str("\n\n[内容过长，已截断]");
    (value, true)
}

fn is_subagent_source(source: Option<&Value>) -> bool {
    source
        .and_then(Value::as_object)
        .map(|source| source.contains_key("subagent"))
        .unwrap_or(false)
}

fn infer_uuid_from_filename(path: &Path) -> Option<String> {
    let filename = path.file_name()?.to_string_lossy();
    let chars = filename.chars().collect::<Vec<_>>();
    if chars.len() < 36 {
        return None;
    }
    for start in 0..=chars.len().saturating_sub(36) {
        let candidate = chars[start..start + 36].iter().collect::<String>();
        if uuid::Uuid::parse_str(&candidate).is_ok() {
            return Some(candidate);
        }
    }
    None
}

fn path_basename(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_end_matches(['/', '\\']);
    if trimmed.is_empty() {
        return None;
    }
    trimmed
        .split(['/', '\\'])
        .next_back()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

fn display_path(path: &Path) -> String {
    normalize_display_path(path.display().to_string())
}

fn normalize_display_path(path: String) -> String {
    if let Some(stripped) = path.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{}", stripped);
    }
    if let Some(stripped) = path.strip_prefix(r"\\?\") {
        return stripped.to_string();
    }
    path
}

fn path_is_under_workspace(path: &str, workspace_compare_path: &str) -> bool {
    let candidate = normalize_compare_path(path);
    if candidate == workspace_compare_path {
        return true;
    }
    let separator = if cfg!(windows) { "\\" } else { "/" };
    candidate.starts_with(&format!("{workspace_compare_path}{separator}"))
}

fn normalize_compare_path(path: &str) -> String {
    let mut value = path.trim().replace('/', "\\");
    if let Some(stripped) = value.strip_prefix(r"\\?\UNC\") {
        value = format!(r"\\{}", stripped);
    } else if let Some(stripped) = value.strip_prefix(r"\\?\") {
        value = stripped.to_string();
    }
    while value.len() > 3 && value.ends_with('\\') {
        value.pop();
    }
    if cfg!(windows) {
        value.to_ascii_lowercase()
    } else {
        value.replace('\\', "/")
    }
}

fn history_window_label(workspace_id: &str) -> String {
    format!(
        "{HISTORY_WINDOW_PREFIX}{}",
        workspace_id
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() {
                    character
                } else {
                    '_'
                }
            })
            .collect::<String>()
    )
}
