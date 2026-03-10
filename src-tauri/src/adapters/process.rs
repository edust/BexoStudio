use std::{
    collections::{HashMap, HashSet},
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct LaunchCommand {
    pub executable_path: PathBuf,
    pub args: Vec<String>,
    pub current_dir: Option<PathBuf>,
    pub envs: Vec<(String, String)>,
    pub timeout: Duration,
    pub tracking: Option<ProcessTrackingContext>,
    pub retain_after_timeout: bool,
}

#[derive(Debug, Clone)]
pub struct ProcessLaunchResult {
    pub executable_path: String,
    pub duration_ms: i64,
}

#[derive(Debug, Clone)]
pub struct ProcessTrackingContext {
    pub registry: ChildProcessRegistry,
    pub run_id: String,
    pub action_keys: Vec<ActionProcessKey>,
    pub run_cancel_requested: Arc<AtomicBool>,
    pub action_cancel_requested: Vec<Arc<AtomicBool>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ActionProcessKey {
    pub project_task_id: String,
    pub action_id: String,
}

#[derive(Debug, Clone, Default)]
pub struct ChildProcessRegistry {
    inner: Arc<Mutex<HashMap<String, HashMap<ActionProcessKey, Vec<TrackedChildProcess>>>>>,
}

#[derive(Debug, Clone)]
struct TrackedChildProcess {
    pid: u32,
    executable_path: String,
}

pub fn find_first_executable(candidates: &[&str]) -> Option<PathBuf> {
    candidates
        .iter()
        .find_map(|candidate| find_executable(candidate))
}

pub fn resolve_configured_executable(
    configured_path: &str,
    nested_candidates: &[&str],
    error_code: &str,
    label: &str,
) -> AppResult<PathBuf> {
    let trimmed = configured_path.trim();
    if trimmed.is_empty() {
        return Err(AppError::new(
            error_code,
            format!("{label} path is required"),
        ));
    }

    let path = PathBuf::from(trimmed);
    if !path.is_absolute() {
        return Err(
            AppError::new(error_code, format!("{label} path must be absolute"))
                .with_detail("path", trimmed.to_string()),
        );
    }

    let metadata = fs::metadata(&path).map_err(|error| {
        AppError::new(error_code, format!("{label} path does not exist"))
            .with_detail("path", trimmed.to_string())
            .with_detail("reason", error.to_string())
    })?;

    if metadata.is_file() {
        return Ok(path);
    }

    if !metadata.is_dir() {
        return Err(AppError::new(
            error_code,
            format!("{label} path must point to a file or directory"),
        )
        .with_detail("path", trimmed.to_string()));
    }

    nested_candidates
        .iter()
        .map(|candidate| path.join(candidate))
        .find(|candidate| is_file(candidate))
        .ok_or_else(|| {
            AppError::new(
                error_code,
                format!("{label} directory does not contain a supported executable"),
            )
            .with_detail("path", trimmed.to_string())
            .with_detail("candidates", nested_candidates.join(", "))
        })
}

pub async fn run_launch_command(command: LaunchCommand) -> AppResult<ProcessLaunchResult> {
    let timeout = command.timeout;
    let handle = tauri::async_runtime::spawn_blocking(move || launch_blocking(command));

    match tokio::time::timeout(timeout + Duration::from_millis(250), handle).await {
        Ok(joined) => match joined {
            Ok(result) => result,
            Err(error) => Err(AppError::new(
                "PROCESS_TASK_FAILED",
                "process launch task join failed",
            )
            .with_detail("reason", error.to_string())),
        },
        Err(_) => Err(AppError::new("IO_TIMEOUT", "process launch timed out").retryable(true)),
    }
}

impl ChildProcessRegistry {
    pub fn register(
        &self,
        run_id: &str,
        action_keys: &[ActionProcessKey],
        pid: u32,
        executable_path: &str,
    ) {
        if action_keys.is_empty() {
            return;
        }
        let mut registry = self.inner.lock().expect("child process registry poisoned");
        let processes = registry.entry(run_id.to_string()).or_default();
        for action_key in action_keys {
            let tracked = processes.entry(action_key.clone()).or_default();
            if tracked.iter().any(|process| process.pid == pid) {
                continue;
            }
            tracked.push(TrackedChildProcess {
                pid,
                executable_path: executable_path.to_string(),
            });
        }
    }

    pub fn unregister(&self, run_id: &str, action_keys: &[ActionProcessKey], pid: u32) {
        if action_keys.is_empty() {
            return;
        }
        let mut registry = self.inner.lock().expect("child process registry poisoned");
        let Some(processes) = registry.get_mut(run_id) else {
            return;
        };

        for action_key in action_keys {
            let Some(tracked) = processes.get_mut(action_key) else {
                continue;
            };
            tracked.retain(|process| process.pid != pid);
            if tracked.is_empty() {
                processes.remove(action_key);
            }
        }
        if processes.is_empty() {
            registry.remove(run_id);
        }
    }

    pub fn clear_run(&self, run_id: &str) -> usize {
        let removed = self
            .inner
            .lock()
            .expect("child process registry poisoned")
            .remove(run_id);
        count_unique_processes(removed.as_ref())
    }

    pub fn count_action_processes(&self, run_id: &str, action_key: &ActionProcessKey) -> usize {
        self.inner
            .lock()
            .expect("child process registry poisoned")
            .get(run_id)
            .and_then(|processes| processes.get(action_key))
            .map(|tracked| unique_processes(tracked.iter().cloned()).len())
            .unwrap_or(0)
    }

    pub async fn terminate_run_processes(&self, run_id: &str) -> usize {
        let run_id = run_id.to_string();
        let run_id_for_worker = run_id.clone();
        let registry = self.clone();

        tauri::async_runtime::spawn_blocking(move || {
            registry.terminate_run_processes_blocking(&run_id_for_worker)
        })
        .await
        .unwrap_or_else(|error| {
            log::error!(
                target: "bexo::adapter::process",
                "terminate_run_processes join failed for run {}: {}",
                run_id,
                error
            );
            0
        })
    }

    pub async fn terminate_action_processes(
        &self,
        run_id: &str,
        action_key: &ActionProcessKey,
    ) -> usize {
        let run_id_for_log = run_id.to_string();
        let run_id_for_worker = run_id.to_string();
        let action_key_for_worker = action_key.clone();
        let action_key_for_log = action_key.clone();
        let registry = self.clone();

        tauri::async_runtime::spawn_blocking(move || {
            registry.terminate_action_processes_blocking(&run_id_for_worker, &action_key_for_worker)
        })
        .await
        .unwrap_or_else(|error| {
            log::error!(
                target: "bexo::adapter::process",
                "terminate_action_processes join failed for run {} project task {} action {}: {}",
                run_id_for_log,
                action_key_for_log.project_task_id,
                action_key_for_log.action_id,
                error
            );
            0
        })
    }

    fn terminate_run_processes_blocking(&self, run_id: &str) -> usize {
        let tracked_processes = self
            .inner
            .lock()
            .expect("child process registry poisoned")
            .get(run_id)
            .map(|processes| {
                let mut flattened = Vec::new();
                for tracked in processes.values() {
                    flattened.extend(tracked.iter().cloned());
                }
                unique_processes(flattened.into_iter())
            })
            .unwrap_or_default();

        let mut terminated = 0usize;
        for process in tracked_processes.values() {
            match terminate_process_tree(process.pid) {
                Ok(true) => terminated += 1,
                Ok(false) => {
                    log::debug!(
                        target: "bexo::adapter::process",
                        "process {} for run {} already exited",
                        process.pid,
                        run_id
                    );
                }
                Err(error) => {
                    log::warn!(
                        target: "bexo::adapter::process",
                        "failed to terminate process {} for run {} ({}): {}",
                        process.pid,
                        run_id,
                        process.executable_path,
                        error
                    );
                }
            }
        }

        terminated
    }

    fn terminate_action_processes_blocking(
        &self,
        run_id: &str,
        action_key: &ActionProcessKey,
    ) -> usize {
        let tracked_processes = self
            .inner
            .lock()
            .expect("child process registry poisoned")
            .get(run_id)
            .and_then(|processes| processes.get(action_key))
            .map(|tracked| unique_processes(tracked.iter().cloned()))
            .unwrap_or_default();

        let mut terminated = 0usize;
        for process in tracked_processes.values() {
            match terminate_process_tree(process.pid) {
                Ok(true) => terminated += 1,
                Ok(false) => {
                    log::debug!(
                        target: "bexo::adapter::process",
                        "process {} for run {} project task {} action {} already exited",
                        process.pid,
                        run_id,
                        action_key.project_task_id,
                        action_key.action_id
                    );
                }
                Err(error) => {
                    log::warn!(
                        target: "bexo::adapter::process",
                        "failed to terminate process {} for run {} project task {} action {} ({}): {}",
                        process.pid,
                        run_id,
                        action_key.project_task_id,
                        action_key.action_id,
                        process.executable_path,
                        error
                    );
                }
            }
        }

        terminated
    }
}

fn find_executable(candidate: &str) -> Option<PathBuf> {
    let candidate_path = Path::new(candidate);
    if candidate_path.components().count() > 1 {
        return resolve_candidate(candidate_path);
    }

    let path_value = env::var_os("PATH")?;
    env::split_paths(&path_value)
        .find_map(|directory| resolve_candidate(&directory.join(candidate)))
}

fn resolve_candidate(path: &Path) -> Option<PathBuf> {
    if path.extension().is_some() {
        return is_file(path).then(|| path.to_path_buf());
    }

    let mut with_extensions = candidate_extensions();
    with_extensions.insert(0, OsString::new());

    with_extensions.into_iter().find_map(|extension| {
        let resolved = if extension.is_empty() {
            path.to_path_buf()
        } else {
            let mut value = path.as_os_str().to_os_string();
            value.push(extension);
            PathBuf::from(value)
        };
        is_file(&resolved).then_some(resolved)
    })
}

fn candidate_extensions() -> Vec<OsString> {
    let pathext = env::var_os("PATHEXT")
        .map(|value| {
            value
                .to_string_lossy()
                .split(';')
                .filter(|part| !part.trim().is_empty())
                .map(OsString::from)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if pathext.is_empty() {
        vec![
            OsString::from(".EXE"),
            OsString::from(".CMD"),
            OsString::from(".BAT"),
            OsString::from(".COM"),
        ]
    } else {
        pathext
    }
}

fn is_file(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|metadata| metadata.is_file())
        .unwrap_or(false)
}

fn launch_blocking(command: LaunchCommand) -> AppResult<ProcessLaunchResult> {
    let launched_at = Instant::now();
    let executable_path = command.executable_path.clone();
    let tracking = command.tracking.clone();

    if let Some(tracking) = &tracking {
        if tracking.run_cancel_requested.load(Ordering::SeqCst)
            || tracking
                .action_cancel_requested
                .iter()
                .any(|token| token.load(Ordering::SeqCst))
        {
            return Err(process_cancelled_error(&executable_path, None));
        }
    }

    let (program, prefix_args) = resolve_program(&command.executable_path);

    let mut process = Command::new(&program);
    process.args(prefix_args);
    process.args(&command.args);
    process.stdin(Stdio::null());
    process.stdout(Stdio::null());
    process.stderr(Stdio::null());
    if let Some(current_dir) = &command.current_dir {
        process.current_dir(current_dir);
    }
    for (key, value) in &command.envs {
        process.env(key, value);
    }

    let mut child = process.spawn().map_err(|error| {
        AppError::new("PROCESS_SPAWN_FAILED", "failed to spawn external command")
            .with_detail("executablePath", executable_path.display().to_string())
            .with_detail("reason", error.to_string())
    })?;
    let process_id = child.id();
    if let Some(tracking) = &tracking {
        tracking.registry.register(
            &tracking.run_id,
            &tracking.action_keys,
            process_id,
            &executable_path.display().to_string(),
        );
    }

    loop {
        if let Some(tracking) = &tracking {
            if tracking.run_cancel_requested.load(Ordering::SeqCst)
                || tracking
                    .action_cancel_requested
                    .iter()
                    .any(|token| token.load(Ordering::SeqCst))
            {
                let _ = terminate_process_tree(process_id);
                let _ = child.wait();
                tracking
                    .registry
                    .unregister(&tracking.run_id, &tracking.action_keys, process_id);
                return Err(process_cancelled_error(&executable_path, Some(process_id)));
            }
        }

        match child.try_wait() {
            Ok(Some(status)) => {
                if let Some(tracking) = &tracking {
                    tracking.registry.unregister(
                        &tracking.run_id,
                        &tracking.action_keys,
                        process_id,
                    );
                }
                if status.success() {
                    return Ok(ProcessLaunchResult {
                        executable_path: executable_path.display().to_string(),
                        duration_ms: launched_at.elapsed().as_millis() as i64,
                    });
                }
                return Err(AppError::new(
                    "PROCESS_EXITED_WITH_ERROR",
                    "external command exited with an error",
                )
                .with_detail("executablePath", executable_path.display().to_string())
                .with_detail(
                    "exitCode",
                    status
                        .code()
                        .map(|code| code.to_string())
                        .unwrap_or_else(|| "unknown".to_string()),
                ));
            }
            Ok(None) => {
                if launched_at.elapsed() >= command.timeout {
                    if !command.retain_after_timeout {
                        if let Some(tracking) = &tracking {
                            tracking.registry.unregister(
                                &tracking.run_id,
                                &tracking.action_keys,
                                process_id,
                            );
                        }
                    }
                    return Ok(ProcessLaunchResult {
                        executable_path: executable_path.display().to_string(),
                        duration_ms: launched_at.elapsed().as_millis() as i64,
                    });
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(error) => {
                if let Some(tracking) = &tracking {
                    tracking.registry.unregister(
                        &tracking.run_id,
                        &tracking.action_keys,
                        process_id,
                    );
                }
                return Err(AppError::new(
                    "PROCESS_WAIT_FAILED",
                    "failed while waiting for external command",
                )
                .with_detail("executablePath", executable_path.display().to_string())
                .with_detail("reason", error.to_string()));
            }
        }
    }
}

fn resolve_program(executable_path: &Path) -> (PathBuf, Vec<String>) {
    let extension = executable_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());

    if matches!(extension.as_deref(), Some("cmd" | "bat")) {
        return (
            PathBuf::from("cmd.exe"),
            vec!["/C".to_string(), executable_path.display().to_string()],
        );
    }

    (executable_path.to_path_buf(), Vec::new())
}

fn process_cancelled_error(executable_path: &Path, process_id: Option<u32>) -> AppError {
    let mut error = AppError::new("PROCESS_CANCELLED", "external command was cancelled")
        .with_detail("executablePath", executable_path.display().to_string());
    if let Some(process_id) = process_id {
        error = error.with_detail("processId", process_id.to_string());
    }
    error
}

fn terminate_process_tree(pid: u32) -> AppResult<bool> {
    let output = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| {
            AppError::new(
                "PROCESS_KILL_FAILED",
                "failed to terminate external command",
            )
            .with_detail("processId", pid.to_string())
            .with_detail("reason", error.to_string())
        })?;

    if output.status.success() {
        return Ok(true);
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
    let stderr = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
    if stdout.contains("not found")
        || stdout.contains("no running instance")
        || stderr.contains("not found")
        || stderr.contains("no running instance")
    {
        return Ok(false);
    }

    Err(
        AppError::new("PROCESS_KILL_FAILED", "taskkill exited with an error")
            .with_detail("processId", pid.to_string())
            .with_detail(
                "exitCode",
                output
                    .status
                    .code()
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
            )
            .with_detail("stdout", stdout)
            .with_detail("stderr", stderr),
    )
}

fn unique_processes<I>(tracked: I) -> HashMap<u32, TrackedChildProcess>
where
    I: IntoIterator<Item = TrackedChildProcess>,
{
    let mut unique = HashMap::new();
    for process in tracked {
        unique.entry(process.pid).or_insert(process);
    }
    unique
}

fn count_unique_processes(
    tracked: Option<&HashMap<ActionProcessKey, Vec<TrackedChildProcess>>>,
) -> usize {
    let Some(tracked) = tracked else {
        return 0;
    };

    let mut process_ids = HashSet::new();
    for processes in tracked.values() {
        for process in processes {
            process_ids.insert(process.pid);
        }
    }
    process_ids.len()
}
