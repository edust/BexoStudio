use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use chrono::Utc;
use tauri::{AppHandle, Emitter};

use crate::{
    adapters::{
        run_launch_command, ActionProcessKey, ChildProcessRegistry, CodexAdapter, CodexLaunchInput,
        DefaultCodexAdapter, IdeAdapter, JetBrainsAdapter, LaunchCommand, ProcessLaunchResult,
        ProcessTrackingContext, TerminalAdapter, TerminalLaunchInput, VSCodeAdapter,
        WindowsTerminalAdapter,
    },
    domain::{
        ensure_absolute_directory, AppPreferences, CancelRestoreActionResult,
        CancelRestoreRunResult, OpenLogDirectoryResult, RecentRestoreTarget, RestoreActionPlan,
        RestoreCapabilities, RestorePreviewStats, RestoreProjectPlan, RestoreRunDetail,
        RestoreRunEvent, RestoreRunProjectRecord, RestoreRunSummary, SnapshotLaunchTaskPayload,
        SnapshotProjectPayload, SnapshotRecord, StartRestoreRunInput, RESTORE_RUN_EVENT_NAME,
    },
    error::{AppError, AppResult},
    logging::RestoreLogStore,
    persistence::{
        finalize_restore_run, get_restore_run_summary, get_snapshot, insert_restore_run_plan,
        list_restore_runs as persist_list_restore_runs, list_snapshots as persist_list_snapshots,
        recover_interrupted_restore_runs, update_restore_run_status, update_restore_run_task,
        Database,
    },
};

use super::{
    planner_service::{build_restore_preview, build_restore_run_detail},
    PreferencesService,
};

#[derive(Debug, Clone)]
pub struct RestoreService {
    database: Database,
    restore_log_store: RestoreLogStore,
    active_runs: Arc<Mutex<HashMap<String, Arc<ActiveRestoreRun>>>>,
    child_process_registry: ChildProcessRegistry,
}

#[derive(Debug)]
struct ActiveRestoreRun {
    run_id: String,
    workspace_id: String,
    snapshot_id: String,
    app_handle: Option<AppHandle>,
    cancel_requested: Arc<AtomicBool>,
    actions: Arc<Mutex<HashMap<ActionProcessKey, ActiveActionRuntime>>>,
}

#[derive(Debug, Clone)]
struct ActiveActionRuntime {
    action: RestoreActionPlan,
    cancel_token: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
enum ActionCancelRequestResult {
    CancelRequested {
        requested_now: bool,
        action: RestoreActionPlan,
    },
    AlreadyFinished,
    NotFound,
}

impl ActiveRestoreRun {
    fn new(app_handle: Option<&AppHandle>, run_id: String, snapshot: &SnapshotRecord) -> Self {
        Self {
            run_id,
            workspace_id: snapshot.workspace_id.clone(),
            snapshot_id: snapshot.id.clone(),
            app_handle: app_handle.cloned(),
            cancel_requested: Arc::new(AtomicBool::new(false)),
            actions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn emit(&self, event: RestoreRunEvent) {
        let Some(app_handle) = &self.app_handle else {
            return;
        };

        if let Err(error) = app_handle.emit(RESTORE_RUN_EVENT_NAME, event) {
            log::error!(
                target: "bexo::service::restore",
                "failed to emit restore run event: {}",
                error
            );
        }
    }

    fn request_cancel(&self) -> bool {
        !self.cancel_requested.swap(true, Ordering::SeqCst)
    }

    fn is_cancel_requested(&self) -> bool {
        self.cancel_requested.load(Ordering::SeqCst)
    }

    fn cancel_token(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancel_requested)
    }

    fn register_project_actions(&self, project_task_id: &str, actions: &[RestoreActionPlan]) {
        let mut registry = self
            .actions
            .lock()
            .expect("active action registry poisoned");
        for action in actions {
            let key = action_key(project_task_id, &action.id);
            let cancel_token = registry
                .get(&key)
                .map(|runtime| Arc::clone(&runtime.cancel_token))
                .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
            registry.insert(
                key,
                ActiveActionRuntime {
                    action: action.clone(),
                    cancel_token,
                },
            );
        }
    }

    fn sync_action(&self, project_task_id: &str, action: &RestoreActionPlan) {
        let mut registry = self
            .actions
            .lock()
            .expect("active action registry poisoned");
        let key = action_key(project_task_id, &action.id);
        let cancel_token = registry
            .get(&key)
            .map(|runtime| Arc::clone(&runtime.cancel_token))
            .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
        let mut merged_action = action.clone();
        if let Some(existing_runtime) = registry.get(&key) {
            merge_action_runtime_fields(&mut merged_action, &existing_runtime.action);
        }
        registry.insert(
            key,
            ActiveActionRuntime {
                action: merged_action,
                cancel_token,
            },
        );
    }

    fn merge_project_actions(&self, project_task_id: &str, actions: &mut [RestoreActionPlan]) {
        let registry = self
            .actions
            .lock()
            .expect("active action registry poisoned");
        for action in actions {
            if let Some(existing_runtime) = registry.get(&action_key(project_task_id, &action.id)) {
                merge_action_runtime_fields(action, &existing_runtime.action);
            }
        }
    }

    fn action_cancel_tokens(
        &self,
        project_task_id: &str,
        action_ids: &[String],
    ) -> Vec<Arc<AtomicBool>> {
        let registry = self
            .actions
            .lock()
            .expect("active action registry poisoned");
        action_ids
            .iter()
            .filter_map(|action_id| {
                registry
                    .get(&action_key(project_task_id, action_id))
                    .map(|runtime| Arc::clone(&runtime.cancel_token))
            })
            .collect()
    }

    fn is_action_cancel_requested(&self, project_task_id: &str, action_id: &str) -> bool {
        self.actions
            .lock()
            .expect("active action registry poisoned")
            .get(&action_key(project_task_id, action_id))
            .map(|runtime| runtime.cancel_token.load(Ordering::SeqCst))
            .unwrap_or(false)
    }

    fn request_action_cancel(&self, action_key: &ActionProcessKey) -> ActionCancelRequestResult {
        let mut registry = self
            .actions
            .lock()
            .expect("active action registry poisoned");
        let Some(runtime) = registry.get_mut(action_key) else {
            return ActionCancelRequestResult::NotFound;
        };

        if is_finished_action_status(&runtime.action.status) {
            return ActionCancelRequestResult::AlreadyFinished;
        }

        let requested_now = !runtime.cancel_token.swap(true, Ordering::SeqCst);
        if runtime.action.cancel_requested_at.is_none() {
            runtime.action.cancel_requested_at = Some(Utc::now().to_rfc3339());
        }
        if !matches!(runtime.action.status.as_str(), "cancelled" | "skipped") {
            runtime.action.status = "cancel_requested".to_string();
            runtime.action.reason = Some(action_cancel_requested_reason(&runtime.action.label));
        }

        ActionCancelRequestResult::CancelRequested {
            requested_now,
            action: runtime.action.clone(),
        }
    }
}

#[derive(Debug, Clone)]
struct RestoreRuntimeContext {
    active_run: Arc<ActiveRestoreRun>,
    run_id: String,
    workspace_id: String,
    snapshot_id: String,
}

impl RestoreRuntimeContext {
    fn new(app_handle: Option<&AppHandle>, run_id: String, snapshot: &SnapshotRecord) -> Self {
        let active_run = Arc::new(ActiveRestoreRun::new(app_handle, run_id.clone(), snapshot));
        Self {
            active_run,
            run_id,
            workspace_id: snapshot.workspace_id.clone(),
            snapshot_id: snapshot.id.clone(),
        }
    }

    fn emit(&self, event: RestoreRunEvent) {
        self.active_run.emit(event);
    }

    fn is_cancel_requested(&self) -> bool {
        self.active_run.is_cancel_requested()
    }

    fn register_project_actions(&self, project_task_id: &str, actions: &[RestoreActionPlan]) {
        self.active_run
            .register_project_actions(project_task_id, actions);
    }

    fn sync_action(&self, project_task_id: &str, action: &RestoreActionPlan) {
        self.active_run.sync_action(project_task_id, action);
    }

    fn merge_project_actions(&self, project_task_id: &str, actions: &mut [RestoreActionPlan]) {
        self.active_run
            .merge_project_actions(project_task_id, actions);
    }

    fn is_action_cancel_requested(&self, project_task_id: &str, action_id: &str) -> bool {
        self.active_run
            .is_action_cancel_requested(project_task_id, action_id)
    }

    fn action_cancel_tokens(
        &self,
        project_task_id: &str,
        action_ids: &[String],
    ) -> Vec<Arc<AtomicBool>> {
        self.active_run
            .action_cancel_tokens(project_task_id, action_ids)
    }
}

#[derive(Debug)]
struct ActiveRunGuard {
    active_runs: Arc<Mutex<HashMap<String, Arc<ActiveRestoreRun>>>>,
    run_id: String,
}

impl ActiveRunGuard {
    fn new(
        active_runs: Arc<Mutex<HashMap<String, Arc<ActiveRestoreRun>>>>,
        active_run: Arc<ActiveRestoreRun>,
    ) -> Self {
        active_runs
            .lock()
            .expect("active restore runs poisoned")
            .insert(active_run.run_id.clone(), active_run.clone());
        Self {
            active_runs,
            run_id: active_run.run_id.clone(),
        }
    }
}

impl Drop for ActiveRunGuard {
    fn drop(&mut self) {
        self.active_runs
            .lock()
            .expect("active restore runs poisoned")
            .remove(&self.run_id);
    }
}

impl RestoreService {
    pub fn new(database: Database, restore_log_store: RestoreLogStore) -> Self {
        Self {
            database,
            restore_log_store,
            active_runs: Arc::new(Mutex::new(HashMap::new())),
            child_process_registry: ChildProcessRegistry::default(),
        }
    }

    pub async fn recover_interrupted_runs(&self) -> AppResult<Vec<String>> {
        self.database
            .write(
                "recover_interrupted_restore_runs",
                recover_interrupted_restore_runs,
            )
            .await
    }

    pub async fn get_restore_capabilities(
        &self,
        preferences_service: &PreferencesService,
    ) -> AppResult<RestoreCapabilities> {
        let preferences = preferences_service.get_preferences()?;
        Ok(probe_restore_capabilities(&preferences))
    }

    #[allow(dead_code)]
    pub async fn start_restore_run(
        &self,
        input: StartRestoreRunInput,
        preferences_service: &PreferencesService,
    ) -> AppResult<RestoreRunDetail> {
        self.start_restore_run_with_events(None, input, preferences_service)
            .await
    }

    pub async fn cancel_restore_run(
        &self,
        app_handle: Option<&AppHandle>,
        run_id: String,
    ) -> AppResult<CancelRestoreRunResult> {
        let normalized_run_id = run_id.trim().to_string();
        if normalized_run_id.is_empty() {
            return Err(AppError::validation("runId is required"));
        }

        if let Some(active_run) = self.active_run(&normalized_run_id) {
            let requested_now = active_run.request_cancel();
            let terminated_process_count = self
                .child_process_registry
                .terminate_run_processes(&normalized_run_id)
                .await;

            let event_handle = app_handle
                .cloned()
                .or_else(|| active_run.app_handle.clone());
            if requested_now {
                let summary = self
                    .database
                    .read("get_restore_run_summary_for_cancel_event", {
                        let run_id = normalized_run_id.clone();
                        move |connection| get_restore_run_summary(connection, run_id)
                    })
                    .await
                    .ok();
                let event = RestoreRunEvent {
                    event_type: "run_cancel_requested".to_string(),
                    run_id: active_run.run_id.clone(),
                    workspace_id: active_run.workspace_id.clone(),
                    snapshot_id: Some(active_run.snapshot_id.clone()),
                    project_id: None,
                    project_task_id: None,
                    launch_task_id: None,
                    status: Some("cancel_requested".to_string()),
                    message: Some("已请求取消当前恢复批次".to_string()),
                    occurred_at: Utc::now().to_rfc3339(),
                    run: summary,
                    project: None,
                    action: None,
                    stats: None,
                };

                if let Some(handle) = event_handle {
                    if let Err(error) = handle.emit(RESTORE_RUN_EVENT_NAME, event) {
                        log::error!(
                            target: "bexo::service::restore",
                            "failed to emit cancel request event for run {}: {}",
                            normalized_run_id,
                            error
                        );
                    }
                }
            }

            self.database
                .write("mark_restore_run_cancel_requested", {
                    let run_id = normalized_run_id.clone();
                    move |connection| {
                        update_restore_run_status(
                            connection,
                            &run_id,
                            "cancel_requested",
                            Some("已请求取消当前恢复批次"),
                        )
                    }
                })
                .await?;

            return Ok(CancelRestoreRunResult {
                cancelled: true,
                status: "cancel_requested".to_string(),
                terminated_process_count: terminated_process_count as i64,
            });
        }

        let summary = match self
            .database
            .read("get_restore_run_summary_for_cancel", {
                let run_id = normalized_run_id.clone();
                move |connection| get_restore_run_summary(connection, run_id)
            })
            .await
        {
            Ok(summary) => summary,
            Err(error) if error.code == "RESTORE_RUN_NOT_FOUND" => {
                return Ok(CancelRestoreRunResult {
                    cancelled: false,
                    status: "not_found".to_string(),
                    terminated_process_count: 0,
                });
            }
            Err(error) => return Err(error),
        };

        let status = if matches!(
            summary.status.as_str(),
            "completed"
                | "completed_with_blocks"
                | "completed_with_warnings"
                | "failed"
                | "blocked"
                | "skipped"
                | "cancelled"
        ) {
            "already_finished"
        } else {
            "cancel_requested"
        };

        Ok(CancelRestoreRunResult {
            cancelled: status == "cancel_requested",
            status: status.to_string(),
            terminated_process_count: 0,
        })
    }

    pub async fn cancel_restore_action(
        &self,
        app_handle: Option<&AppHandle>,
        run_id: String,
        project_task_id: String,
        action_id: String,
    ) -> AppResult<CancelRestoreActionResult> {
        let normalized_run_id = run_id.trim().to_string();
        let normalized_project_task_id = project_task_id.trim().to_string();
        let normalized_action_id = action_id.trim().to_string();

        if normalized_run_id.is_empty() {
            return Err(AppError::validation("runId is required"));
        }
        if normalized_project_task_id.is_empty() {
            return Err(AppError::validation("projectTaskId is required"));
        }
        if normalized_action_id.is_empty() {
            return Err(AppError::validation("actionId is required"));
        }

        let process_key = action_key(&normalized_project_task_id, &normalized_action_id);
        if let Some(active_run) = self.active_run(&normalized_run_id) {
            match active_run.request_action_cancel(&process_key) {
                ActionCancelRequestResult::CancelRequested {
                    requested_now,
                    action,
                } => {
                    let tracked_process_count = self
                        .child_process_registry
                        .count_action_processes(&normalized_run_id, &process_key);
                    let terminated_process_count = self
                        .child_process_registry
                        .terminate_action_processes(&normalized_run_id, &process_key)
                        .await;

                    if requested_now {
                        let event_handle = app_handle
                            .cloned()
                            .or_else(|| active_run.app_handle.clone());
                        if let Some(handle) = event_handle {
                            let event = RestoreRunEvent {
                                event_type: "action_cancel_requested".to_string(),
                                run_id: normalized_run_id.clone(),
                                workspace_id: active_run.workspace_id.clone(),
                                snapshot_id: Some(active_run.snapshot_id.clone()),
                                project_id: None,
                                project_task_id: Some(normalized_project_task_id.clone()),
                                launch_task_id: action.launch_task_id.clone(),
                                status: Some(action.status.clone()),
                                message: Some(action_cancel_requested_message(
                                    &action.label,
                                    tracked_process_count,
                                )),
                                occurred_at: Utc::now().to_rfc3339(),
                                run: None,
                                project: None,
                                action: Some(action),
                                stats: None,
                            };

                            if let Err(error) = handle.emit(RESTORE_RUN_EVENT_NAME, event) {
                                log::error!(
                                    target: "bexo::service::restore",
                                    "failed to emit action cancel request event for run {} project task {} action {}: {}",
                                    normalized_run_id,
                                    normalized_project_task_id,
                                    normalized_action_id,
                                    error
                                );
                            }
                        }
                    }

                    return Ok(CancelRestoreActionResult {
                        cancelled: true,
                        status: "cancel_requested".to_string(),
                        terminated_process_count: terminated_process_count as i64,
                        run_id: normalized_run_id,
                        project_task_id: normalized_project_task_id,
                        action_id: normalized_action_id,
                    });
                }
                ActionCancelRequestResult::AlreadyFinished => {
                    return Ok(CancelRestoreActionResult {
                        cancelled: false,
                        status: "already_finished".to_string(),
                        terminated_process_count: 0,
                        run_id: normalized_run_id,
                        project_task_id: normalized_project_task_id,
                        action_id: normalized_action_id,
                    });
                }
                ActionCancelRequestResult::NotFound => {}
            }
        }

        let detail = match self.get_restore_run_detail(normalized_run_id.clone()).await {
            Ok(detail) => detail,
            Err(error) if error.code == "RESTORE_RUN_NOT_FOUND" => {
                return Ok(CancelRestoreActionResult {
                    cancelled: false,
                    status: "not_found".to_string(),
                    terminated_process_count: 0,
                    run_id: normalized_run_id,
                    project_task_id: normalized_project_task_id,
                    action_id: normalized_action_id,
                });
            }
            Err(error) => return Err(error),
        };

        let Some(action) =
            find_restore_action(&detail, &normalized_project_task_id, &normalized_action_id)
        else {
            return Ok(CancelRestoreActionResult {
                cancelled: false,
                status: "not_found".to_string(),
                terminated_process_count: 0,
                run_id: normalized_run_id,
                project_task_id: normalized_project_task_id,
                action_id: normalized_action_id,
            });
        };

        let status = if is_finished_action_status(&action.status)
            || is_finished_run_status(&detail.run.status)
        {
            "already_finished"
        } else {
            "not_found"
        };

        Ok(CancelRestoreActionResult {
            cancelled: false,
            status: status.to_string(),
            terminated_process_count: 0,
            run_id: normalized_run_id,
            project_task_id: normalized_project_task_id,
            action_id: normalized_action_id,
        })
    }

    pub async fn start_restore_run_with_events(
        &self,
        app_handle: Option<&AppHandle>,
        input: StartRestoreRunInput,
        preferences_service: &PreferencesService,
    ) -> AppResult<RestoreRunDetail> {
        let preferences = preferences_service.get_preferences()?;
        let snapshot = self
            .database
            .read("load_snapshot_for_restore", move |connection| {
                get_snapshot(connection, input.snapshot_id.clone())
            })
            .await?;
        let preview = build_restore_preview(snapshot.clone(), input.mode.clone())?;
        let capabilities = probe_restore_capabilities(&preferences);

        let snapshot_for_insert = snapshot.clone();
        let mode_for_insert = preview.mode.clone();
        let projects_for_insert = preview.projects.clone();
        let (run_id, task_rows) = self
            .database
            .write("insert_restore_run_plan", move |connection| {
                insert_restore_run_plan(
                    connection,
                    &snapshot_for_insert,
                    &mode_for_insert,
                    &projects_for_insert,
                )
            })
            .await?;

        let runtime_context = RestoreRuntimeContext::new(app_handle, run_id.clone(), &snapshot);
        let _active_run_guard = ActiveRunGuard::new(
            Arc::clone(&self.active_runs),
            Arc::clone(&runtime_context.active_run),
        );
        let initial_run_summary = self
            .database
            .read("get_restore_run_summary", {
                let run_id = run_id.clone();
                move |connection| get_restore_run_summary(connection, run_id)
            })
            .await?;
        runtime_context.emit(RestoreRunEvent {
            event_type: "run_started".to_string(),
            run_id: runtime_context.run_id.clone(),
            workspace_id: runtime_context.workspace_id.clone(),
            snapshot_id: Some(runtime_context.snapshot_id.clone()),
            project_id: None,
            project_task_id: None,
            launch_task_id: None,
            status: Some(initial_run_summary.status.clone()),
            message: Some("恢复批次已进入执行阶段".to_string()),
            occurred_at: Utc::now().to_rfc3339(),
            run: Some(initial_run_summary),
            project: None,
            action: None,
            stats: Some(preview.stats.clone()),
        });

        let project_task_ids = task_rows
            .iter()
            .filter(|task| task.launch_task_id.is_none())
            .filter_map(|task| {
                task.project_id
                    .as_ref()
                    .map(|project_id| (project_id.clone(), task.id.clone()))
            })
            .collect::<HashMap<_, _>>();
        let launch_task_row_ids = task_rows
            .iter()
            .filter_map(|task| {
                task.launch_task_id
                    .as_ref()
                    .map(|launch_task_id| (launch_task_id.clone(), task.id.clone()))
            })
            .collect::<HashMap<_, _>>();
        let snapshot_projects = snapshot
            .payload
            .projects
            .iter()
            .map(|project| (project.id.clone(), project.clone()))
            .collect::<HashMap<_, _>>();

        for project in &preview.projects {
            let task_id = project_task_ids.get(&project.project_id).ok_or_else(|| {
                AppError::new(
                    "RESTORE_RUN_TASK_NOT_FOUND",
                    "restore run task was not found",
                )
                .with_detail("projectId", project.project_id.clone())
                .with_detail("restoreRunId", run_id.clone())
            })?;
            runtime_context.register_project_actions(task_id, &project.actions);
        }

        let mut runtime_tasks = Vec::with_capacity(preview.projects.len());
        for project in &preview.projects {
            if runtime_context.is_cancel_requested() {
                let skipped_task = self
                    .build_cancelled_pending_project(
                        &runtime_context,
                        project,
                        project_task_ids
                            .get(&project.project_id)
                            .ok_or_else(|| {
                                AppError::new(
                                    "RESTORE_RUN_TASK_NOT_FOUND",
                                    "restore run task was not found",
                                )
                                .with_detail("projectId", project.project_id.clone())
                                .with_detail("restoreRunId", run_id.clone())
                            })?
                            .clone(),
                        &launch_task_row_ids,
                    )
                    .await?;
                runtime_tasks.push(skipped_task);
                continue;
            }

            let snapshot_project = snapshot_projects.get(&project.project_id).ok_or_else(|| {
                AppError::new("PROJECT_NOT_FOUND", "snapshot project was not found")
                    .with_detail("projectId", project.project_id.clone())
            })?;
            let task_id = project_task_ids.get(&project.project_id).ok_or_else(|| {
                AppError::new(
                    "RESTORE_RUN_TASK_NOT_FOUND",
                    "restore run task was not found",
                )
                .with_detail("projectId", project.project_id.clone())
                .with_detail("restoreRunId", run_id.clone())
            })?;

            let runtime_task = self
                .execute_project(
                    snapshot_project.clone(),
                    &runtime_context,
                    task_id.clone(),
                    &launch_task_row_ids,
                    project.clone(),
                    &capabilities,
                )
                .await
                .inspect_err(|error| {
                    log::error!(
                        target: "bexo::service::restore",
                        "execute_project failed for project {}: {}",
                        project.project_id,
                        error
                    );
                });
            let runtime_task = match runtime_task {
                Ok(task) => task,
                Err(error) => {
                    self.child_process_registry
                        .clear_run(&runtime_context.run_id);
                    let snapshot_for_finalize = snapshot.clone();
                    let run_id_for_finalize = run_id.clone();
                    let failure_summary = format!("{}: {}", error.code, error.message);
                    let failure_summary_for_finalize = failure_summary.clone();
                    let _ = self
                        .database
                        .write("finalize_failed_restore_run", move |connection| {
                            finalize_restore_run(
                                connection,
                                &snapshot_for_finalize,
                                &run_id_for_finalize,
                                "failed",
                                Some(failure_summary_for_finalize.as_str()),
                            )
                        })
                        .await;
                    runtime_context.emit(RestoreRunEvent {
                        event_type: "run_finished".to_string(),
                        run_id: runtime_context.run_id.clone(),
                        workspace_id: runtime_context.workspace_id.clone(),
                        snapshot_id: Some(runtime_context.snapshot_id.clone()),
                        project_id: Some(project.project_id.clone()),
                        project_task_id: Some(task_id.clone()),
                        launch_task_id: None,
                        status: Some("failed".to_string()),
                        message: Some(failure_summary),
                        occurred_at: Utc::now().to_rfc3339(),
                        run: None,
                        project: None,
                        action: None,
                        stats: None,
                    });
                    return Err(error);
                }
            };
            runtime_tasks.push(runtime_task);
        }

        let (run_status, error_summary) = if runtime_context.is_cancel_requested() {
            (
                "cancelled".to_string(),
                Some("恢复批次已被用户取消".to_string()),
            )
        } else {
            summarize_run_status(&runtime_tasks)
        };
        let snapshot_for_finalize = snapshot.clone();
        let run_id_for_finalize = run_id.clone();
        let status_for_finalize = run_status.clone();
        let summary_for_finalize = error_summary.clone();
        self.database
            .write("finalize_restore_run", move |connection| {
                finalize_restore_run(
                    connection,
                    &snapshot_for_finalize,
                    &run_id_for_finalize,
                    &status_for_finalize,
                    summary_for_finalize.as_deref(),
                )
            })
            .await?;

        let run_summary = self
            .database
            .read("get_restore_run_summary", move |connection| {
                get_restore_run_summary(connection, run_id)
            })
            .await?;
        let detail = RestoreRunDetail {
            run: run_summary.clone(),
            snapshot,
            stats: build_runtime_stats(&runtime_tasks),
            tasks: runtime_tasks,
        };

        if let Err(error) = self
            .restore_log_store
            .write_run_detail(detail.clone())
            .await
        {
            log::error!(
                target: "bexo::service::restore",
                "failed to write restore run detail log: {}",
                error
            );
        }

        if run_status == "cancelled" {
            let terminated = self
                .child_process_registry
                .terminate_run_processes(&runtime_context.run_id)
                .await;
            log::info!(
                target: "bexo::service::restore",
                "restore run {} cancelled, terminate_run_processes attempted on {} process(es)",
                runtime_context.run_id,
                terminated
            );
        }
        self.child_process_registry
            .clear_run(&runtime_context.run_id);

        runtime_context.emit(RestoreRunEvent {
            event_type: "run_finished".to_string(),
            run_id: runtime_context.run_id.clone(),
            workspace_id: runtime_context.workspace_id.clone(),
            snapshot_id: Some(runtime_context.snapshot_id.clone()),
            project_id: None,
            project_task_id: None,
            launch_task_id: None,
            status: Some(run_summary.status.clone()),
            message: run_summary.error_summary.clone(),
            occurred_at: Utc::now().to_rfc3339(),
            run: Some(run_summary),
            project: None,
            action: None,
            stats: Some(detail.stats.clone()),
        });

        Ok(detail)
    }

    pub async fn list_recent_restore_targets(&self) -> AppResult<Vec<RecentRestoreTarget>> {
        self.database
            .read("list_recent_restore_targets", |connection| {
                let mut snapshots = persist_list_snapshots(connection, None)?;
                snapshots.sort_by(|left, right| {
                    recent_target_sort_key(right)
                        .cmp(recent_target_sort_key(left))
                        .then_with(|| right.updated_at.cmp(&left.updated_at))
                });

                let mut seen_workspaces = HashSet::new();
                let mut targets = Vec::new();
                for snapshot in snapshots {
                    if !seen_workspaces.insert(snapshot.workspace_id.clone()) {
                        continue;
                    }

                    targets.push(RecentRestoreTarget {
                        id: snapshot.id.clone(),
                        workspace_id: snapshot.workspace_id.clone(),
                        workspace_name: snapshot.workspace_name.clone(),
                        snapshot_id: snapshot.id.clone(),
                        snapshot_name: snapshot.name.clone(),
                        project_count: snapshot.project_count,
                        snapshot_updated_at: snapshot.updated_at.clone(),
                        last_restore_at: snapshot.last_restore_at.clone(),
                        last_restore_status: snapshot.last_restore_status.clone(),
                    });

                    if targets.len() >= 5 {
                        break;
                    }
                }

                Ok(targets)
            })
            .await
    }

    pub async fn restore_recent_target(
        &self,
        id: String,
        mode: Option<String>,
        preferences_service: &PreferencesService,
    ) -> AppResult<RestoreRunDetail> {
        self.restore_recent_target_with_events(None, id, mode, preferences_service)
            .await
    }

    pub async fn restore_recent_target_with_events(
        &self,
        app_handle: Option<&AppHandle>,
        id: String,
        mode: Option<String>,
        preferences_service: &PreferencesService,
    ) -> AppResult<RestoreRunDetail> {
        self.start_restore_run_with_events(
            app_handle,
            StartRestoreRunInput {
                snapshot_id: id,
                mode: mode.unwrap_or_else(|| "full".to_string()),
            },
            preferences_service,
        )
        .await
    }

    pub async fn list_restore_runs(&self) -> AppResult<Vec<RestoreRunSummary>> {
        self.database
            .read("list_restore_runs", persist_list_restore_runs)
            .await
    }

    pub async fn get_restore_run_detail(&self, id: String) -> AppResult<RestoreRunDetail> {
        if let Some(detail) = self.restore_log_store.read_run_detail(id.clone()).await? {
            return Ok(detail);
        }

        self.database
            .read("get_restore_run_detail", move |connection| {
                build_restore_run_detail(connection, id)
            })
            .await
    }

    pub async fn open_log_directory(&self) -> AppResult<OpenLogDirectoryResult> {
        let log_dir = self.restore_log_store.ensure_log_dir().await?;
        let result = run_launch_command(LaunchCommand {
            executable_path: PathBuf::from("explorer.exe"),
            args: vec![log_dir.display().to_string()],
            current_dir: None,
            envs: Vec::new(),
            timeout: Duration::from_millis(900),
            tracking: None,
            retain_after_timeout: false,
        })
        .await;

        if let Err(error) = result {
            return Err(
                AppError::new("OPEN_PATH_FAILED", "failed to open restore log directory")
                    .with_detail("path", log_dir.display().to_string())
                    .with_detail("reason", error.to_string()),
            );
        }

        Ok(OpenLogDirectoryResult {
            path: log_dir.display().to_string(),
        })
    }

    async fn execute_project(
        &self,
        snapshot_project: SnapshotProjectPayload,
        runtime_context: &RestoreRuntimeContext,
        task_id: String,
        launch_task_row_ids: &HashMap<String, String>,
        planned_project: RestoreProjectPlan,
        capabilities: &RestoreCapabilities,
    ) -> AppResult<RestoreRunProjectRecord> {
        let mut actions = planned_project.actions.clone();

        if matches!(planned_project.status.as_str(), "blocked" | "skipped") {
            let record = RestoreRunProjectRecord {
                id: task_id,
                restore_run_id: runtime_context.run_id.clone(),
                project_id: Some(planned_project.project_id.clone()),
                project_name: planned_project.project_name,
                path: planned_project.path,
                status: planned_project.status.clone(),
                attempt_count: 0,
                started_at: None,
                finished_at: None,
                error_message: planned_project.reason,
                actions,
            };
            for action in &record.actions {
                runtime_context.sync_action(&record.id, action);
            }
            runtime_context.emit(RestoreRunEvent {
                event_type: "project_finished".to_string(),
                run_id: runtime_context.run_id.clone(),
                workspace_id: runtime_context.workspace_id.clone(),
                snapshot_id: Some(runtime_context.snapshot_id.clone()),
                project_id: Some(planned_project.project_id),
                project_task_id: Some(record.id.clone()),
                launch_task_id: None,
                status: Some(record.status.clone()),
                message: record.error_message.clone(),
                occurred_at: Utc::now().to_rfc3339(),
                run: None,
                project: Some(record.clone()),
                action: None,
                stats: None,
            });
            return Ok(record);
        }

        let started_at = Utc::now().to_rfc3339();
        let task_id_for_update = task_id.clone();
        let started_at_for_update = started_at.clone();
        self.database
            .write("mark_restore_task_running", move |connection| {
                update_restore_run_task(
                    connection,
                    &task_id_for_update,
                    "running",
                    1,
                    Some(started_at_for_update.as_str()),
                    None,
                    None,
                )
            })
            .await?;

        runtime_context.emit(RestoreRunEvent {
            event_type: "project_started".to_string(),
            run_id: runtime_context.run_id.clone(),
            workspace_id: runtime_context.workspace_id.clone(),
            snapshot_id: Some(runtime_context.snapshot_id.clone()),
            project_id: Some(planned_project.project_id.clone()),
            project_task_id: Some(task_id.clone()),
            launch_task_id: None,
            status: Some("running".to_string()),
            message: Some(format!("开始恢复项目 {}", planned_project.project_name)),
            occurred_at: Utc::now().to_rfc3339(),
            run: None,
            project: Some(RestoreRunProjectRecord {
                id: task_id.clone(),
                restore_run_id: runtime_context.run_id.clone(),
                project_id: Some(planned_project.project_id.clone()),
                project_name: planned_project.project_name.clone(),
                path: planned_project.path.clone(),
                status: "running".to_string(),
                attempt_count: 1,
                started_at: Some(started_at.clone()),
                finished_at: None,
                error_message: None,
                actions: actions.clone(),
            }),
            action: None,
            stats: None,
        });

        self.execute_terminal_actions(
            &snapshot_project,
            runtime_context,
            &task_id,
            capabilities,
            &mut actions,
        )
        .await;
        let mut halt_reason = if runtime_context.is_cancel_requested() {
            Some(cancel_reason())
        } else {
            None
        };
        let launch_task_halt_reason = self
            .execute_launch_task_actions(
                &snapshot_project,
                runtime_context,
                &task_id,
                launch_task_row_ids,
                capabilities,
                &mut actions,
            )
            .await?;
        if halt_reason.is_none() {
            halt_reason = launch_task_halt_reason;
        }
        if let Some(reason) = halt_reason.clone() {
            skip_pending_actions(&mut actions, &reason);
        } else {
            self.execute_ide_actions(
                &snapshot_project,
                runtime_context,
                &task_id,
                capabilities,
                &mut actions,
            )
            .await;
        }

        runtime_context.merge_project_actions(&task_id, &mut actions);

        let finished_at = Utc::now().to_rfc3339();
        let (mut status, mut error_message) = summarize_project_actions(&actions);
        if runtime_context.is_cancel_requested() && !matches!(status.as_str(), "failed" | "blocked")
        {
            status = "cancelled".to_string();
            error_message = Some(cancel_reason());
        }

        let task_id_for_finalize = task_id.clone();
        let status_for_finalize = status.clone();
        let finished_at_for_finalize = finished_at.clone();
        let error_for_finalize = error_message.clone();
        let started_at_for_finalize = started_at.clone();
        self.database
            .write("finalize_restore_task", move |connection| {
                update_restore_run_task(
                    connection,
                    &task_id_for_finalize,
                    &status_for_finalize,
                    1,
                    Some(started_at_for_finalize.as_str()),
                    Some(finished_at_for_finalize.as_str()),
                    error_for_finalize.as_deref(),
                )
            })
            .await?;

        let record = RestoreRunProjectRecord {
            id: task_id,
            restore_run_id: runtime_context.run_id.clone(),
            project_id: Some(planned_project.project_id.clone()),
            project_name: planned_project.project_name,
            path: planned_project.path,
            status: status.clone(),
            attempt_count: 1,
            started_at: Some(started_at),
            finished_at: Some(finished_at),
            error_message: error_message.clone(),
            actions,
        };
        for action in &record.actions {
            runtime_context.sync_action(&record.id, action);
        }
        runtime_context.emit(RestoreRunEvent {
            event_type: "project_finished".to_string(),
            run_id: runtime_context.run_id.clone(),
            workspace_id: runtime_context.workspace_id.clone(),
            snapshot_id: Some(runtime_context.snapshot_id.clone()),
            project_id: Some(planned_project.project_id),
            project_task_id: Some(record.id.clone()),
            launch_task_id: None,
            status: Some(status),
            message: error_message,
            occurred_at: Utc::now().to_rfc3339(),
            run: None,
            project: Some(record.clone()),
            action: None,
            stats: None,
        });

        Ok(record)
    }

    async fn execute_launch_task_actions(
        &self,
        snapshot_project: &SnapshotProjectPayload,
        runtime_context: &RestoreRuntimeContext,
        project_task_id: &str,
        launch_task_row_ids: &HashMap<String, String>,
        capabilities: &RestoreCapabilities,
        actions: &mut [RestoreActionPlan],
    ) -> AppResult<Option<String>> {
        let launch_task_by_id = snapshot_project
            .launch_tasks
            .iter()
            .map(|task| (task.id.clone(), task.clone()))
            .collect::<HashMap<_, _>>();
        let project_directory =
            ensure_absolute_directory(&snapshot_project.path, "INVALID_PROJECT_PATH");

        let mut halt_reason: Option<String> = None;
        for action in actions
            .iter_mut()
            .filter(|action| action.kind == "launch_task" && action.status == "planned")
        {
            if halt_reason.is_none() && runtime_context.is_cancel_requested() {
                halt_reason = Some(cancel_reason());
            }

            let launch_task_id = action.launch_task_id.clone().ok_or_else(|| {
                AppError::new(
                    "RESTORE_RUN_TASK_NOT_FOUND",
                    "launch task action is missing launchTaskId",
                )
                .with_detail("projectId", snapshot_project.id.clone())
            })?;
            let task_row_id = launch_task_row_ids.get(&launch_task_id).ok_or_else(|| {
                AppError::new(
                    "RESTORE_RUN_TASK_NOT_FOUND",
                    "launch task restore row was not found",
                )
                .with_detail("launchTaskId", launch_task_id.clone())
                .with_detail("projectId", snapshot_project.id.clone())
            })?;
            let launch_task = launch_task_by_id.get(&launch_task_id).ok_or_else(|| {
                AppError::new(
                    "LAUNCH_TASK_NOT_FOUND",
                    "snapshot launch task was not found",
                )
                .with_detail("launchTaskId", launch_task_id.clone())
                .with_detail("projectId", snapshot_project.id.clone())
            })?;

            if runtime_context.is_action_cancel_requested(project_task_id, &action.id) {
                mark_action_cancelled(action, action_cancelled_reason(&action.label));
                runtime_context.sync_action(project_task_id, action);
                let task_row_id_for_update = task_row_id.clone();
                let finished_at_for_update = action.finished_at.clone();
                let error_for_update = action.reason.clone();
                self.database
                    .write("cancel_pending_launch_task_action", move |connection| {
                        update_restore_run_task(
                            connection,
                            &task_row_id_for_update,
                            "cancelled",
                            0,
                            None,
                            finished_at_for_update.as_deref(),
                            error_for_update.as_deref(),
                        )
                    })
                    .await?;
                emit_action_event(
                    runtime_context,
                    snapshot_project,
                    project_task_id,
                    "action_cancelled",
                    action,
                    action.reason.clone(),
                );
                halt_reason = Some(cancel_reason());
                continue;
            }

            if let Some(reason) = halt_reason.clone() {
                mark_action_skipped(action, reason.clone());
                runtime_context.sync_action(project_task_id, action);
                let task_row_id_for_update = task_row_id.clone();
                let skipped_reason = reason;
                self.database
                    .write("skip_launch_task_after_failure", move |connection| {
                        let finished_at = Utc::now().to_rfc3339();
                        update_restore_run_task(
                            connection,
                            &task_row_id_for_update,
                            "skipped",
                            0,
                            None,
                            Some(finished_at.as_str()),
                            Some(skipped_reason.as_str()),
                        )
                    })
                    .await?;
                emit_action_event(
                    runtime_context,
                    snapshot_project,
                    project_task_id,
                    "action_finished",
                    action,
                    action.reason.clone(),
                );
                continue;
            }

            mark_action_running(action);
            runtime_context.sync_action(project_task_id, action);
            let task_row_id_for_update = task_row_id.clone();
            let started_at_for_update = action.started_at.clone();
            self.database
                .write("mark_launch_task_running", move |connection| {
                    update_restore_run_task(
                        connection,
                        &task_row_id_for_update,
                        "running",
                        1,
                        started_at_for_update.as_deref(),
                        None,
                        None,
                    )
                })
                .await?;
            emit_action_event(
                runtime_context,
                snapshot_project,
                project_task_id,
                "action_started",
                action,
                Some(format!("启动任务开始执行：{}", launch_task.name)),
            );

            let tracking = self.process_tracking_context(
                runtime_context,
                project_task_id,
                &[action.id.clone()],
            );
            match execute_launch_task(
                launch_task,
                snapshot_project,
                capabilities,
                project_directory.as_ref(),
                tracking,
            )
            .await
            {
                Ok(result) => {
                    mark_action_completed(
                        action,
                        &result,
                        Some(result.executable_path.clone()),
                        Some("launch_task".to_string()),
                    );
                    let task_row_id_for_update = task_row_id.clone();
                    let started_at_for_update = action.started_at.clone();
                    let finished_at_for_update = action.finished_at.clone();
                    self.database
                        .write("finalize_launch_task_completed", move |connection| {
                            update_restore_run_task(
                                connection,
                                &task_row_id_for_update,
                                "completed",
                                1,
                                started_at_for_update.as_deref(),
                                finished_at_for_update.as_deref(),
                                None,
                            )
                        })
                        .await?;
                }
                Err(error) => {
                    mark_action_from_error(
                        action,
                        &error,
                        Some(launch_task.command.clone()),
                        Some("launch_task".to_string()),
                    );
                    let task_row_id_for_update = task_row_id.clone();
                    let status_for_update = action.status.clone();
                    let started_at_for_update = action.started_at.clone();
                    let finished_at_for_update = action.finished_at.clone();
                    let error_for_update = action.reason.clone();
                    self.database
                        .write("finalize_launch_task_failed", move |connection| {
                            update_restore_run_task(
                                connection,
                                &task_row_id_for_update,
                                &status_for_update,
                                1,
                                started_at_for_update.as_deref(),
                                finished_at_for_update.as_deref(),
                                error_for_update.as_deref(),
                            )
                        })
                        .await?;

                    if error.code == "PROCESS_CANCELLED" {
                        halt_reason = Some(cancel_reason());
                    } else if !launch_task.continue_on_failure {
                        halt_reason = action.reason.clone();
                    }
                }
            }

            runtime_context.sync_action(project_task_id, action);
            let event_type = if action.status == "cancelled" {
                "action_cancelled"
            } else {
                "action_finished"
            };
            emit_action_event(
                runtime_context,
                snapshot_project,
                project_task_id,
                event_type,
                action,
                action.reason.clone(),
            );
        }

        Ok(halt_reason)
    }
}

impl RestoreService {
    async fn execute_terminal_actions(
        &self,
        snapshot_project: &SnapshotProjectPayload,
        runtime_context: &RestoreRuntimeContext,
        project_task_id: &str,
        capabilities: &RestoreCapabilities,
        actions: &mut [RestoreActionPlan],
    ) {
        let terminal_index = actions
            .iter()
            .position(|action| action.kind == "terminal_context" && action.status == "planned");
        let mut codex_index = actions
            .iter()
            .position(|action| action.kind == "codex_session" && action.status == "planned");

        if terminal_index.is_none() && codex_index.is_none() {
            return;
        }

        if let Some(index) = terminal_index {
            let action_id = actions[index].id.clone();
            if runtime_context.is_action_cancel_requested(project_task_id, &action_id) {
                let label = actions[index].label.clone();
                mark_action_cancelled(&mut actions[index], action_cancelled_reason(&label));
                runtime_context.sync_action(project_task_id, &actions[index]);
                emit_action_event(
                    runtime_context,
                    snapshot_project,
                    project_task_id,
                    "action_cancelled",
                    &actions[index],
                    actions[index].reason.clone(),
                );

                if let Some(codex_action_index) = codex_index {
                    mark_action_skipped(
                        &mut actions[codex_action_index],
                        "终端动作已取消，Codex 动作已跳过".to_string(),
                    );
                    runtime_context.sync_action(project_task_id, &actions[codex_action_index]);
                    emit_action_event(
                        runtime_context,
                        snapshot_project,
                        project_task_id,
                        "action_finished",
                        &actions[codex_action_index],
                        actions[codex_action_index].reason.clone(),
                    );
                }
                return;
            }
        }

        if let Some(index) = codex_index {
            let action_id = actions[index].id.clone();
            if runtime_context.is_action_cancel_requested(project_task_id, &action_id) {
                let label = actions[index].label.clone();
                mark_action_cancelled(&mut actions[index], action_cancelled_reason(&label));
                runtime_context.sync_action(project_task_id, &actions[index]);
                emit_action_event(
                    runtime_context,
                    snapshot_project,
                    project_task_id,
                    "action_cancelled",
                    &actions[index],
                    actions[index].reason.clone(),
                );
                codex_index = None;
            }
        }

        if terminal_index.is_none() && codex_index.is_none() {
            return;
        }

        let project_path =
            match ensure_absolute_directory(&snapshot_project.path, "INVALID_PROJECT_PATH") {
                Ok(path) => path,
                Err(error) => {
                    if let Some(index) = terminal_index {
                        mark_action_from_error(&mut actions[index], &error, None, None);
                        runtime_context.sync_action(project_task_id, &actions[index]);
                        emit_action_event(
                            runtime_context,
                            snapshot_project,
                            project_task_id,
                            "action_finished",
                            &actions[index],
                            actions[index].reason.clone(),
                        );
                    }
                    if let Some(index) = codex_index {
                        mark_action_from_error(&mut actions[index], &error, None, None);
                        runtime_context.sync_action(project_task_id, &actions[index]);
                        emit_action_event(
                            runtime_context,
                            snapshot_project,
                            project_task_id,
                            "action_finished",
                            &actions[index],
                            actions[index].reason.clone(),
                        );
                    }
                    return;
                }
            };

        if !capabilities.terminal.available {
            let error = AppError::new(
                "TERMINAL_ADAPTER_UNAVAILABLE",
                "Windows Terminal is not available on this machine",
            )
            .with_detail("adapter", "windows_terminal")
            .with_detail("message", capabilities.terminal.message.clone());
            if let Some(index) = terminal_index {
                mark_action_from_error(
                    &mut actions[index],
                    &error,
                    capabilities.terminal.executable_path.clone(),
                    Some(capabilities.terminal.source.clone()),
                );
                runtime_context.sync_action(project_task_id, &actions[index]);
                emit_action_event(
                    runtime_context,
                    snapshot_project,
                    project_task_id,
                    "action_finished",
                    &actions[index],
                    actions[index].reason.clone(),
                );
            }
            if let Some(index) = codex_index {
                mark_action_from_error(
                    &mut actions[index],
                    &error,
                    capabilities.terminal.executable_path.clone(),
                    Some(capabilities.terminal.source.clone()),
                );
                runtime_context.sync_action(project_task_id, &actions[index]);
                emit_action_event(
                    runtime_context,
                    snapshot_project,
                    project_task_id,
                    "action_finished",
                    &actions[index],
                    actions[index].reason.clone(),
                );
            }
            return;
        }

        let codex_adapter = DefaultCodexAdapter;
        let mut codex_launch_plan = None;
        if let Some(index) = codex_index {
            if !capabilities.codex.available {
                let error = AppError::new(
                    "CODEX_ADAPTER_UNAVAILABLE",
                    "Codex CLI is not available on this machine",
                )
                .with_detail("adapter", "codex")
                .with_detail("message", capabilities.codex.message.clone());
                mark_action_from_error(
                    &mut actions[index],
                    &error,
                    capabilities.codex.executable_path.clone(),
                    Some(capabilities.codex.source.clone()),
                );
            } else if let Some(profile) = &snapshot_project.codex_profile {
                match codex_adapter.build_launch_plan(
                    capabilities
                        .codex
                        .executable_path
                        .as_deref()
                        .unwrap_or_default(),
                    CodexLaunchInput {
                        profile,
                        startup_mode_override: None,
                        extra_args: &[],
                    },
                ) {
                    Ok(plan) => codex_launch_plan = Some(plan),
                    Err(error) => mark_action_from_error(
                        &mut actions[index],
                        &error,
                        capabilities.codex.executable_path.clone(),
                        Some(capabilities.codex.source.clone()),
                    ),
                }
            } else {
                let error = AppError::new(
                    "CODEX_PROFILE_NOT_FOUND",
                    "snapshot project is missing a codex profile",
                );
                mark_action_from_error(&mut actions[index], &error, None, None);
            }
            if let Some(index) = codex_index {
                if actions[index].status != "planned" {
                    runtime_context.sync_action(project_task_id, &actions[index]);
                    emit_action_event(
                        runtime_context,
                        snapshot_project,
                        project_task_id,
                        "action_finished",
                        &actions[index],
                        actions[index].reason.clone(),
                    );
                }
            }
        }

        if terminal_index.is_none() && codex_launch_plan.is_none() {
            return;
        }

        let terminal_adapter = WindowsTerminalAdapter;
        let launch_command = match terminal_adapter.build_launch_plan(
            capabilities
                .terminal
                .executable_path
                .as_deref()
                .unwrap_or_default(),
            TerminalLaunchInput {
                project_path,
                startup_command: codex_launch_plan
                    .as_ref()
                    .and_then(|plan| plan.terminal_command.clone()),
                envs: codex_launch_plan
                    .as_ref()
                    .map(|plan| plan.envs.clone())
                    .unwrap_or_default(),
            },
        ) {
            Ok(command) => command,
            Err(error) => {
                if let Some(index) = terminal_index {
                    mark_action_from_error(
                        &mut actions[index],
                        &error,
                        capabilities.terminal.executable_path.clone(),
                        Some(capabilities.terminal.source.clone()),
                    );
                    runtime_context.sync_action(project_task_id, &actions[index]);
                    emit_action_event(
                        runtime_context,
                        snapshot_project,
                        project_task_id,
                        "action_finished",
                        &actions[index],
                        actions[index].reason.clone(),
                    );
                }
                if let Some(index) = codex_index {
                    let executable_path = codex_launch_plan
                        .as_ref()
                        .map(|plan| plan.executable_path.clone());
                    mark_action_from_error(
                        &mut actions[index],
                        &error,
                        executable_path,
                        Some(capabilities.codex.source.clone()),
                    );
                    runtime_context.sync_action(project_task_id, &actions[index]);
                    emit_action_event(
                        runtime_context,
                        snapshot_project,
                        project_task_id,
                        "action_finished",
                        &actions[index],
                        actions[index].reason.clone(),
                    );
                }
                return;
            }
        };

        if let Some(index) = terminal_index {
            mark_action_running(&mut actions[index]);
            runtime_context.sync_action(project_task_id, &actions[index]);
            emit_action_event(
                runtime_context,
                snapshot_project,
                project_task_id,
                "action_started",
                &actions[index],
                Some("终端上下文开始启动".to_string()),
            );
        }
        if let Some(index) = codex_index {
            mark_action_running(&mut actions[index]);
            runtime_context.sync_action(project_task_id, &actions[index]);
            emit_action_event(
                runtime_context,
                snapshot_project,
                project_task_id,
                "action_started",
                &actions[index],
                Some("Codex 启动动作开始执行".to_string()),
            );
        }

        let mut tracking_action_ids = Vec::new();
        if let Some(index) = terminal_index {
            tracking_action_ids.push(actions[index].id.clone());
        }
        if let Some(index) = codex_index {
            tracking_action_ids.push(actions[index].id.clone());
        }

        let tracked_command = LaunchCommand {
            executable_path: launch_command.executable_path,
            args: launch_command.args,
            current_dir: launch_command.current_dir,
            envs: launch_command.envs,
            timeout: launch_command.timeout,
            tracking: Some(self.process_tracking_context(
                runtime_context,
                project_task_id,
                &tracking_action_ids,
            )),
            retain_after_timeout: true,
        };
        match run_launch_command(tracked_command).await {
            Ok(result) => {
                if let Some(index) = terminal_index {
                    mark_action_completed(
                        &mut actions[index],
                        &result,
                        Some(result.executable_path.clone()),
                        Some(capabilities.terminal.source.clone()),
                    );
                    runtime_context.sync_action(project_task_id, &actions[index]);
                    emit_action_event(
                        runtime_context,
                        snapshot_project,
                        project_task_id,
                        "action_finished",
                        &actions[index],
                        actions[index].reason.clone(),
                    );
                }
                if let Some(index) = codex_index {
                    if let Some(plan) = &codex_launch_plan {
                        mark_action_completed(
                            &mut actions[index],
                            &result,
                            Some(plan.executable_path.clone()),
                            Some(capabilities.codex.source.clone()),
                        );
                        runtime_context.sync_action(project_task_id, &actions[index]);
                        emit_action_event(
                            runtime_context,
                            snapshot_project,
                            project_task_id,
                            "action_finished",
                            &actions[index],
                            actions[index].reason.clone(),
                        );
                    }
                }
            }
            Err(error) => {
                if let Some(index) = terminal_index {
                    mark_action_from_error(
                        &mut actions[index],
                        &error,
                        capabilities.terminal.executable_path.clone(),
                        Some(capabilities.terminal.source.clone()),
                    );
                    runtime_context.sync_action(project_task_id, &actions[index]);
                    let event_type = if actions[index].status == "cancelled" {
                        "action_cancelled"
                    } else {
                        "action_finished"
                    };
                    emit_action_event(
                        runtime_context,
                        snapshot_project,
                        project_task_id,
                        event_type,
                        &actions[index],
                        actions[index].reason.clone(),
                    );
                }
                if let Some(index) = codex_index {
                    let executable_path = codex_launch_plan
                        .as_ref()
                        .map(|plan| plan.executable_path.clone());
                    mark_action_from_error(
                        &mut actions[index],
                        &error,
                        executable_path,
                        Some(capabilities.codex.source.clone()),
                    );
                    runtime_context.sync_action(project_task_id, &actions[index]);
                    let event_type = if actions[index].status == "cancelled" {
                        "action_cancelled"
                    } else {
                        "action_finished"
                    };
                    emit_action_event(
                        runtime_context,
                        snapshot_project,
                        project_task_id,
                        event_type,
                        &actions[index],
                        actions[index].reason.clone(),
                    );
                }
            }
        }
    }

    async fn execute_ide_actions(
        &self,
        snapshot_project: &SnapshotProjectPayload,
        runtime_context: &RestoreRuntimeContext,
        project_task_id: &str,
        capabilities: &RestoreCapabilities,
        actions: &mut [RestoreActionPlan],
    ) {
        let Some(index) = actions
            .iter()
            .position(|action| action.kind == "ide_window" && action.status == "planned")
        else {
            return;
        };

        let action_id = actions[index].id.clone();
        if runtime_context.is_action_cancel_requested(project_task_id, &action_id) {
            let label = actions[index].label.clone();
            mark_action_cancelled(&mut actions[index], action_cancelled_reason(&label));
            runtime_context.sync_action(project_task_id, &actions[index]);
            emit_action_event(
                runtime_context,
                snapshot_project,
                project_task_id,
                "action_cancelled",
                &actions[index],
                actions[index].reason.clone(),
            );
            return;
        }

        let project_path =
            match ensure_absolute_directory(&snapshot_project.path, "INVALID_PROJECT_PATH") {
                Ok(path) => path,
                Err(error) => {
                    mark_action_from_error(&mut actions[index], &error, None, None);
                    runtime_context.sync_action(project_task_id, &actions[index]);
                    emit_action_event(
                        runtime_context,
                        snapshot_project,
                        project_task_id,
                        "action_finished",
                        &actions[index],
                        actions[index].reason.clone(),
                    );
                    return;
                }
            };

        let ide_type = snapshot_project.ide_type.as_deref().unwrap_or_default();
        let (availability, launch_result) = match ide_type {
            "vscode" => {
                let adapter = VSCodeAdapter;
                let availability = capabilities.vscode.clone();
                let result = if availability.available {
                    adapter.build_launch_plan(
                        availability.executable_path.as_deref().unwrap_or_default(),
                        &project_path,
                    )
                } else {
                    Err(AppError::new(
                        "IDE_ADAPTER_UNAVAILABLE",
                        "IDE CLI is not available on this machine",
                    ))
                };
                (availability, result)
            }
            "jetbrains" => {
                let adapter = JetBrainsAdapter;
                let availability = capabilities.jetbrains.clone();
                let result = if availability.available {
                    adapter.build_launch_plan(
                        availability.executable_path.as_deref().unwrap_or_default(),
                        &project_path,
                    )
                } else {
                    Err(AppError::new(
                        "IDE_ADAPTER_UNAVAILABLE",
                        "IDE CLI is not available on this machine",
                    ))
                };
                (availability, result)
            }
            _ => {
                let error = AppError::new("IDE_TYPE_UNSUPPORTED", "unsupported IDE type")
                    .with_detail("ideType", ide_type.to_string());
                mark_action_from_error(&mut actions[index], &error, None, None);
                return;
            }
        };

        if !availability.available {
            let error = AppError::new(
                "IDE_ADAPTER_UNAVAILABLE",
                "IDE CLI is not available on this machine",
            )
            .with_detail("adapter", availability.key.clone())
            .with_detail("message", availability.message.clone());
            mark_action_from_error(
                &mut actions[index],
                &error,
                availability.executable_path,
                Some(availability.source.clone()),
            );
            runtime_context.sync_action(project_task_id, &actions[index]);
            emit_action_event(
                runtime_context,
                snapshot_project,
                project_task_id,
                "action_finished",
                &actions[index],
                actions[index].reason.clone(),
            );
            return;
        }

        match launch_result {
            Ok(command) => {
                mark_action_running(&mut actions[index]);
                runtime_context.sync_action(project_task_id, &actions[index]);
                emit_action_event(
                    runtime_context,
                    snapshot_project,
                    project_task_id,
                    "action_started",
                    &actions[index],
                    Some("IDE 启动动作开始执行".to_string()),
                );
                let tracked_command = LaunchCommand {
                    executable_path: command.executable_path,
                    args: command.args,
                    current_dir: command.current_dir,
                    envs: command.envs,
                    timeout: command.timeout,
                    tracking: Some(self.process_tracking_context(
                        runtime_context,
                        project_task_id,
                        &[actions[index].id.clone()],
                    )),
                    retain_after_timeout: true,
                };
                match run_launch_command(tracked_command).await {
                    Ok(result) => {
                        mark_action_completed(
                            &mut actions[index],
                            &result,
                            availability.executable_path,
                            Some(availability.source.clone()),
                        );
                        runtime_context.sync_action(project_task_id, &actions[index]);
                        emit_action_event(
                            runtime_context,
                            snapshot_project,
                            project_task_id,
                            "action_finished",
                            &actions[index],
                            actions[index].reason.clone(),
                        );
                    }
                    Err(error) => {
                        mark_action_from_error(
                            &mut actions[index],
                            &error,
                            availability.executable_path,
                            Some(availability.source.clone()),
                        );
                        runtime_context.sync_action(project_task_id, &actions[index]);
                        let event_type = if actions[index].status == "cancelled" {
                            "action_cancelled"
                        } else {
                            "action_finished"
                        };
                        emit_action_event(
                            runtime_context,
                            snapshot_project,
                            project_task_id,
                            event_type,
                            &actions[index],
                            actions[index].reason.clone(),
                        );
                    }
                }
            }
            Err(error) => {
                mark_action_from_error(
                    &mut actions[index],
                    &error,
                    availability.executable_path,
                    Some(availability.source.clone()),
                );
                runtime_context.sync_action(project_task_id, &actions[index]);
                emit_action_event(
                    runtime_context,
                    snapshot_project,
                    project_task_id,
                    "action_finished",
                    &actions[index],
                    actions[index].reason.clone(),
                );
            }
        }
    }

    async fn build_cancelled_pending_project(
        &self,
        runtime_context: &RestoreRuntimeContext,
        planned_project: &RestoreProjectPlan,
        task_id: String,
        launch_task_row_ids: &HashMap<String, String>,
    ) -> AppResult<RestoreRunProjectRecord> {
        let mut actions = planned_project.actions.clone();
        skip_pending_actions(&mut actions, &cancel_reason());

        for action in actions.iter().filter(|action| action.kind == "launch_task") {
            let Some(launch_task_id) = action.launch_task_id.as_ref() else {
                continue;
            };
            let Some(task_row_id) = launch_task_row_ids.get(launch_task_id) else {
                continue;
            };
            let task_row_id_for_update = task_row_id.clone();
            let reason = cancel_reason();
            self.database
                .write("skip_cancelled_pending_launch_task", move |connection| {
                    let finished_at = Utc::now().to_rfc3339();
                    update_restore_run_task(
                        connection,
                        &task_row_id_for_update,
                        "skipped",
                        0,
                        None,
                        Some(finished_at.as_str()),
                        Some(reason.as_str()),
                    )
                })
                .await?;
        }

        let task_id_for_update = task_id.clone();
        let reason = cancel_reason();
        self.database
            .write("skip_cancelled_pending_project_task", move |connection| {
                let finished_at = Utc::now().to_rfc3339();
                update_restore_run_task(
                    connection,
                    &task_id_for_update,
                    "skipped",
                    0,
                    None,
                    Some(finished_at.as_str()),
                    Some(reason.as_str()),
                )
            })
            .await?;

        let record = RestoreRunProjectRecord {
            id: task_id,
            restore_run_id: runtime_context.run_id.clone(),
            project_id: Some(planned_project.project_id.clone()),
            project_name: planned_project.project_name.clone(),
            path: planned_project.path.clone(),
            status: "skipped".to_string(),
            attempt_count: 0,
            started_at: None,
            finished_at: Some(Utc::now().to_rfc3339()),
            error_message: Some(cancel_reason()),
            actions,
        };
        for action in &record.actions {
            runtime_context.sync_action(&record.id, action);
        }
        runtime_context.emit(RestoreRunEvent {
            event_type: "project_finished".to_string(),
            run_id: runtime_context.run_id.clone(),
            workspace_id: runtime_context.workspace_id.clone(),
            snapshot_id: Some(runtime_context.snapshot_id.clone()),
            project_id: Some(planned_project.project_id.clone()),
            project_task_id: Some(record.id.clone()),
            launch_task_id: None,
            status: Some(record.status.clone()),
            message: record.error_message.clone(),
            occurred_at: Utc::now().to_rfc3339(),
            run: None,
            project: Some(record.clone()),
            action: None,
            stats: None,
        });

        Ok(record)
    }

    fn active_run(&self, run_id: &str) -> Option<Arc<ActiveRestoreRun>> {
        self.active_runs
            .lock()
            .expect("active restore runs poisoned")
            .get(run_id)
            .cloned()
    }

    fn process_tracking_context(
        &self,
        runtime_context: &RestoreRuntimeContext,
        project_task_id: &str,
        action_ids: &[String],
    ) -> ProcessTrackingContext {
        ProcessTrackingContext {
            registry: self.child_process_registry.clone(),
            run_id: runtime_context.run_id.clone(),
            action_keys: action_ids
                .iter()
                .map(|action_id| action_key(project_task_id, action_id))
                .collect(),
            run_cancel_requested: runtime_context.active_run.cancel_token(),
            action_cancel_requested: runtime_context
                .action_cancel_tokens(project_task_id, action_ids),
        }
    }
}

async fn execute_launch_task(
    launch_task: &SnapshotLaunchTaskPayload,
    snapshot_project: &SnapshotProjectPayload,
    capabilities: &RestoreCapabilities,
    project_directory: Result<&String, &AppError>,
    tracking: ProcessTrackingContext,
) -> AppResult<ProcessLaunchResult> {
    match launch_task.task_type.as_str() {
        "terminal_command" => {
            let working_dir = resolve_launch_task_working_dir(launch_task, project_directory)?;
            run_launch_command(LaunchCommand {
                executable_path: PathBuf::from(launch_task.command.clone()),
                args: launch_task.args.clone(),
                current_dir: Some(PathBuf::from(working_dir)),
                envs: Vec::new(),
                timeout: Duration::from_millis(launch_task.timeout_ms as u64),
                tracking: Some(tracking),
                retain_after_timeout: true,
            })
            .await
        }
        "open_path" => {
            let path = PathBuf::from(launch_task.command.clone());
            let metadata = std::fs::metadata(&path).map_err(|error| {
                AppError::new(
                    "INVALID_LAUNCH_TASK_PATH",
                    "launch task target does not exist",
                )
                .with_detail("path", launch_task.command.clone())
                .with_detail("reason", error.to_string())
            })?;
            if !metadata.is_dir() && !metadata.is_file() {
                return Err(AppError::new(
                    "INVALID_LAUNCH_TASK_PATH",
                    "launch task target must be a file or directory",
                )
                .with_detail("path", launch_task.command.clone()));
            }

            run_launch_command(LaunchCommand {
                executable_path: PathBuf::from("explorer.exe"),
                args: vec![launch_task.command.clone()],
                current_dir: None,
                envs: Vec::new(),
                timeout: Duration::from_millis(launch_task.timeout_ms as u64),
                tracking: Some(tracking),
                retain_after_timeout: true,
            })
            .await
        }
        "ide" => {
            let target_path = resolve_launch_task_working_dir(launch_task, project_directory)?;
            let (availability, launch_result) = match launch_task.command.as_str() {
                "vscode" => {
                    let adapter = VSCodeAdapter;
                    let availability = capabilities.vscode.clone();
                    let result = if availability.available {
                        adapter.build_launch_plan(
                            availability.executable_path.as_deref().unwrap_or_default(),
                            &target_path,
                        )
                    } else {
                        Err(AppError::new(
                            "IDE_ADAPTER_UNAVAILABLE",
                            "VS Code CLI is not available on this machine",
                        )
                        .with_detail("adapter", availability.key.clone())
                        .with_detail("message", availability.message.clone()))
                    };
                    (availability, result)
                }
                "jetbrains" => {
                    let adapter = JetBrainsAdapter;
                    let availability = capabilities.jetbrains.clone();
                    let result = if availability.available {
                        adapter.build_launch_plan(
                            availability.executable_path.as_deref().unwrap_or_default(),
                            &target_path,
                        )
                    } else {
                        Err(AppError::new(
                            "IDE_ADAPTER_UNAVAILABLE",
                            "JetBrains CLI is not available on this machine",
                        )
                        .with_detail("adapter", availability.key.clone())
                        .with_detail("message", availability.message.clone()))
                    };
                    (availability, result)
                }
                other => {
                    return Err(AppError::new(
                        "IDE_TYPE_UNSUPPORTED",
                        "unsupported launch task IDE type",
                    )
                    .with_detail("ideType", other.to_string()));
                }
            };

            let mut command = launch_result?;
            command.args.extend(launch_task.args.clone());
            command.timeout = Duration::from_millis(launch_task.timeout_ms as u64);
            command.tracking = Some(tracking);
            command.retain_after_timeout = true;
            run_launch_command(command).await.map_err(|error| {
                error
                    .with_detail(
                        "executablePath",
                        availability.executable_path.unwrap_or_default(),
                    )
                    .with_detail("source", availability.source.clone())
            })
        }
        "codex" => {
            if !capabilities.terminal.available {
                return Err(AppError::new(
                    "TERMINAL_ADAPTER_UNAVAILABLE",
                    "Windows Terminal is not available on this machine",
                )
                .with_detail("adapter", "windows_terminal")
                .with_detail("message", capabilities.terminal.message.clone()));
            }

            let profile = snapshot_project.codex_profile.as_ref().ok_or_else(|| {
                AppError::new(
                    "CODEX_PROFILE_NOT_FOUND",
                    "snapshot project is missing a codex profile",
                )
            })?;
            let startup_mode = resolve_codex_launch_task_mode(launch_task, profile)?;
            if startup_mode != "terminal_only" && !capabilities.codex.available {
                return Err(AppError::new(
                    "CODEX_ADAPTER_UNAVAILABLE",
                    "Codex CLI is not available on this machine",
                )
                .with_detail("adapter", "codex")
                .with_detail("message", capabilities.codex.message.clone()));
            }

            let codex_adapter = DefaultCodexAdapter;
            let codex_launch_plan = codex_adapter.build_launch_plan(
                capabilities
                    .codex
                    .executable_path
                    .as_deref()
                    .unwrap_or_default(),
                CodexLaunchInput {
                    profile,
                    startup_mode_override: Some(startup_mode.as_str()),
                    extra_args: &launch_task.args,
                },
            )?;
            let terminal_adapter = WindowsTerminalAdapter;
            let mut command = terminal_adapter.build_launch_plan(
                capabilities
                    .terminal
                    .executable_path
                    .as_deref()
                    .unwrap_or_default(),
                TerminalLaunchInput {
                    project_path: resolve_launch_task_working_dir(launch_task, project_directory)?,
                    startup_command: codex_launch_plan.terminal_command,
                    envs: codex_launch_plan.envs,
                },
            )?;
            command.timeout = Duration::from_millis(launch_task.timeout_ms as u64);
            command.tracking = Some(tracking);
            command.retain_after_timeout = true;
            run_launch_command(command).await
        }
        other => Err(AppError::new(
            "LAUNCH_TASK_TYPE_UNSUPPORTED",
            "launch task type is not supported during runtime",
        )
        .with_detail("taskType", other.to_string())),
    }
}

fn resolve_launch_task_working_dir(
    launch_task: &SnapshotLaunchTaskPayload,
    project_directory: Result<&String, &AppError>,
) -> AppResult<String> {
    if !launch_task.working_dir.trim().is_empty() {
        return ensure_absolute_directory(
            &launch_task.working_dir,
            "INVALID_LAUNCH_TASK_WORKING_DIR",
        );
    }

    match project_directory {
        Ok(directory) => Ok(directory.clone()),
        Err(error) => Err(error.clone()),
    }
}

fn resolve_codex_launch_task_mode(
    launch_task: &SnapshotLaunchTaskPayload,
    profile: &crate::domain::SnapshotCodexProfilePayload,
) -> AppResult<String> {
    if launch_task.command == "inherit_profile" {
        return Ok(profile.startup_mode.clone());
    }

    match launch_task.command.as_str() {
        "terminal_only" | "run_codex" | "resume_last" => Ok(launch_task.command.clone()),
        other => Err(AppError::new(
            "CODEX_STARTUP_MODE_INVALID",
            "unsupported launch task codex mode",
        )
        .with_detail("startupMode", other.to_string())),
    }
}

fn probe_restore_capabilities(preferences: &AppPreferences) -> RestoreCapabilities {
    let terminal =
        WindowsTerminalAdapter.detect(preferences.terminal.windows_terminal_path.as_deref());
    let vscode = VSCodeAdapter.detect(preferences.ide.vscode_path.as_deref());
    let jetbrains = JetBrainsAdapter.detect(preferences.ide.jetbrains_path.as_deref());
    let codex = DefaultCodexAdapter.detect(preferences.terminal.codex_cli_path.as_deref());

    RestoreCapabilities {
        checked_at: Utc::now().to_rfc3339(),
        terminal,
        vscode,
        jetbrains,
        codex,
    }
}

fn emit_action_event(
    runtime_context: &RestoreRuntimeContext,
    snapshot_project: &SnapshotProjectPayload,
    project_task_id: &str,
    event_type: &str,
    action: &RestoreActionPlan,
    message: Option<String>,
) {
    runtime_context.emit(RestoreRunEvent {
        event_type: event_type.to_string(),
        run_id: runtime_context.run_id.clone(),
        workspace_id: runtime_context.workspace_id.clone(),
        snapshot_id: Some(runtime_context.snapshot_id.clone()),
        project_id: Some(snapshot_project.id.clone()),
        project_task_id: Some(project_task_id.to_string()),
        launch_task_id: action.launch_task_id.clone(),
        status: Some(action.status.clone()),
        message,
        occurred_at: Utc::now().to_rfc3339(),
        run: None,
        project: None,
        action: Some(action.clone()),
        stats: None,
    });
}

fn mark_action_running(action: &mut RestoreActionPlan) {
    if action.started_at.is_none() {
        action.started_at = Some(Utc::now().to_rfc3339());
    }
    action.status = "running".to_string();
    action.finished_at = None;
    action.reason = None;
    action.duration_ms = None;
    action.diagnostic_code = None;
}

fn merge_action_runtime_fields(action: &mut RestoreActionPlan, runtime_action: &RestoreActionPlan) {
    if action.cancel_requested_at.is_none() {
        action.cancel_requested_at = runtime_action.cancel_requested_at.clone();
    }
}

fn mark_action_completed(
    action: &mut RestoreActionPlan,
    result: &ProcessLaunchResult,
    executable_path: Option<String>,
    executable_source: Option<String>,
) {
    let finished_at = Utc::now().to_rfc3339();
    action.status = "completed".to_string();
    action.reason = None;
    if action.started_at.is_none() {
        action.started_at = Some(finished_at.clone());
    }
    action.finished_at = Some(finished_at);
    action.duration_ms = Some(result.duration_ms);
    action.executable_path = executable_path;
    action.executable_source = executable_source;
    action.diagnostic_code = None;
}

fn mark_action_from_error(
    action: &mut RestoreActionPlan,
    error: &AppError,
    executable_path: Option<String>,
    executable_source: Option<String>,
) {
    let finished_at = Utc::now().to_rfc3339();
    action.status = if error.code == "PROCESS_CANCELLED" {
        "cancelled".to_string()
    } else if is_blocking_error(error) {
        "blocked".to_string()
    } else {
        "failed".to_string()
    };
    action.reason = Some(format!("{}: {}", error.code, error.message));
    if action.started_at.is_none() {
        action.started_at = Some(finished_at.clone());
    }
    action.finished_at = Some(finished_at);
    action.duration_ms = None;
    action.executable_path = executable_path;
    action.executable_source = executable_source;
    action.diagnostic_code = Some(error.code.clone());
}

fn mark_action_skipped(action: &mut RestoreActionPlan, reason: String) {
    action.status = "skipped".to_string();
    action.reason = Some(reason);
    action.finished_at = Some(Utc::now().to_rfc3339());
    action.diagnostic_code = None;
}

fn mark_action_cancelled(action: &mut RestoreActionPlan, reason: String) {
    let finished_at = Utc::now().to_rfc3339();
    action.status = "cancelled".to_string();
    action.reason = Some(reason);
    if action.started_at.is_none() {
        action.started_at = Some(finished_at.clone());
    }
    action.finished_at = Some(finished_at);
    action.duration_ms = None;
    action.diagnostic_code = Some("ACTION_CANCELLED".to_string());
}

fn skip_pending_actions(actions: &mut [RestoreActionPlan], reason: &str) {
    for action in actions
        .iter_mut()
        .filter(|action| action.status == "planned")
    {
        mark_action_skipped(action, reason.to_string());
    }
}

fn is_finished_action_status(status: &str) -> bool {
    matches!(
        status,
        "completed"
            | "completed_with_blocks"
            | "completed_with_warnings"
            | "failed"
            | "blocked"
            | "skipped"
            | "cancelled"
    )
}

fn is_finished_run_status(status: &str) -> bool {
    matches!(
        status,
        "completed"
            | "completed_with_blocks"
            | "completed_with_warnings"
            | "failed"
            | "blocked"
            | "skipped"
            | "cancelled"
    )
}

fn is_blocking_error(error: &AppError) -> bool {
    matches!(
        error.code.as_str(),
        "INVALID_PROJECT_PATH"
            | "INVALID_CODEX_HOME"
            | "CODEX_ADAPTER_UNAVAILABLE"
            | "TERMINAL_ADAPTER_UNAVAILABLE"
            | "IDE_ADAPTER_UNAVAILABLE"
            | "IDE_TYPE_UNSUPPORTED"
            | "CODEX_PROFILE_NOT_FOUND"
            | "CODEX_STARTUP_MODE_INVALID"
            | "INVALID_LAUNCH_TASK_WORKING_DIR"
            | "INVALID_LAUNCH_TASK_PATH"
            | "LAUNCH_TASK_TYPE_UNSUPPORTED"
    )
}

fn summarize_project_actions(actions: &[RestoreActionPlan]) -> (String, Option<String>) {
    if let Some(action) = actions.iter().find(|action| action.status == "cancelled") {
        return ("cancelled".to_string(), action.reason.clone());
    }
    if let Some(action) = actions.iter().find(|action| action.status == "failed") {
        return ("failed".to_string(), action.reason.clone());
    }
    if let Some(action) = actions.iter().find(|action| action.status == "blocked") {
        return ("blocked".to_string(), action.reason.clone());
    }
    if actions.iter().any(|action| action.status == "completed") {
        return ("completed".to_string(), None);
    }
    if actions.iter().any(|action| action.status == "running") {
        return ("running".to_string(), None);
    }
    (
        "skipped".to_string(),
        actions.iter().find_map(|action| action.reason.clone()),
    )
}

fn summarize_run_status(tasks: &[RestoreRunProjectRecord]) -> (String, Option<String>) {
    let failed = tasks.iter().filter(|task| task.status == "failed").count();
    let blocked = tasks.iter().filter(|task| task.status == "blocked").count();
    let cancelled = tasks
        .iter()
        .filter(|task| task.status == "cancelled")
        .count();

    let status = if failed > 0 || blocked > 0 {
        "completed_with_blocks".to_string()
    } else if cancelled > 0 {
        "completed_with_warnings".to_string()
    } else {
        "completed".to_string()
    };

    let summary = match (failed, blocked, cancelled) {
        (0, 0, 0) => None,
        (failed, 0, 0) => Some(format!("{failed} project(s) failed during restore")),
        (0, blocked, 0) => Some(format!("{blocked} project(s) were blocked during restore")),
        (0, 0, cancelled) => Some(format!(
            "{cancelled} project(s) were cancelled during restore"
        )),
        (failed, blocked, cancelled) => Some(format!(
            "{failed} project(s) failed, {blocked} project(s) were blocked, {cancelled} project(s) were cancelled during restore"
        )),
    };

    (status, summary)
}

fn build_runtime_stats(tasks: &[RestoreRunProjectRecord]) -> RestorePreviewStats {
    let mut stats = RestorePreviewStats {
        total_projects: tasks.len() as i64,
        planned_projects: 0,
        running_projects: 0,
        completed_projects: 0,
        cancelled_projects: 0,
        failed_projects: 0,
        blocked_projects: 0,
        skipped_projects: 0,
        total_actions: 0,
        planned_actions: 0,
        running_actions: 0,
        completed_actions: 0,
        cancelled_actions: 0,
        failed_actions: 0,
        blocked_actions: 0,
        skipped_actions: 0,
    };

    for task in tasks {
        match task.status.as_str() {
            "planned" => stats.planned_projects += 1,
            "running" => stats.running_projects += 1,
            "completed" => stats.completed_projects += 1,
            "cancelled" => stats.cancelled_projects += 1,
            "failed" => stats.failed_projects += 1,
            "blocked" => stats.blocked_projects += 1,
            _ => stats.skipped_projects += 1,
        }

        for action in &task.actions {
            stats.total_actions += 1;
            match action.status.as_str() {
                "planned" => stats.planned_actions += 1,
                "running" => stats.running_actions += 1,
                "completed" => stats.completed_actions += 1,
                "cancelled" => stats.cancelled_actions += 1,
                "failed" => stats.failed_actions += 1,
                "blocked" => stats.blocked_actions += 1,
                _ => stats.skipped_actions += 1,
            }
        }
    }

    stats
}

fn cancel_reason() -> String {
    "恢复批次已被用户取消".to_string()
}

fn action_cancel_requested_reason(label: &str) -> String {
    format!("已请求取消动作：{label}")
}

fn action_cancel_requested_message(label: &str, tracked_process_count: usize) -> String {
    if tracked_process_count > 0 {
        format!("已请求取消动作：{label}，并尝试终止 {tracked_process_count} 个进程")
    } else {
        action_cancel_requested_reason(label)
    }
}

fn action_cancelled_reason(label: &str) -> String {
    format!("动作已取消：{label}")
}

fn action_key(project_task_id: &str, action_id: &str) -> ActionProcessKey {
    ActionProcessKey {
        project_task_id: project_task_id.to_string(),
        action_id: action_id.to_string(),
    }
}

fn find_restore_action<'a>(
    detail: &'a RestoreRunDetail,
    project_task_id: &str,
    action_id: &str,
) -> Option<&'a RestoreActionPlan> {
    detail
        .tasks
        .iter()
        .find(|task| task.id == project_task_id)
        .and_then(|task| task.actions.iter().find(|action| action.id == action_id))
}

fn recent_target_sort_key(snapshot: &SnapshotRecord) -> &str {
    snapshot
        .last_restore_at
        .as_deref()
        .unwrap_or(snapshot.updated_at.as_str())
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        sync::{Mutex, OnceLock},
    };

    use crate::{
        domain::{
            AppPreferences, CreateSnapshotInput, DiagnosticsPreferences, IdePreferences,
            StartRestoreRunInput, TerminalPreferences, TrayPreferences, UpsertCodexProfileInput,
            UpsertLaunchTaskInput, UpsertProjectInput, UpsertWorkspaceInput,
            WorkspacePreferences,
        },
        services::{
            PlannerService, PreferencesService, ProfileService, RestoreService, WorkspaceService,
        },
    };

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn unique_db_path(name: &str) -> std::path::PathBuf {
        env::temp_dir().join(format!(
            "bexo-studio-{name}-{}.sqlite3",
            uuid::Uuid::new_v4()
        ))
    }

    fn write_shim(path: &std::path::Path, body: &str) {
        fs::write(path, body).expect("write shim");
    }

    #[tokio::test]
    async fn restore_run_executes_terminal_codex_launch_task_and_ide_with_shims() {
        let _guard = env_lock().lock().expect("env lock");
        let original_path = env::var_os("PATH");
        let temp_root =
            env::temp_dir().join(format!("bexo-restore-shims-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_root).expect("create temp root");
        let shim_dir = temp_root.join("bin");
        fs::create_dir_all(&shim_dir).expect("create shim dir");
        let terminal_log = temp_root.join("wt.log");
        let ide_log = temp_root.join("code.log");
        let launch_task_log = temp_root.join("launch-task.log");

        write_shim(
            &shim_dir.join("wt.cmd"),
            &format!(
                "@echo off\r\necho CODEX_HOME=%CODEX_HOME%>>\"{}\"\r\necho ARGS=%*>>\"{}\"\r\nexit /b 0\r\n",
                terminal_log.display(),
                terminal_log.display()
            ),
        );
        write_shim(
            &shim_dir.join("code.cmd"),
            &format!(
                "@echo off\r\necho ARGS=%*>>\"{}\"\r\nexit /b 0\r\n",
                ide_log.display()
            ),
        );
        write_shim(&shim_dir.join("codex.cmd"), "@echo off\r\nexit /b 0\r\n");
        write_shim(
            &shim_dir.join("serve.cmd"),
            &format!(
                "@echo off\r\necho CWD=%CD%>>\"{}\"\r\necho ARGS=%*>>\"{}\"\r\nexit /b 0\r\n",
                launch_task_log.display(),
                launch_task_log.display()
            ),
        );

        let mut path_entries = vec![shim_dir.clone()];
        if let Some(existing) = original_path.as_ref() {
            path_entries.extend(env::split_paths(existing));
        }
        let joined_path = env::join_paths(path_entries).expect("join paths");
        env::set_var("PATH", joined_path);

        let database = crate::persistence::Database::new(unique_db_path("restore-service"));
        database.initialize().await.expect("db init");

        let log_store = crate::logging::RestoreLogStore::new(temp_root.join("logs"));
        let workspace_service = WorkspaceService::new(database.clone());
        let profile_service = ProfileService::new(database.clone());
        let planner_service = PlannerService::new(database.clone(), log_store.clone());
        let restore_service = RestoreService::new(database.clone(), log_store.clone());
        let preferences_service = PreferencesService::new();

        let workspace = workspace_service
            .upsert_workspace(UpsertWorkspaceInput {
                id: None,
                name: "Restore Workspace".into(),
                description: None,
                icon: None,
                color: Some("#12A3FF".into()),
                sort_order: Some(0),
                is_default: Some(false),
                is_archived: Some(false),
            })
            .await
            .expect("create workspace");

        let codex_home = temp_root.join("codex-home");
        fs::create_dir_all(&codex_home).expect("create codex home");

        let profile = profile_service
            .upsert_codex_profile(UpsertCodexProfileInput {
                id: None,
                name: "Restore Profile".into(),
                description: None,
                codex_home: codex_home.display().to_string(),
                startup_mode: "resume_last".into(),
                resume_strategy: "resume_last".into(),
                default_args: vec!["--model".into(), "gpt-5".into()],
                is_default: Some(true),
            })
            .await
            .expect("create profile");

        let project_dir = temp_root.join("project");
        fs::create_dir_all(&project_dir).expect("create project dir");

        let project = workspace_service
            .upsert_project(UpsertProjectInput {
                id: None,
                workspace_id: workspace.id.clone(),
                name: "Restore Project".into(),
                path: project_dir.display().to_string(),
                platform: "windows".into(),
                terminal_type: "windows_terminal".into(),
                ide_type: Some("vscode".into()),
                codex_profile_id: Some(profile.id.clone()),
                open_terminal: true,
                open_ide: true,
                auto_resume_codex: true,
                sort_order: Some(0),
            })
            .await
            .expect("create project");

        workspace_service
            .upsert_launch_task(UpsertLaunchTaskInput {
                id: None,
                project_id: project.id.clone(),
                name: "Start Dev Server".into(),
                task_type: "terminal_command".into(),
                enabled: Some(true),
                command: shim_dir.join("serve.cmd").display().to_string(),
                args: vec!["--port".into(), "3030".into()],
                working_dir: Some(project_dir.display().to_string()),
                timeout_ms: Some(2_000),
                continue_on_failure: Some(false),
                retry_policy: None,
                sort_order: Some(0),
            })
            .await
            .expect("create launch task");

        let snapshot = planner_service
            .create_snapshot(CreateSnapshotInput {
                workspace_id: workspace.id,
                name: "Restore Snapshot".into(),
                description: None,
            })
            .await
            .expect("create snapshot");

        let detail = restore_service
            .start_restore_run(
                StartRestoreRunInput {
                    snapshot_id: snapshot.id,
                    mode: "full".into(),
                },
                &preferences_service,
            )
            .await
            .expect("start restore run");

        assert_eq!(detail.run.status, "completed");
        assert_eq!(detail.tasks.len(), 1);
        assert_eq!(detail.tasks[0].status, "completed");
        assert_eq!(detail.tasks[0].actions.len(), 4);
        assert!(detail.tasks[0]
            .actions
            .iter()
            .all(|action| action.status == "completed"));

        let terminal_payload = fs::read_to_string(&terminal_log).expect("read terminal log");
        assert!(terminal_payload.contains("CODEX_HOME="));
        assert!(terminal_payload.contains("resume"));
        assert!(terminal_payload.contains("--last"));
        assert!(terminal_payload.contains(&project_dir.display().to_string()));

        let ide_payload = fs::read_to_string(&ide_log).expect("read ide log");
        assert!(ide_payload.contains(&project_dir.display().to_string()));

        let launch_task_payload =
            fs::read_to_string(&launch_task_log).expect("read launch task log");
        assert!(launch_task_payload.contains("CWD="));
        assert!(launch_task_payload.contains(&project_dir.display().to_string()));
        assert!(launch_task_payload.contains("--port 3030"));

        let log_files = fs::read_dir(temp_root.join("logs"))
            .expect("read log dir")
            .count();
        assert!(log_files >= 1);

        if let Some(value) = original_path {
            env::set_var("PATH", value);
        } else {
            env::remove_var("PATH");
        }
    }

    #[tokio::test]
    async fn restore_capabilities_prefer_user_configured_paths() {
        let temp_root =
            env::temp_dir().join(format!("bexo-configured-tools-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_root).expect("create temp root");

        let wt_dir = temp_root.join("wt");
        let code_dir = temp_root.join("code");
        let codex_dir = temp_root.join("codex");
        let idea_dir = temp_root.join("idea");
        fs::create_dir_all(&wt_dir).expect("create wt dir");
        fs::create_dir_all(&code_dir).expect("create code dir");
        fs::create_dir_all(&codex_dir).expect("create codex dir");
        fs::create_dir_all(&idea_dir).expect("create idea dir");

        write_shim(&wt_dir.join("wt.cmd"), "@echo off\r\nexit /b 0\r\n");
        write_shim(&code_dir.join("code.cmd"), "@echo off\r\nexit /b 0\r\n");
        write_shim(&codex_dir.join("codex.cmd"), "@echo off\r\nexit /b 0\r\n");
        write_shim(&idea_dir.join("idea.cmd"), "@echo off\r\nexit /b 0\r\n");

        let preferences_service = PreferencesService::new();
        preferences_service.hydrate_for_test(AppPreferences {
            terminal: TerminalPreferences {
                windows_terminal_path: Some(wt_dir.display().to_string()),
                codex_cli_path: Some(codex_dir.display().to_string()),
                command_templates: Vec::new(),
            },
            ide: IdePreferences {
                vscode_path: Some(code_dir.display().to_string()),
                jetbrains_path: Some(idea_dir.display().to_string()),
            },
            workspace: WorkspacePreferences::default(),
            tray: TrayPreferences::default(),
            diagnostics: DiagnosticsPreferences::default(),
        });

        let database = crate::persistence::Database::new(unique_db_path("restore-capabilities"));
        database.initialize().await.expect("db init");
        let restore_service = RestoreService::new(
            database,
            crate::logging::RestoreLogStore::new(temp_root.join("logs")),
        );

        let capabilities = restore_service
            .get_restore_capabilities(&preferences_service)
            .await
            .expect("get capabilities");

        assert_eq!(capabilities.terminal.source, "user_config");
        assert!(capabilities
            .terminal
            .executable_path
            .unwrap()
            .ends_with("wt.cmd"));
        assert_eq!(capabilities.vscode.source, "user_config");
        assert!(capabilities
            .vscode
            .executable_path
            .unwrap()
            .ends_with("code.cmd"));
        assert_eq!(capabilities.codex.source, "user_config");
        assert!(capabilities
            .codex
            .executable_path
            .unwrap()
            .ends_with("codex.cmd"));
        assert_eq!(capabilities.jetbrains.source, "user_config");
        assert!(capabilities
            .jetbrains
            .executable_path
            .unwrap()
            .ends_with("idea.cmd"));
    }
}
