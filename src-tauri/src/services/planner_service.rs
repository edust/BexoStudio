use std::collections::HashMap;

use chrono::Utc;
use rusqlite::Connection;

use crate::{
    domain::{
        parse_restore_mode, CodexProfileRecord, CreateSnapshotInput, RestoreActionPlan,
        RestorePreview, RestorePreviewInput, RestorePreviewStats, RestoreProjectPlan,
        RestoreRunDetail, RestoreRunProjectRecord, SnapshotCodexProfilePayload,
        SnapshotLaunchTaskPayload, SnapshotPayload, SnapshotProjectPayload, SnapshotRecord,
        SnapshotWorkspacePayload, StartRestoreDryRunInput, UpdateSnapshotInput, WorkspaceRecord,
    },
    error::{AppError, AppResult},
    logging::RestoreLogStore,
    persistence::{
        create_snapshot as persist_snapshot, get_restore_run_summary, get_snapshot,
        insert_restore_dry_run, list_codex_profiles, list_restore_run_tasks,
        list_snapshots as persist_list_snapshots, list_workspaces,
        update_snapshot as persist_update_snapshot, Database,
    },
};

#[derive(Debug, Clone)]
pub struct PlannerService {
    database: Database,
    restore_log_store: RestoreLogStore,
}

impl PlannerService {
    pub fn new(database: Database, restore_log_store: RestoreLogStore) -> Self {
        Self {
            database,
            restore_log_store,
        }
    }

    pub async fn list_snapshots(
        &self,
        workspace_id: Option<String>,
    ) -> AppResult<Vec<SnapshotRecord>> {
        self.database
            .read("list_snapshots", move |connection| {
                persist_list_snapshots(connection, workspace_id)
            })
            .await
    }

    pub async fn create_snapshot(&self, input: CreateSnapshotInput) -> AppResult<SnapshotRecord> {
        self.database
            .write("create_snapshot", move |connection| {
                let workspace_id = input.workspace_id.clone();
                let workspaces = list_workspaces(connection)?;
                let workspace = workspaces
                    .into_iter()
                    .find(|workspace| workspace.id == workspace_id)
                    .ok_or_else(|| {
                        AppError::new("WORKSPACE_NOT_FOUND", "workspace was not found")
                            .with_detail("workspaceId", workspace_id.clone())
                    })?;
                let profiles = list_codex_profiles(connection)?;
                let payload = build_snapshot_payload(&workspace, &profiles);
                persist_snapshot(connection, input, payload)
            })
            .await
    }

    pub async fn update_snapshot(&self, input: UpdateSnapshotInput) -> AppResult<SnapshotRecord> {
        self.database
            .write("update_snapshot", move |connection| {
                persist_update_snapshot(connection, input)
            })
            .await
    }

    pub async fn preview_restore(&self, input: RestorePreviewInput) -> AppResult<RestorePreview> {
        self.database
            .read("preview_restore", move |connection| {
                let snapshot = get_snapshot(connection, input.snapshot_id.clone())?;
                build_restore_preview(snapshot, input.mode)
            })
            .await
    }

    pub async fn start_restore_dry_run(
        &self,
        input: StartRestoreDryRunInput,
    ) -> AppResult<RestoreRunDetail> {
        let detail = self
            .database
            .write("start_restore_dry_run", move |connection| {
                let snapshot = get_snapshot(connection, input.snapshot_id.clone())?;
                let preview = build_restore_preview(snapshot.clone(), input.mode.clone())?;
                let run_id = insert_restore_dry_run(
                    connection,
                    &snapshot,
                    &preview.mode,
                    &preview.projects,
                )?;
                build_restore_run_detail(connection, run_id)
            })
            .await?;

        if let Err(error) = self
            .restore_log_store
            .write_run_detail(detail.clone())
            .await
        {
            log::error!(
                target: "bexo::service::planner",
                "failed to write restore run log detail: {}",
                error
            );
        }

        Ok(detail)
    }
}

fn build_snapshot_payload(
    workspace: &WorkspaceRecord,
    profiles: &[CodexProfileRecord],
) -> SnapshotPayload {
    let profiles_by_id = profiles
        .iter()
        .map(|profile| (profile.id.as_str(), profile))
        .collect::<HashMap<_, _>>();

    let mut projects = workspace
        .projects
        .iter()
        .map(|project| SnapshotProjectPayload {
            id: project.id.clone(),
            name: project.name.clone(),
            path: project.path.clone(),
            platform: project.platform.clone(),
            terminal_type: project.terminal_type.clone(),
            ide_type: project.ide_type.clone(),
            open_terminal: project.open_terminal,
            open_ide: project.open_ide,
            auto_resume_codex: project.auto_resume_codex,
            sort_order: project.sort_order,
            codex_profile: project
                .codex_profile_id
                .as_deref()
                .and_then(|profile_id| profiles_by_id.get(profile_id))
                .map(|profile| SnapshotCodexProfilePayload {
                    id: profile.id.clone(),
                    name: profile.name.clone(),
                    codex_home: profile.codex_home.clone(),
                    startup_mode: profile.startup_mode.clone(),
                    resume_strategy: profile.resume_strategy.clone(),
                    default_args: profile.default_args.clone(),
                }),
            launch_tasks: project
                .launch_tasks
                .iter()
                .map(|task| SnapshotLaunchTaskPayload {
                    id: task.id.clone(),
                    name: task.name.clone(),
                    task_type: task.task_type.clone(),
                    enabled: task.enabled,
                    command: task.command.clone(),
                    args: task.args.clone(),
                    working_dir: task.working_dir.clone(),
                    timeout_ms: task.timeout_ms,
                    continue_on_failure: task.continue_on_failure,
                    retry_policy: task.retry_policy.clone(),
                    sort_order: task.sort_order,
                })
                .collect(),
        })
        .collect::<Vec<_>>();

    projects.sort_by_key(|project| project.sort_order);

    SnapshotPayload {
        workspace: SnapshotWorkspacePayload {
            id: workspace.id.clone(),
            name: workspace.name.clone(),
            description: workspace.description.clone(),
            icon: workspace.icon.clone(),
            color: workspace.color.clone(),
        },
        projects,
        captured_at: Utc::now().to_rfc3339(),
    }
}

pub(crate) fn build_restore_preview(
    snapshot: SnapshotRecord,
    mode: String,
) -> AppResult<RestorePreview> {
    let mode = parse_restore_mode(&mode)?;
    let projects = snapshot
        .payload
        .projects
        .iter()
        .map(|project| build_project_plan(project, &mode))
        .collect::<Vec<_>>();
    let stats = build_restore_stats(&projects);

    Ok(RestorePreview {
        snapshot,
        mode,
        stats,
        projects,
    })
}

fn build_project_plan(project: &SnapshotProjectPayload, mode: &str) -> RestoreProjectPlan {
    let mut actions = vec![
        build_terminal_action(project, mode),
        build_codex_action(project, mode),
    ];
    actions.extend(build_launch_task_actions(project, mode));
    actions.push(build_ide_action(project, mode));

    let (status, reason) = summarize_project_status(&actions);

    RestoreProjectPlan {
        project_id: project.id.clone(),
        project_name: project.name.clone(),
        path: project.path.clone(),
        status,
        reason,
        actions,
    }
}

fn build_terminal_action(project: &SnapshotProjectPayload, mode: &str) -> RestoreActionPlan {
    if !project.open_terminal {
        return skipped_action(
            "builtin:terminal_context",
            "terminal_context",
            "终端上下文已关闭",
            &project.terminal_type,
            "项目未开启自动打开终端",
        );
    }

    if !matches!(mode, "full" | "terminals_only") {
        return skipped_action(
            "builtin:terminal_context",
            "terminal_context",
            "终端上下文未纳入当前恢复模式",
            &project.terminal_type,
            "当前恢复模式未包含终端",
        );
    }

    planned_action(
        "builtin:terminal_context",
        "terminal_context",
        "打开终端上下文",
        &project.terminal_type,
    )
}

fn build_codex_action(project: &SnapshotProjectPayload, mode: &str) -> RestoreActionPlan {
    if !project.auto_resume_codex {
        return skipped_action(
            "builtin:codex_session",
            "codex_session",
            "Codex 恢复已关闭",
            "codex",
            "项目未开启自动恢复 Codex",
        );
    }

    if !matches!(mode, "full" | "codex_only") {
        return skipped_action(
            "builtin:codex_session",
            "codex_session",
            "Codex 恢复未纳入当前恢复模式",
            "codex",
            "当前恢复模式未包含 Codex",
        );
    }

    let Some(profile) = &project.codex_profile else {
        return blocked_action(
            "builtin:codex_session",
            "codex_session",
            "Codex Profile 缺失",
            "codex",
            "项目未绑定 Codex Profile",
        );
    };

    match profile.startup_mode.as_str() {
        "terminal_only" => skipped_action(
            "builtin:codex_session",
            "codex_session",
            "Codex Profile 配置为仅终端",
            "codex",
            "Profile 启动模式为仅终端",
        ),
        "run_codex" => planned_action(
            "builtin:codex_session",
            "codex_session",
            "启动 Codex CLI",
            "codex",
        ),
        "resume_last" => planned_action(
            "builtin:codex_session",
            "codex_session",
            "恢复最近 Codex 会话",
            "codex",
        ),
        other => blocked_action(
            "builtin:codex_session",
            "codex_session",
            "Codex 启动模式无效",
            "codex",
            &format!("未知的 Profile 启动模式：{other}"),
        ),
    }
}

fn build_ide_action(project: &SnapshotProjectPayload, mode: &str) -> RestoreActionPlan {
    if !project.open_ide {
        return skipped_action(
            "builtin:ide_window",
            "ide_window",
            "IDE 打开已关闭",
            "ide",
            "项目未开启自动打开 IDE",
        );
    }

    if !matches!(mode, "full" | "ide_only") {
        return skipped_action(
            "builtin:ide_window",
            "ide_window",
            "IDE 打开未纳入当前恢复模式",
            "ide",
            "当前恢复模式未包含 IDE",
        );
    }

    let Some(ide_type) = project.ide_type.as_deref() else {
        return blocked_action(
            "builtin:ide_window",
            "ide_window",
            "IDE 类型缺失",
            "ide",
            "项目未配置 IDE 类型",
        );
    };

    planned_action(
        "builtin:ide_window",
        "ide_window",
        "打开 IDE 工作区",
        ide_type,
    )
}

fn build_launch_task_actions(
    project: &SnapshotProjectPayload,
    mode: &str,
) -> Vec<RestoreActionPlan> {
    let mut launch_tasks = project.launch_tasks.clone();
    launch_tasks.sort_by_key(|task| task.sort_order);

    launch_tasks
        .iter()
        .map(|task| build_launch_task_action(project, task, mode))
        .collect()
}

fn build_launch_task_action(
    project: &SnapshotProjectPayload,
    task: &SnapshotLaunchTaskPayload,
    mode: &str,
) -> RestoreActionPlan {
    if !task.enabled {
        return launch_task_action(task, "skipped", Some("启动任务已禁用".to_string()));
    }

    let included_in_mode = matches!(
        (task.task_type.as_str(), mode),
        ("terminal_command" | "open_path", "full" | "terminals_only")
            | ("codex", "full" | "codex_only")
            | ("ide", "full" | "ide_only")
    );
    if !included_in_mode {
        return launch_task_action(
            task,
            "skipped",
            Some("当前恢复模式未包含此启动任务".to_string()),
        );
    }

    if task.task_type == "codex" && project.codex_profile.is_none() {
        return launch_task_action(
            task,
            "blocked",
            Some("项目未绑定 Codex Profile，无法执行 Codex 启动任务".to_string()),
        );
    }

    launch_task_action(task, "planned", None)
}

fn launch_task_action(
    task: &SnapshotLaunchTaskPayload,
    status: &str,
    reason: Option<String>,
) -> RestoreActionPlan {
    RestoreActionPlan {
        id: task.id.clone(),
        kind: "launch_task".to_string(),
        label: task.name.clone(),
        adapter: task.task_type.clone(),
        task_type: Some(task.task_type.clone()),
        launch_task_id: Some(task.id.clone()),
        continue_on_failure: task.continue_on_failure,
        status: status.to_string(),
        reason,
        started_at: None,
        finished_at: None,
        duration_ms: None,
        executable_path: None,
        executable_source: None,
        cancel_requested_at: None,
        diagnostic_code: None,
    }
}

fn planned_action(id: &str, kind: &str, label: &str, adapter: &str) -> RestoreActionPlan {
    RestoreActionPlan {
        id: id.to_string(),
        kind: kind.to_string(),
        label: label.to_string(),
        adapter: adapter.to_string(),
        task_type: None,
        launch_task_id: None,
        continue_on_failure: false,
        status: "planned".to_string(),
        reason: None,
        started_at: None,
        finished_at: None,
        duration_ms: None,
        executable_path: None,
        executable_source: None,
        cancel_requested_at: None,
        diagnostic_code: None,
    }
}

fn skipped_action(
    id: &str,
    kind: &str,
    label: &str,
    adapter: &str,
    reason: &str,
) -> RestoreActionPlan {
    RestoreActionPlan {
        id: id.to_string(),
        kind: kind.to_string(),
        label: label.to_string(),
        adapter: adapter.to_string(),
        task_type: None,
        launch_task_id: None,
        continue_on_failure: false,
        status: "skipped".to_string(),
        reason: Some(reason.to_string()),
        started_at: None,
        finished_at: None,
        duration_ms: None,
        executable_path: None,
        executable_source: None,
        cancel_requested_at: None,
        diagnostic_code: None,
    }
}

fn blocked_action(
    id: &str,
    kind: &str,
    label: &str,
    adapter: &str,
    reason: &str,
) -> RestoreActionPlan {
    RestoreActionPlan {
        id: id.to_string(),
        kind: kind.to_string(),
        label: label.to_string(),
        adapter: adapter.to_string(),
        task_type: None,
        launch_task_id: None,
        continue_on_failure: false,
        status: "blocked".to_string(),
        reason: Some(reason.to_string()),
        started_at: None,
        finished_at: None,
        duration_ms: None,
        executable_path: None,
        executable_source: None,
        cancel_requested_at: None,
        diagnostic_code: None,
    }
}

fn summarize_project_status(actions: &[RestoreActionPlan]) -> (String, Option<String>) {
    if let Some(action) = actions.iter().find(|action| action.status == "blocked") {
        return ("blocked".to_string(), action.reason.clone());
    }

    if actions.iter().any(|action| action.status == "planned") {
        return ("planned".to_string(), None);
    }

    (
        "skipped".to_string(),
        actions.iter().find_map(|action| action.reason.clone()),
    )
}

fn build_restore_stats(projects: &[RestoreProjectPlan]) -> RestorePreviewStats {
    let mut stats = RestorePreviewStats {
        total_projects: projects.len() as i64,
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

    for project in projects {
        match project.status.as_str() {
            "planned" => stats.planned_projects += 1,
            "running" => stats.running_projects += 1,
            "completed" => stats.completed_projects += 1,
            "cancelled" => stats.cancelled_projects += 1,
            "failed" => stats.failed_projects += 1,
            "blocked" => stats.blocked_projects += 1,
            _ => stats.skipped_projects += 1,
        }

        for action in &project.actions {
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

pub(crate) fn build_restore_run_detail(
    connection: &Connection,
    id: String,
) -> AppResult<RestoreRunDetail> {
    let run = get_restore_run_summary(connection, id.clone())?;
    let snapshot_id = run.snapshot_id.clone().ok_or_else(|| {
        AppError::new(
            "RESTORE_RUN_ORPHANED",
            "restore run is missing snapshot reference",
        )
        .with_detail("restoreRunId", id.clone())
    })?;
    let snapshot = get_snapshot(connection, snapshot_id)?;
    let preview_mode = extract_restore_mode(&run.run_mode)?;
    let preview = build_restore_preview(snapshot.clone(), preview_mode)?;
    let task_rows = list_restore_run_tasks(connection, id)?;
    let mut task_by_project = HashMap::new();
    let mut task_by_launch_task = HashMap::new();
    for task in task_rows {
        if let Some(launch_task_id) = task.launch_task_id.clone() {
            task_by_launch_task.insert(launch_task_id, task);
        } else if let Some(project_id) = task.project_id.clone() {
            task_by_project.insert(project_id, task);
        }
    }

    let tasks = preview
        .projects
        .iter()
        .map(|project| {
            let task = task_by_project.get(&project.project_id);
            let actions = project
                .actions
                .iter()
                .map(|action| {
                    action
                        .launch_task_id
                        .as_ref()
                        .and_then(|launch_task_id| task_by_launch_task.get(launch_task_id))
                        .map(|row| merge_launch_task_state(action, row))
                        .unwrap_or_else(|| action.clone())
                })
                .collect::<Vec<_>>();
            RestoreRunProjectRecord {
                id: task
                    .map(|task| task.id.clone())
                    .unwrap_or_else(|| format!("preview:{}", project.project_id)),
                restore_run_id: run.id.clone(),
                project_id: Some(project.project_id.clone()),
                project_name: project.project_name.clone(),
                path: project.path.clone(),
                status: task
                    .map(|task| task.status.clone())
                    .unwrap_or_else(|| project.status.clone()),
                attempt_count: task.map(|task| task.attempt_count).unwrap_or(0),
                started_at: task.and_then(|task| task.started_at.clone()),
                finished_at: task.and_then(|task| task.finished_at.clone()),
                error_message: task
                    .and_then(|task| task.error_message.clone())
                    .or_else(|| project.reason.clone()),
                actions,
            }
        })
        .collect::<Vec<_>>();
    let stats = build_restore_run_detail_stats(&tasks);

    Ok(RestoreRunDetail {
        run,
        snapshot,
        stats,
        tasks,
    })
}

fn extract_restore_mode(run_mode: &str) -> AppResult<String> {
    parse_restore_mode(run_mode.strip_prefix("dry_run:").unwrap_or(run_mode))
}

fn merge_launch_task_state(
    action: &RestoreActionPlan,
    row: &crate::domain::RestoreRunTaskRecord,
) -> RestoreActionPlan {
    let mut merged = action.clone();
    merged.status = row.status.clone();
    merged.reason = row.error_message.clone().or_else(|| merged.reason.clone());
    merged.started_at = row.started_at.clone().or_else(|| merged.started_at.clone());
    merged.finished_at = row
        .finished_at
        .clone()
        .or_else(|| merged.finished_at.clone());
    merged.cancel_requested_at = merged.cancel_requested_at.clone();
    merged.diagnostic_code = merged.diagnostic_code.clone();
    merged
}

fn build_restore_run_detail_stats(tasks: &[RestoreRunProjectRecord]) -> RestorePreviewStats {
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

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use crate::{
        domain::{
            CreateSnapshotInput, RestorePreviewInput, StartRestoreDryRunInput, UpdateSnapshotInput,
            UpsertCodexProfileInput, UpsertProjectInput, UpsertWorkspaceInput,
        },
        services::{PlannerService, ProfileService, RestoreService, WorkspaceService},
    };

    fn unique_db_path(name: &str) -> std::path::PathBuf {
        env::temp_dir().join(format!(
            "bexo-studio-{name}-{}.sqlite3",
            uuid::Uuid::new_v4()
        ))
    }

    #[tokio::test]
    async fn snapshot_preview_and_dry_run_roundtrip() {
        let database = crate::persistence::Database::new(unique_db_path("planner"));
        database.initialize().await.expect("db init");

        let workspace_service = WorkspaceService::new(database.clone());
        let profile_service = ProfileService::new(database.clone());
        let planner_service = PlannerService::new(
            database.clone(),
            crate::logging::RestoreLogStore::new(
                env::temp_dir().join(format!("bexo-planner-logs-{}", uuid::Uuid::new_v4())),
            ),
        );
        let restore_service = RestoreService::new(
            database.clone(),
            crate::logging::RestoreLogStore::new(
                env::temp_dir().join(format!("bexo-planner-logs-{}", uuid::Uuid::new_v4())),
            ),
        );

        let workspace = workspace_service
            .upsert_workspace(UpsertWorkspaceInput {
                id: None,
                name: "Workspace Planner".into(),
                description: Some("Planner workspace".into()),
                icon: Some("box".into()),
                color: Some("#12A3FF".into()),
                sort_order: Some(0),
                is_default: Some(false),
                is_archived: Some(false),
            })
            .await
            .expect("create workspace");

        let codex_home = env::temp_dir().join(format!("bexo-codex-home-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&codex_home).expect("create codex home");

        let profile = profile_service
            .upsert_codex_profile(UpsertCodexProfileInput {
                id: None,
                name: "Planner Profile".into(),
                description: Some("For planner tests".into()),
                codex_home: codex_home.display().to_string(),
                startup_mode: "resume_last".into(),
                resume_strategy: "resume_last".into(),
                default_args: vec!["--model".into(), "gpt-5".into()],
                is_default: Some(true),
            })
            .await
            .expect("create profile");

        let project_directory =
            env::temp_dir().join(format!("bexo-planner-project-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&project_directory).expect("create project directory");

        workspace_service
            .upsert_project(UpsertProjectInput {
                id: None,
                workspace_id: workspace.id.clone(),
                name: "Planner Project".into(),
                path: project_directory.display().to_string(),
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

        let snapshot = planner_service
            .create_snapshot(CreateSnapshotInput {
                workspace_id: workspace.id.clone(),
                name: "Morning Restore".into(),
                description: Some("Daily working set".into()),
            })
            .await
            .expect("create snapshot");

        assert_eq!(snapshot.project_count, 1);
        assert_eq!(
            snapshot.payload.projects[0]
                .codex_profile
                .as_ref()
                .map(|profile| profile.id.as_str()),
            Some(profile.id.as_str())
        );

        let updated_snapshot = planner_service
            .update_snapshot(UpdateSnapshotInput {
                id: snapshot.id.clone(),
                name: "Morning Restore Revised".into(),
                description: Some("Updated".into()),
            })
            .await
            .expect("update snapshot");

        assert_eq!(updated_snapshot.name, "Morning Restore Revised");

        let preview = planner_service
            .preview_restore(RestorePreviewInput {
                snapshot_id: updated_snapshot.id.clone(),
                mode: "full".into(),
            })
            .await
            .expect("preview restore");

        assert_eq!(preview.projects.len(), 1);
        assert_eq!(preview.stats.planned_actions, 3);
        assert_eq!(preview.projects[0].status, "planned");

        let run_detail = planner_service
            .start_restore_dry_run(StartRestoreDryRunInput {
                snapshot_id: updated_snapshot.id.clone(),
                mode: "full".into(),
            })
            .await
            .expect("start dry run");

        assert_eq!(run_detail.run.run_mode, "dry_run:full");
        assert_eq!(run_detail.tasks.len(), 1);
        assert_eq!(run_detail.tasks[0].status, "planned");

        let runs = restore_service
            .list_restore_runs()
            .await
            .expect("list runs");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].planned_task_count, 1);
    }

    #[tokio::test]
    async fn create_snapshot_requires_projects() {
        let database = crate::persistence::Database::new(unique_db_path("snapshot-empty"));
        database.initialize().await.expect("db init");

        let workspace_service = WorkspaceService::new(database.clone());
        let planner_service = PlannerService::new(
            database,
            crate::logging::RestoreLogStore::new(
                env::temp_dir().join(format!("bexo-planner-logs-{}", uuid::Uuid::new_v4())),
            ),
        );

        let workspace = workspace_service
            .upsert_workspace(UpsertWorkspaceInput {
                id: None,
                name: "Empty Workspace".into(),
                description: None,
                icon: None,
                color: Some("#12A3FF".into()),
                sort_order: Some(0),
                is_default: Some(false),
                is_archived: Some(false),
            })
            .await
            .expect("create workspace");

        let error = planner_service
            .create_snapshot(CreateSnapshotInput {
                workspace_id: workspace.id,
                name: "Should Fail".into(),
                description: None,
            })
            .await
            .expect_err("snapshot should fail");

        assert_eq!(error.code, "SNAPSHOT_SOURCE_EMPTY");
    }
}
