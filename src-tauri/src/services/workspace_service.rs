use std::time::Duration;

use crate::{
    adapters::{
        run_launch_command, IdeAdapter, JetBrainsAdapter, TerminalAdapter, TerminalLaunchInput,
        VSCodeAdapter, WindowsTerminalAdapter, WindowsTerminalTabLaunchInput,
    },
    domain::{
        ensure_absolute_directory, DeleteResult, LaunchTaskRecord, OpenWorkspaceInEditorResult,
        OpenWorkspaceTerminalResult, ProjectRecord, RunWorkspaceTerminalCommandResult,
        RunWorkspaceTerminalCommandsResult, UpsertLaunchTaskInput, UpsertProjectInput,
        UpsertWorkspaceInput, WorkspaceRecord,
    },
    error::{AppError, AppResult},
    persistence::{
        delete_launch_task, delete_workspace, list_launch_tasks, list_workspaces,
        register_workspace_folder, remove_workspace_registration, upsert_launch_task,
        upsert_project, upsert_workspace, Database,
    },
};

use super::PreferencesService;

#[derive(Debug, Clone)]
pub struct WorkspaceService {
    database: Database,
}

impl WorkspaceService {
    const RUN_ALL_STAGGER_MS: u64 = 10_000;

    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub async fn list_workspaces(&self) -> AppResult<Vec<WorkspaceRecord>> {
        self.database.read("list_workspaces", list_workspaces).await
    }

    pub async fn upsert_workspace(
        &self,
        input: UpsertWorkspaceInput,
    ) -> AppResult<WorkspaceRecord> {
        self.database
            .write("upsert_workspace", move |connection| {
                upsert_workspace(connection, input)
            })
            .await
    }

    pub async fn delete_workspace(&self, id: String) -> AppResult<DeleteResult> {
        self.database
            .write("delete_workspace", move |connection| {
                delete_workspace(connection, id)
            })
            .await
    }

    pub async fn register_workspace_folder(&self, path: String) -> AppResult<WorkspaceRecord> {
        self.database
            .write("register_workspace_folder", move |connection| {
                register_workspace_folder(connection, path)
            })
            .await
    }

    pub async fn remove_workspace_registration(&self, id: String) -> AppResult<DeleteResult> {
        self.database
            .write("remove_workspace_registration", move |connection| {
                remove_workspace_registration(connection, id)
            })
            .await
    }

    pub async fn open_workspace_terminal(
        &self,
        workspace_id: String,
        preferences_service: &PreferencesService,
    ) -> AppResult<OpenWorkspaceTerminalResult> {
        let workspace_id = workspace_id.trim().to_string();
        if workspace_id.is_empty() {
            return Err(AppError::validation("workspaceId is required"));
        }

        let workspace = self
            .database
            .read("list_workspaces_for_terminal_launch", list_workspaces)
            .await?
            .into_iter()
            .find(|workspace| workspace.id == workspace_id)
            .ok_or_else(|| {
                AppError::new("WORKSPACE_NOT_FOUND", "workspace was not found")
                    .with_detail("workspaceId", workspace_id.clone())
            })?;

        let workspace_path = resolve_workspace_terminal_path(&workspace)?;
        let preferences = preferences_service.get_preferences()?;
        let terminal_adapter = WindowsTerminalAdapter;
        let availability =
            terminal_adapter.detect(preferences.terminal.windows_terminal_path.as_deref());

        if !availability.available {
            let error_code =
                if availability.source == "user_config" && availability.status == "invalid" {
                    "WINDOWS_TERMINAL_PATH_INVALID"
                } else {
                    "TERMINAL_ADAPTER_UNAVAILABLE"
                };
            let error_message = if error_code == "WINDOWS_TERMINAL_PATH_INVALID" {
                "configured Windows Terminal path is invalid"
            } else {
                "Windows Terminal is not available on this machine"
            };

            return Err(AppError::new(error_code, error_message)
                .with_detail("adapter", "windows_terminal")
                .with_detail("workspaceId", workspace.id.clone())
                .with_detail("workspacePath", workspace_path)
                .with_detail("message", availability.message));
        }

        let executable_path = availability.executable_path.ok_or_else(|| {
            AppError::new(
                "TERMINAL_ADAPTER_UNAVAILABLE",
                "Windows Terminal executable path is missing",
            )
            .with_detail("adapter", "windows_terminal")
            .with_detail("workspaceId", workspace.id.clone())
        })?;

        let launch_command = terminal_adapter.build_launch_plan(
            &executable_path,
            TerminalLaunchInput {
                project_path: workspace_path.clone(),
                startup_command: None,
                envs: Vec::new(),
            },
        )?;

        run_launch_command(launch_command).await?;

        Ok(OpenWorkspaceTerminalResult {
            workspace_id,
            workspace_path,
        })
    }

    pub async fn open_workspace_in_editor(
        &self,
        workspace_id: String,
        editor_key: String,
        preferences_service: &PreferencesService,
    ) -> AppResult<OpenWorkspaceInEditorResult> {
        let workspace_id = require_non_empty_workspace_id(workspace_id)?;
        let editor_key = require_workspace_editor_key(editor_key)?;
        let workspace = self
            .database
            .read("list_workspaces_for_editor_launch", list_workspaces)
            .await?
            .into_iter()
            .find(|workspace| workspace.id == workspace_id)
            .ok_or_else(|| {
                AppError::new("WORKSPACE_NOT_FOUND", "workspace was not found")
                    .with_detail("workspaceId", workspace_id.clone())
            })?;
        let project = resolve_workspace_primary_project(&workspace)?;
        let workspace_path =
            ensure_absolute_directory(project.path.trim(), "INVALID_WORKSPACE_PATH")
                .map_err(|error| error.with_detail("workspaceId", workspace.id.clone()))?;
        let preferences = preferences_service.get_preferences()?;
        let (availability, launch_command) =
            build_workspace_editor_launch_command(&editor_key, &workspace_path, &preferences)?;

        let editor_label = availability.label.clone();
        run_launch_command(launch_command).await?;

        Ok(OpenWorkspaceInEditorResult {
            workspace_id: workspace.id,
            workspace_path,
            editor_key,
            editor_label,
        })
    }

    pub async fn run_workspace_terminal_command(
        &self,
        workspace_id: String,
        launch_task_id: String,
        preferences_service: &PreferencesService,
    ) -> AppResult<RunWorkspaceTerminalCommandResult> {
        let workspace_id = require_non_empty_workspace_id(workspace_id)?;
        let launch_task_id = require_non_empty_launch_task_id(launch_task_id)?;
        let workspace = self
            .database
            .read(
                "list_workspaces_for_single_terminal_command",
                list_workspaces,
            )
            .await?
            .into_iter()
            .find(|workspace| workspace.id == workspace_id)
            .ok_or_else(|| {
                AppError::new("WORKSPACE_NOT_FOUND", "workspace was not found")
                    .with_detail("workspaceId", workspace_id.clone())
            })?;

        let project = resolve_workspace_primary_project(&workspace)?;
        let task = resolve_workspace_terminal_task(&project, &launch_task_id)?;
        let launch_context =
            resolve_terminal_launch_context(preferences_service, &workspace, &project, &task)?;
        let command_line = build_terminal_command_line(&task);
        let launch_command = launch_context.terminal_adapter.build_tab_launch_plan(
            &launch_context.executable_path,
            WindowsTerminalTabLaunchInput {
                project_path: launch_context.working_dir.clone(),
                startup_command: Some(build_windows_shell_startup_command(
                    &launch_context.shell_executable,
                    &command_line,
                )),
                envs: Vec::new(),
                window_target: Some("new".to_string()),
                title: Some(task.name.clone()),
            },
        )?;

        run_launch_command(launch_command).await?;

        Ok(RunWorkspaceTerminalCommandResult {
            workspace_id: workspace.id,
            launch_task_id: task.id,
            workspace_path: launch_context.working_dir,
            command_line,
        })
    }

    pub async fn run_workspace_terminal_commands(
        &self,
        workspace_id: String,
        preferences_service: &PreferencesService,
    ) -> AppResult<RunWorkspaceTerminalCommandsResult> {
        self.run_workspace_terminal_commands_with_stagger(
            workspace_id,
            preferences_service,
            Duration::from_millis(Self::RUN_ALL_STAGGER_MS),
        )
        .await
    }

    async fn run_workspace_terminal_commands_with_stagger(
        &self,
        workspace_id: String,
        preferences_service: &PreferencesService,
        stagger: Duration,
    ) -> AppResult<RunWorkspaceTerminalCommandsResult> {
        let workspace_id = require_non_empty_workspace_id(workspace_id)?;
        let workspace = self
            .database
            .read(
                "list_workspaces_for_terminal_command_batch",
                list_workspaces,
            )
            .await?
            .into_iter()
            .find(|workspace| workspace.id == workspace_id)
            .ok_or_else(|| {
                AppError::new("WORKSPACE_NOT_FOUND", "workspace was not found")
                    .with_detail("workspaceId", workspace_id.clone())
            })?;

        let project = resolve_workspace_primary_project(&workspace)?;
        let tasks = resolve_workspace_terminal_tasks_for_batch(&project)?;
        let launch_context =
            resolve_terminal_launch_context(preferences_service, &workspace, &project, &tasks[0])?;
        let window_target = format!("bexo-workspace-{}-{}", workspace.id, uuid::Uuid::new_v4());

        let mut launched_task_ids = Vec::with_capacity(tasks.len());
        for (index, task) in tasks.iter().enumerate() {
            if index > 0 {
                tokio::time::sleep(stagger).await;
            }

            let command_line = build_terminal_command_line(task);
            let launch_command = launch_context.terminal_adapter.build_tab_launch_plan(
                &launch_context.executable_path,
                WindowsTerminalTabLaunchInput {
                    project_path: resolve_terminal_working_dir_for_task(task, &project)?,
                    startup_command: Some(build_windows_shell_startup_command(
                        &launch_context.shell_executable,
                        &command_line,
                    )),
                    envs: Vec::new(),
                    window_target: Some(window_target.clone()),
                    title: Some(task.name.clone()),
                },
            )?;

            run_launch_command(launch_command).await?;
            launched_task_ids.push(task.id.clone());
        }

        Ok(RunWorkspaceTerminalCommandsResult {
            workspace_id: workspace.id,
            workspace_path: launch_context.working_dir,
            launched_count: launched_task_ids.len(),
            launched_task_ids,
            window_target,
            stagger_ms: stagger.as_millis() as i64,
        })
    }

    pub async fn upsert_project(&self, input: UpsertProjectInput) -> AppResult<ProjectRecord> {
        self.database
            .write("upsert_project", move |connection| {
                upsert_project(connection, input)
            })
            .await
    }

    pub async fn list_launch_tasks(&self, project_id: String) -> AppResult<Vec<LaunchTaskRecord>> {
        self.database
            .read("list_launch_tasks", move |connection| {
                list_launch_tasks(connection, project_id)
            })
            .await
    }

    pub async fn upsert_launch_task(
        &self,
        input: UpsertLaunchTaskInput,
    ) -> AppResult<LaunchTaskRecord> {
        self.database
            .write("upsert_launch_task", move |connection| {
                upsert_launch_task(connection, input)
            })
            .await
    }

    pub async fn delete_launch_task(&self, id: String) -> AppResult<DeleteResult> {
        self.database
            .write("delete_launch_task", move |connection| {
                delete_launch_task(connection, id)
            })
            .await
    }
}

#[derive(Debug, Clone)]
struct TerminalLaunchContext {
    terminal_adapter: WindowsTerminalAdapter,
    executable_path: String,
    shell_executable: String,
    working_dir: String,
}

fn resolve_workspace_terminal_path(workspace: &WorkspaceRecord) -> AppResult<String> {
    let raw_path = workspace
        .projects
        .iter()
        .find_map(|project| {
            let trimmed = project.path.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        })
        .ok_or_else(|| {
            AppError::new(
                "INVALID_WORKSPACE_PATH",
                "workspace does not contain a registered project path",
            )
            .with_detail("workspaceId", workspace.id.clone())
        })?;

    ensure_absolute_directory(&raw_path, "INVALID_WORKSPACE_PATH")
        .map_err(|error| error.with_detail("workspaceId", workspace.id.clone()))
}

fn require_non_empty_workspace_id(workspace_id: String) -> AppResult<String> {
    let normalized = workspace_id.trim().to_string();
    if normalized.is_empty() {
        return Err(AppError::validation("workspaceId is required"));
    }
    Ok(normalized)
}

fn require_non_empty_launch_task_id(launch_task_id: String) -> AppResult<String> {
    let normalized = launch_task_id.trim().to_string();
    if normalized.is_empty() {
        return Err(AppError::validation("launchTaskId is required"));
    }
    Ok(normalized)
}

fn require_workspace_editor_key(editor_key: String) -> AppResult<String> {
    let normalized = editor_key.trim().to_lowercase();
    if normalized == "vscode" || normalized == "jetbrains" {
        return Ok(normalized);
    }

    Err(AppError::validation("editorKey must be vscode or jetbrains")
        .with_detail("editorKey", editor_key))
}

fn resolve_workspace_primary_project(workspace: &WorkspaceRecord) -> AppResult<ProjectRecord> {
    workspace
        .projects
        .iter()
        .find(|project| !project.path.trim().is_empty())
        .cloned()
        .ok_or_else(|| {
            AppError::new(
                "WORKSPACE_PROJECT_NOT_FOUND",
                "workspace does not contain a registered project",
            )
            .with_detail("workspaceId", workspace.id.clone())
        })
}

fn resolve_workspace_terminal_task(
    project: &ProjectRecord,
    launch_task_id: &str,
) -> AppResult<LaunchTaskRecord> {
    let task = project
        .launch_tasks
        .iter()
        .find(|task| task.id == launch_task_id)
        .cloned()
        .ok_or_else(|| {
            AppError::new("LAUNCH_TASK_NOT_FOUND", "launch task was not found")
                .with_detail("launchTaskId", launch_task_id.to_string())
                .with_detail("projectId", project.id.clone())
        })?;

    if task.task_type != "terminal_command" {
        return Err(AppError::new(
            "INVALID_LAUNCH_TASK_TYPE",
            "launch task is not a terminal command",
        )
        .with_detail("launchTaskId", task.id.clone())
        .with_detail("taskType", task.task_type));
    }

    if !task.enabled {
        return Err(
            AppError::new("LAUNCH_TASK_DISABLED", "launch task is disabled")
                .with_detail("launchTaskId", task.id.clone()),
        );
    }

    Ok(task)
}

fn resolve_workspace_terminal_tasks_for_batch(
    project: &ProjectRecord,
) -> AppResult<Vec<LaunchTaskRecord>> {
    let tasks = project
        .launch_tasks
        .iter()
        .filter(|task| task.task_type == "terminal_command" && task.enabled)
        .cloned()
        .collect::<Vec<_>>();

    if tasks.is_empty() {
        return Err(AppError::new(
            "NO_TERMINAL_COMMANDS",
            "workspace does not contain enabled terminal commands",
        )
        .with_detail("projectId", project.id.clone()));
    }

    let mut tasks = tasks;
    tasks.sort_by_key(|task| task.sort_order);
    Ok(tasks)
}

fn build_workspace_editor_launch_command(
    editor_key: &str,
    workspace_path: &str,
    preferences: &crate::domain::AppPreferences,
) -> AppResult<(crate::domain::AdapterAvailability, crate::adapters::LaunchCommand)> {
    match editor_key {
        "vscode" => {
            let adapter = VSCodeAdapter;
            let availability = adapter.detect(preferences.ide.vscode_path.as_deref());
            if !availability.available {
                return Err(build_workspace_editor_unavailable_error(
                    "vscode",
                    workspace_path,
                    &availability,
                ));
            }

            let executable_path = availability.executable_path.clone().ok_or_else(|| {
                AppError::new(
                    "IDE_ADAPTER_UNAVAILABLE",
                    "VS Code executable path is missing",
                )
                .with_detail("editorKey", "vscode")
                .with_detail("workspacePath", workspace_path.to_string())
            })?;
            let command = adapter.build_launch_plan(&executable_path, workspace_path)?;
            Ok((availability, command))
        }
        "jetbrains" => {
            let adapter = JetBrainsAdapter;
            let availability = adapter.detect(preferences.ide.jetbrains_path.as_deref());
            if !availability.available {
                return Err(build_workspace_editor_unavailable_error(
                    "jetbrains",
                    workspace_path,
                    &availability,
                ));
            }

            let executable_path = availability.executable_path.clone().ok_or_else(|| {
                AppError::new(
                    "IDE_ADAPTER_UNAVAILABLE",
                    "JetBrains executable path is missing",
                )
                .with_detail("editorKey", "jetbrains")
                .with_detail("workspacePath", workspace_path.to_string())
            })?;
            let command = adapter.build_launch_plan(&executable_path, workspace_path)?;
            Ok((availability, command))
        }
        _ => Err(AppError::validation("editorKey must be vscode or jetbrains")
            .with_detail("editorKey", editor_key.to_string())),
    }
}

fn build_workspace_editor_unavailable_error(
    editor_key: &str,
    workspace_path: &str,
    availability: &crate::domain::AdapterAvailability,
) -> AppError {
    let error_code = if availability.source == "user_config" && availability.status == "invalid" {
        if editor_key == "vscode" {
            "VSCODE_PATH_INVALID"
        } else {
            "JETBRAINS_PATH_INVALID"
        }
    } else {
        "IDE_ADAPTER_UNAVAILABLE"
    };
    let error_message = if editor_key == "vscode" {
        if error_code == "VSCODE_PATH_INVALID" {
            "configured VS Code path is invalid"
        } else {
            "VS Code is not available on this machine"
        }
    } else if error_code == "JETBRAINS_PATH_INVALID" {
        "configured JetBrains path is invalid"
    } else {
        "JetBrains IDE is not available on this machine"
    };

    AppError::new(error_code, error_message)
        .with_detail("editorKey", editor_key.to_string())
        .with_detail("workspacePath", workspace_path.to_string())
        .with_detail("message", availability.message.clone())
}

fn resolve_terminal_launch_context(
    preferences_service: &PreferencesService,
    workspace: &WorkspaceRecord,
    project: &ProjectRecord,
    task: &LaunchTaskRecord,
) -> AppResult<TerminalLaunchContext> {
    let preferences = preferences_service.get_preferences()?;
    let terminal_adapter = WindowsTerminalAdapter;
    let availability =
        terminal_adapter.detect(preferences.terminal.windows_terminal_path.as_deref());

    if !availability.available {
        let error_code = if availability.source == "user_config" && availability.status == "invalid"
        {
            "WINDOWS_TERMINAL_PATH_INVALID"
        } else {
            "TERMINAL_ADAPTER_UNAVAILABLE"
        };
        let error_message = if error_code == "WINDOWS_TERMINAL_PATH_INVALID" {
            "configured Windows Terminal path is invalid"
        } else {
            "Windows Terminal is not available on this machine"
        };

        return Err(AppError::new(error_code, error_message)
            .with_detail("adapter", "windows_terminal")
            .with_detail("workspaceId", workspace.id.clone())
            .with_detail("projectId", project.id.clone())
            .with_detail("launchTaskId", task.id.clone())
            .with_detail("message", availability.message));
    }

    let executable_path = availability.executable_path.ok_or_else(|| {
        AppError::new(
            "TERMINAL_ADAPTER_UNAVAILABLE",
            "Windows Terminal executable path is missing",
        )
        .with_detail("adapter", "windows_terminal")
        .with_detail("workspaceId", workspace.id.clone())
    })?;
    let shell_executable = terminal_adapter
        .detect_shell_executable()
        .ok_or_else(|| {
            AppError::new(
                "SHELL_EXECUTABLE_UNAVAILABLE",
                "PowerShell executable is not available on this machine",
            )
            .with_detail("workspaceId", workspace.id.clone())
            .with_detail("projectId", project.id.clone())
            .with_detail("launchTaskId", task.id.clone())
        })?
        .display()
        .to_string();
    let working_dir = resolve_terminal_working_dir_for_task(task, project)?;

    Ok(TerminalLaunchContext {
        terminal_adapter,
        executable_path,
        shell_executable,
        working_dir,
    })
}

fn resolve_terminal_working_dir_for_task(
    task: &LaunchTaskRecord,
    project: &ProjectRecord,
) -> AppResult<String> {
    let candidate = if task.working_dir.trim().is_empty() {
        project.path.trim().to_string()
    } else {
        task.working_dir.trim().to_string()
    };

    ensure_absolute_directory(&candidate, "INVALID_WORKSPACE_PATH")
        .map_err(|error| error.with_detail("launchTaskId", task.id.clone()))
}

fn build_windows_shell_startup_command(shell_executable: &str, command_line: &str) -> Vec<String> {
    vec![
        shell_executable.to_string(),
        "-NoExit".to_string(),
        "-Command".to_string(),
        command_line.to_string(),
    ]
}

fn build_terminal_command_line(task: &LaunchTaskRecord) -> String {
    let mut segments = Vec::with_capacity(task.args.len() + 1);
    segments.push(quote_if_needed(task.command.trim()));
    segments.extend(task.args.iter().map(|arg| quote_if_needed(arg.trim())));
    segments.join(" ")
}

fn quote_if_needed(value: &str) -> String {
    if !value
        .chars()
        .any(|character| character.is_whitespace() || matches!(character, '"' | '\'' | '\\'))
    {
        return value.to_string();
    }

    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use crate::domain::{
        AppPreferences, DiagnosticsPreferences, IdePreferences, TerminalPreferences,
        TrayPreferences, UpsertLaunchTaskInput, UpsertProjectInput, UpsertWorkspaceInput,
        WorkspacePreferences,
    };

    use super::WorkspaceService;

    fn unique_db_path(name: &str) -> std::path::PathBuf {
        env::temp_dir().join(format!(
            "bexo-studio-{name}-{}.sqlite3",
            uuid::Uuid::new_v4()
        ))
    }

    #[tokio::test]
    async fn workspace_and_project_crud_roundtrip() {
        let database = crate::persistence::Database::new(unique_db_path("workspace"));
        database.initialize().await.expect("db init");

        let service = WorkspaceService::new(database.clone());
        let workspace = service
            .upsert_workspace(UpsertWorkspaceInput {
                id: None,
                name: "Workspace Alpha".into(),
                description: Some("Alpha description".into()),
                icon: Some("folder".into()),
                color: Some("#12A3FF".into()),
                sort_order: Some(1),
                is_default: Some(true),
                is_archived: Some(false),
            })
            .await
            .expect("create workspace");

        let updated_workspace = service
            .upsert_workspace(UpsertWorkspaceInput {
                id: Some(workspace.id.clone()),
                name: "Workspace Alpha Updated".into(),
                description: Some("Updated description".into()),
                icon: Some("folder-open".into()),
                color: Some("#12A3FF".into()),
                sort_order: Some(2),
                is_default: Some(true),
                is_archived: Some(false),
            })
            .await
            .expect("update workspace");

        assert_eq!(updated_workspace.name, "Workspace Alpha Updated");

        let project_directory =
            env::temp_dir().join(format!("bexo-project-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&project_directory).expect("create project directory");

        let project = service
            .upsert_project(UpsertProjectInput {
                id: None,
                workspace_id: workspace.id.clone(),
                name: "Project One".into(),
                path: project_directory.display().to_string(),
                platform: "windows".into(),
                terminal_type: "windows_terminal".into(),
                ide_type: Some("vscode".into()),
                codex_profile_id: None,
                open_terminal: true,
                open_ide: true,
                auto_resume_codex: false,
                sort_order: Some(0),
            })
            .await
            .expect("create project");

        assert_eq!(project.name, "Project One");

        let launch_task = service
            .upsert_launch_task(UpsertLaunchTaskInput {
                id: None,
                project_id: project.id.clone(),
                name: "Dev Server".into(),
                task_type: "terminal_command".into(),
                enabled: Some(true),
                command: "npm".into(),
                args: vec!["run".into(), "dev".into()],
                working_dir: Some(project_directory.display().to_string()),
                timeout_ms: Some(5_000),
                continue_on_failure: Some(false),
                retry_policy: None,
                sort_order: Some(0),
            })
            .await
            .expect("create launch task");

        assert_eq!(launch_task.name, "Dev Server");
        let launch_tasks = service
            .list_launch_tasks(project.id.clone())
            .await
            .expect("list launch tasks");
        assert_eq!(launch_tasks.len(), 1);
        assert_eq!(launch_tasks[0].id, launch_task.id);

        let workspaces = service.list_workspaces().await.expect("list workspaces");
        assert_eq!(workspaces.len(), 1);
        assert_eq!(workspaces[0].projects.len(), 1);
        assert_eq!(workspaces[0].projects[0].id, project.id);
        assert_eq!(workspaces[0].projects[0].launch_tasks.len(), 1);
    }

    #[tokio::test]
    async fn register_workspace_folder_creates_workspace_and_project() {
        let database = crate::persistence::Database::new(unique_db_path("register-workspace"));
        database.initialize().await.expect("db init");

        let service = WorkspaceService::new(database);
        let project_directory =
            env::temp_dir().join(format!("bexo-register-workspace-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&project_directory).expect("create workspace directory");

        let workspace = service
            .register_workspace_folder(project_directory.display().to_string())
            .await
            .expect("register workspace folder");

        assert_eq!(workspace.projects.len(), 1);
        assert_eq!(
            workspace.projects[0].path,
            project_directory.display().to_string()
        );
        assert_eq!(workspace.projects[0].name, workspace.name);
    }

    #[tokio::test]
    async fn remove_workspace_registration_removes_workspace_and_projects() {
        let database = crate::persistence::Database::new(unique_db_path("remove-workspace"));
        database.initialize().await.expect("db init");

        let service = WorkspaceService::new(database);
        let project_directory =
            env::temp_dir().join(format!("bexo-remove-workspace-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&project_directory).expect("create workspace directory");

        let workspace = service
            .register_workspace_folder(project_directory.display().to_string())
            .await
            .expect("register workspace folder");

        let removed = service
            .remove_workspace_registration(workspace.id.clone())
            .await
            .expect("remove workspace registration");

        assert_eq!(removed.id, workspace.id);
        let workspaces = service.list_workspaces().await.expect("list workspaces");
        assert!(workspaces.is_empty());
    }

    #[tokio::test]
    async fn register_workspace_folder_rejects_duplicate_paths() {
        let database = crate::persistence::Database::new(unique_db_path("duplicate-workspace"));
        database.initialize().await.expect("db init");

        let service = WorkspaceService::new(database);
        let project_directory =
            env::temp_dir().join(format!("bexo-duplicate-workspace-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&project_directory).expect("create workspace directory");
        let path = project_directory.display().to_string();

        service
            .register_workspace_folder(path.clone())
            .await
            .expect("first registration");

        let duplicate_error = service
            .register_workspace_folder(path)
            .await
            .expect_err("duplicate registration should fail");

        assert_eq!(duplicate_error.code, "WORKSPACE_PATH_ALREADY_REGISTERED");
    }

    #[tokio::test]
    async fn open_workspace_terminal_uses_configured_windows_terminal() {
        let database = crate::persistence::Database::new(unique_db_path("open-workspace-terminal"));
        database.initialize().await.expect("db init");

        let service = WorkspaceService::new(database);
        let workspace_directory = env::temp_dir().join(format!(
            "bexo-open-workspace-terminal-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&workspace_directory).expect("create workspace directory");

        let shim_directory =
            env::temp_dir().join(format!("bexo-open-terminal-shim-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&shim_directory).expect("create shim directory");
        let log_path = shim_directory.join("wt.log");
        fs::write(
            shim_directory.join("wt.cmd"),
            format!(
                "@echo off\r\necho ARGS=%*>>\"{}\"\r\nexit /b 0\r\n",
                log_path.display()
            ),
        )
        .expect("write wt shim");

        let preferences_service = crate::services::PreferencesService::new();
        preferences_service.hydrate_for_test(AppPreferences {
            terminal: TerminalPreferences {
                windows_terminal_path: Some(shim_directory.display().to_string()),
                codex_cli_path: None,
                command_templates: Vec::new(),
            },
            ide: IdePreferences::default(),
            workspace: WorkspacePreferences::default(),
            tray: TrayPreferences::default(),
            diagnostics: DiagnosticsPreferences::default(),
        });

        let workspace = service
            .register_workspace_folder(workspace_directory.display().to_string())
            .await
            .expect("register workspace folder");

        let result = service
            .open_workspace_terminal(workspace.id.clone(), &preferences_service)
            .await
            .expect("open workspace terminal");

        assert_eq!(result.workspace_id, workspace.id);
        assert_eq!(
            result.workspace_path,
            workspace_directory.display().to_string()
        );

        let terminal_payload = fs::read_to_string(&log_path).expect("read terminal log");
        assert!(terminal_payload.contains("new-tab"));
        assert!(terminal_payload.contains("-d"));
        assert!(terminal_payload.contains(&workspace_directory.display().to_string()));
    }

    #[tokio::test]
    async fn open_workspace_in_editor_uses_configured_vscode() {
        let database = crate::persistence::Database::new(unique_db_path("open-workspace-editor"));
        database.initialize().await.expect("db init");

        let service = WorkspaceService::new(database);
        let workspace_directory = env::temp_dir().join(format!(
            "bexo-open-workspace-editor-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&workspace_directory).expect("create workspace directory");

        let shim_directory =
            env::temp_dir().join(format!("bexo-open-editor-shim-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&shim_directory).expect("create shim directory");
        let log_path = shim_directory.join("code.log");
        fs::write(
            shim_directory.join("code.cmd"),
            format!(
                "@echo off\r\necho ARGS=%*>>\"{}\"\r\nexit /b 0\r\n",
                log_path.display()
            ),
        )
        .expect("write code shim");

        let preferences_service = crate::services::PreferencesService::new();
        preferences_service.hydrate_for_test(AppPreferences {
            terminal: TerminalPreferences::default(),
            ide: IdePreferences {
                vscode_path: Some(shim_directory.display().to_string()),
                jetbrains_path: None,
            },
            workspace: WorkspacePreferences::default(),
            tray: TrayPreferences::default(),
            diagnostics: DiagnosticsPreferences::default(),
        });

        let workspace = service
            .register_workspace_folder(workspace_directory.display().to_string())
            .await
            .expect("register workspace folder");

        let result = service
            .open_workspace_in_editor(workspace.id.clone(), "vscode".into(), &preferences_service)
            .await
            .expect("open workspace in editor");

        assert_eq!(result.workspace_id, workspace.id);
        assert_eq!(result.editor_key, "vscode");
        assert_eq!(
            result.workspace_path,
            workspace_directory.display().to_string()
        );

        let ide_payload = fs::read_to_string(&log_path).expect("read ide log");
        assert!(ide_payload.contains("-n"));
        assert!(ide_payload.contains(&workspace_directory.display().to_string()));
    }
}
