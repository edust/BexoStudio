use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use crate::{
    domain::{
        ensure_absolute_directory, WorkspaceResourceEntry, WorkspaceResourceGitStatusEntry,
        WorkspaceResourceGitStatusResponse,
    },
    error::{AppError, AppResult},
    persistence::{get_workspace_primary_project_path, Database},
};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone)]
pub struct ResourceBrowserService {
    database: Database,
}

impl ResourceBrowserService {
    const RESOURCE_IO_TIMEOUT: Duration = Duration::from_secs(4);
    const GIT_COMMAND_TIMEOUT: Duration = Duration::from_secs(3);

    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub async fn list_workspace_resource_children(
        &self,
        workspace_id: String,
        target_path: Option<String>,
    ) -> AppResult<Vec<WorkspaceResourceEntry>> {
        let workspace_root = self.resolve_workspace_root_path_value(workspace_id).await?;

        run_blocking(
            "list_workspace_resource_children",
            Self::RESOURCE_IO_TIMEOUT,
            move || {
                let root_path =
                    canonicalize_existing_directory(&workspace_root, "INVALID_WORKSPACE_PATH")?;
                let requested_path = match target_path {
                    Some(path) if !path.trim().is_empty() => path.trim().to_string(),
                    _ => workspace_root,
                };
                let requested_canonical =
                    canonicalize_existing_directory(&requested_path, "INVALID_RESOURCE_PATH")?;
                ensure_path_within_workspace(&root_path, &requested_canonical)?;

                let read_dir = std::fs::read_dir(&requested_canonical).map_err(|error| {
                    AppError::new(
                        "RESOURCE_READ_FAILED",
                        "failed to read workspace resource directory",
                    )
                    .with_detail("path", requested_canonical.display().to_string())
                    .with_detail("reason", error.to_string())
                })?;
                let mut entries = Vec::new();
                for entry in read_dir {
                    let entry = entry.map_err(|error| {
                        AppError::new(
                            "RESOURCE_READ_FAILED",
                            "failed to iterate workspace resource directory",
                        )
                        .with_detail("path", requested_canonical.display().to_string())
                        .with_detail("reason", error.to_string())
                    })?;
                    entries.push(build_resource_entry(entry)?);
                }

                entries.sort_by(
                    |left, right| match (left.kind.as_str(), right.kind.as_str()) {
                        ("directory", "file") => std::cmp::Ordering::Less,
                        ("file", "directory") => std::cmp::Ordering::Greater,
                        _ => left.name.to_lowercase().cmp(&right.name.to_lowercase()),
                    },
                );

                Ok(entries)
            },
        )
        .await
    }

    pub async fn resolve_workspace_root_path(&self, workspace_id: String) -> AppResult<String> {
        let workspace_root = self.resolve_workspace_root_path_value(workspace_id).await?;

        run_blocking(
            "resolve_workspace_resource_root_path",
            Self::RESOURCE_IO_TIMEOUT,
            move || {
                let workspace_root_path =
                    canonicalize_existing_directory(&workspace_root, "INVALID_WORKSPACE_PATH")?;
                Ok(workspace_root_path.display().to_string())
            },
        )
        .await
    }

    pub async fn get_workspace_resource_git_statuses(
        &self,
        workspace_id: String,
    ) -> AppResult<WorkspaceResourceGitStatusResponse> {
        let workspace_root = self.resolve_workspace_root_path_value(workspace_id).await?;

        run_blocking(
            "get_workspace_resource_git_statuses",
            Self::RESOURCE_IO_TIMEOUT,
            move || {
                let workspace_root_path =
                    canonicalize_existing_directory(&workspace_root, "INVALID_WORKSPACE_PATH")?;

                let repository_root = match run_git_command(
                    &workspace_root_path,
                    &["rev-parse", "--show-toplevel"],
                    Self::GIT_COMMAND_TIMEOUT,
                ) {
                    Ok(output) => {
                        let trimmed = output.trim();
                        if trimmed.is_empty() {
                            None
                        } else {
                            Some(canonicalize_existing_directory(
                                trimmed,
                                "GIT_REPOSITORY_UNAVAILABLE",
                            )?)
                        }
                    }
                    Err(error)
                        if error.code == "GIT_UNAVAILABLE"
                            || error.code == "NOT_GIT_REPOSITORY" =>
                    {
                        None
                    }
                    Err(error) => return Err(error),
                };

                let Some(repository_root_path) = repository_root else {
                    return Ok(WorkspaceResourceGitStatusResponse {
                        workspace_root_path: workspace_root_path.display().to_string(),
                        git_available: false,
                        repository_root_path: None,
                        statuses: Vec::new(),
                    });
                };

                let raw_status_output = run_git_command(
                    &workspace_root_path,
                    &[
                        "status",
                        "--porcelain=v1",
                        "-z",
                        "--ignored=matching",
                        "--untracked-files=normal",
                    ],
                    Self::GIT_COMMAND_TIMEOUT,
                )?;
                let statuses = parse_git_status_entries(
                    raw_status_output.as_bytes(),
                    &workspace_root_path,
                    &repository_root_path,
                )?;

                Ok(WorkspaceResourceGitStatusResponse {
                    workspace_root_path: workspace_root_path.display().to_string(),
                    git_available: true,
                    repository_root_path: Some(repository_root_path.display().to_string()),
                    statuses,
                })
            },
        )
        .await
    }

    async fn resolve_workspace_root_path_value(&self, workspace_id: String) -> AppResult<String> {
        let normalized_id = workspace_id.trim().to_string();
        if normalized_id.is_empty() {
            return Err(AppError::validation("workspaceId is required"));
        }

        self.database
            .read(
                "get_workspace_primary_project_path_for_resource_browser",
                move |connection| get_workspace_primary_project_path(connection, normalized_id),
            )
            .await
            .and_then(|path| {
                ensure_absolute_directory(&path, "INVALID_WORKSPACE_PATH")
                    .map_err(|error| error.with_detail("workspacePath", path))
            })
    }
}

fn build_resource_entry(entry: std::fs::DirEntry) -> AppResult<WorkspaceResourceEntry> {
    let file_type = entry.file_type().map_err(|error| {
        AppError::new(
            "RESOURCE_READ_FAILED",
            "failed to inspect workspace resource",
        )
        .with_detail("path", entry.path().display().to_string())
        .with_detail("reason", error.to_string())
    })?;

    let name = entry.file_name().to_string_lossy().to_string();
    let kind = if file_type.is_dir() {
        "directory"
    } else {
        "file"
    };

    Ok(WorkspaceResourceEntry {
        path: entry.path().display().to_string(),
        name: name.clone(),
        kind: kind.to_string(),
        is_hidden: is_hidden_name(&name),
    })
}

fn is_hidden_name(name: &str) -> bool {
    name.starts_with('.')
}

fn canonicalize_existing_directory(path: &str, error_code: &str) -> AppResult<PathBuf> {
    let normalized = ensure_absolute_directory(path, error_code)?;
    std::fs::canonicalize(&normalized).map_err(|error| {
        AppError::new(error_code, "failed to canonicalize directory path")
            .with_detail("path", normalized)
            .with_detail("reason", error.to_string())
    })
}

fn canonicalize_existing_path(path: &Path, error_code: &str) -> AppResult<PathBuf> {
    std::fs::canonicalize(path).map_err(|error| {
        AppError::new(error_code, "failed to canonicalize path")
            .with_detail("path", path.display().to_string())
            .with_detail("reason", error.to_string())
    })
}

fn ensure_path_within_workspace(workspace_root: &Path, target_path: &Path) -> AppResult<()> {
    if target_path.starts_with(workspace_root) {
        return Ok(());
    }

    Err(AppError::new(
        "RESOURCE_PATH_OUT_OF_SCOPE",
        "resource path is outside workspace root",
    )
    .with_detail("workspaceRoot", workspace_root.display().to_string())
    .with_detail("targetPath", target_path.display().to_string()))
}

fn run_git_command(cwd: &Path, args: &[&str], timeout: Duration) -> AppResult<String> {
    let mut command = Command::new("git");
    configure_background_command(&mut command);
    let mut child = command
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            let code = if error.kind() == std::io::ErrorKind::NotFound {
                "GIT_UNAVAILABLE"
            } else {
                "GIT_COMMAND_FAILED"
            };
            AppError::new(code, "failed to start git command")
                .with_detail("cwd", cwd.display().to_string())
                .with_detail("args", args.join(" "))
                .with_detail("reason", error.to_string())
        })?;

    let started_at = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = child.wait_with_output().map_err(|error| {
                    AppError::new("GIT_COMMAND_FAILED", "failed to read git command output")
                        .with_detail("cwd", cwd.display().to_string())
                        .with_detail("args", args.join(" "))
                        .with_detail("reason", error.to_string())
                })?;

                if status.success() {
                    return String::from_utf8(output.stdout).map_err(|error| {
                        AppError::new("GIT_OUTPUT_INVALID", "git output was not valid UTF-8")
                            .with_detail("cwd", cwd.display().to_string())
                            .with_detail("args", args.join(" "))
                            .with_detail("reason", error.to_string())
                    });
                }

                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                let is_not_repo = stderr.contains("not a git repository");
                return Err(AppError::new(
                    if is_not_repo {
                        "NOT_GIT_REPOSITORY"
                    } else {
                        "GIT_COMMAND_FAILED"
                    },
                    "git command failed",
                )
                .with_detail("cwd", cwd.display().to_string())
                .with_detail("args", args.join(" "))
                .with_detail("stderr", stderr));
            }
            Ok(None) => {
                if started_at.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(
                        AppError::new("GIT_COMMAND_TIMEOUT", "git command timed out")
                            .with_detail("cwd", cwd.display().to_string())
                            .with_detail("args", args.join(" "))
                            .retryable(true),
                    );
                }

                thread::sleep(Duration::from_millis(25));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(
                    AppError::new("GIT_COMMAND_FAILED", "failed to poll git command")
                        .with_detail("cwd", cwd.display().to_string())
                        .with_detail("args", args.join(" "))
                        .with_detail("reason", error.to_string()),
                );
            }
        }
    }
}

fn configure_background_command(command: &mut Command) {
    #[cfg(windows)]
    {
        command.creation_flags(CREATE_NO_WINDOW);
    }
}

fn parse_git_status_entries(
    payload: &[u8],
    workspace_root: &Path,
    repository_root: &Path,
) -> AppResult<Vec<WorkspaceResourceGitStatusEntry>> {
    let segments = payload
        .split(|byte| *byte == 0)
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let mut entries = Vec::new();
    let mut cursor = 0usize;

    while cursor < segments.len() {
        let segment = segments[cursor];
        if segment.len() < 4 {
            cursor += 1;
            continue;
        }

        let x = segment[0] as char;
        let y = segment[1] as char;
        let path = decode_git_path(&segment[3..])?;
        let status = classify_git_status(x, y);
        let requires_original_path = matches!(x, 'R' | 'C') || matches!(y, 'R' | 'C');

        let original_path = if requires_original_path {
            let next = segments.get(cursor + 1).ok_or_else(|| {
                AppError::new(
                    "GIT_OUTPUT_INVALID",
                    "git rename status did not include original path",
                )
                .with_detail("path", path.clone())
            })?;
            cursor += 2;
            Some(decode_git_path(next)?)
        } else {
            cursor += 1;
            None
        };

        let Some(status) = status else {
            continue;
        };

        let absolute_path = canonicalize_git_entry_path(repository_root, &path)?;
        if !absolute_path.starts_with(workspace_root) {
            continue;
        }

        let absolute_original_path = match original_path {
            Some(original_path) => {
                let resolved = canonicalize_git_entry_path(repository_root, &original_path)?;
                resolved.starts_with(workspace_root).then(|| resolved)
            }
            None => None,
        };

        entries.push(WorkspaceResourceGitStatusEntry {
            path: absolute_path.display().to_string(),
            status: status.to_string(),
            original_path: absolute_original_path.map(|value| value.display().to_string()),
        });
    }

    Ok(entries)
}

fn decode_git_path(path_bytes: &[u8]) -> AppResult<String> {
    String::from_utf8(path_bytes.to_vec()).map_err(|error| {
        AppError::new("GIT_OUTPUT_INVALID", "git path entry was not valid UTF-8")
            .with_detail("reason", error.to_string())
    })
}

fn classify_git_status(x: char, y: char) -> Option<&'static str> {
    if x == '!' || y == '!' {
        return Some("ignored");
    }

    if x == '?' || y == '?' {
        return Some("untracked");
    }

    if matches!(x, 'R' | 'C') || matches!(y, 'R' | 'C') {
        return Some("renamed");
    }

    if matches!(x, 'M' | 'A' | 'D' | 'U') || matches!(y, 'M' | 'A' | 'D' | 'U') {
        return Some("modified");
    }

    None
}

fn canonicalize_git_entry_path(repository_root: &Path, relative_path: &str) -> AppResult<PathBuf> {
    let relative = Path::new(relative_path);
    let joined = repository_root.join(relative);

    if joined.exists() {
        return canonicalize_existing_path(&joined, "GIT_OUTPUT_INVALID");
    }

    normalize_missing_path(repository_root, relative)
}

fn normalize_missing_path(repository_root: &Path, relative_path: &Path) -> AppResult<PathBuf> {
    let mut normalized = canonicalize_existing_path(repository_root, "GIT_OUTPUT_INVALID")?;
    for component in relative_path.components() {
        normalized.push(component.as_os_str());
    }
    Ok(normalized)
}

async fn run_blocking<T, F>(
    operation_name: &'static str,
    timeout: Duration,
    operation: F,
) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> AppResult<T> + Send + 'static,
{
    let handle = tauri::async_runtime::spawn_blocking(operation);
    match tokio::time::timeout(timeout, handle).await {
        Ok(joined) => match joined {
            Ok(result) => result,
            Err(error) => Err(AppError::new(
                "RESOURCE_TASK_FAILED",
                "resource browser task join failed",
            )
            .with_detail("operation", operation_name)
            .with_detail("reason", error.to_string())),
        },
        Err(_) => Err(AppError::new(
            "RESOURCE_TASK_TIMEOUT",
            "resource browser operation timed out",
        )
        .with_detail("operation", operation_name)
        .retryable(true)),
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_git_status, parse_git_status_entries};

    #[test]
    fn classify_git_status_maps_supported_values() {
        assert_eq!(classify_git_status('M', ' '), Some("modified"));
        assert_eq!(classify_git_status('R', ' '), Some("renamed"));
        assert_eq!(classify_git_status('?', '?'), Some("untracked"));
        assert_eq!(classify_git_status('!', '!'), Some("ignored"));
        assert_eq!(classify_git_status(' ', ' '), None);
    }

    #[test]
    fn parse_git_status_entries_handles_rename_and_ignored() {
        let workspace_root = std::env::temp_dir().join("bexo-resource-browser-workspace");
        let repository_root = workspace_root.clone();
        std::fs::create_dir_all(workspace_root.join("src")).expect("create workspace tree");
        std::fs::write(workspace_root.join("src").join("new.ts"), "export {};\n")
            .expect("write renamed target");

        let payload = b"R  src/new.ts\0src/old.ts\0!! dist/\0";
        let entries = parse_git_status_entries(payload, &workspace_root, &repository_root)
            .expect("parse git entries");

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].status, "renamed");
        assert!(entries[0].path.replace('\\', "/").ends_with("/src/new.ts"));
        assert!(entries[0]
            .original_path
            .as_deref()
            .unwrap_or_default()
            .replace('\\', "/")
            .ends_with("/src/old.ts"));
        assert_eq!(entries[1].status, "ignored");
    }
}
