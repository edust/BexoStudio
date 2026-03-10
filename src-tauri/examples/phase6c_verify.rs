use std::{env, path::PathBuf, time::Duration};

use bexo_studio_lib::automation_support::{
    AppPreferences, CreateSnapshotInput, Database, DiagnosticsPreferences, IdePreferences,
    PlannerService, PreferencesService, RestoreLogStore, RestorePreviewInput, RestoreService,
    StartRestoreRunInput, TerminalPreferences, TrayPreferences, UpsertLaunchTaskInput,
    UpsertProjectInput, UpsertWorkspaceInput, WorkspacePreferences, WorkspaceService,
};
use serde_json::json;
use tokio::time::sleep;

fn repo_project_path() -> Result<String, Box<dyn std::error::Error>> {
    let cwd = env::current_dir()?;
    let resolved = if cwd.join("src-tauri").exists() {
        cwd
    } else if cwd.file_name().and_then(|value| value.to_str()) == Some("src-tauri") {
        cwd.parent()
            .map(PathBuf::from)
            .ok_or("failed to resolve repository root")?
    } else {
        cwd
    };
    Ok(resolved.display().to_string())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app_data_dir = PathBuf::from(
        env::var("APPDATA").map_err(|error| format!("APPDATA is unavailable: {error}"))?,
    )
    .join("studio.bexo.desktop");
    std::fs::create_dir_all(&app_data_dir)?;

    let database = Database::new(app_data_dir.join("bexo-studio.sqlite3"));
    database.initialize().await?;

    let workspace_service = WorkspaceService::new(database.clone());
    let planner_service = PlannerService::new(
        database.clone(),
        RestoreLogStore::new(app_data_dir.join("restore-runs")),
    );
    let restore_service = RestoreService::new(
        database.clone(),
        RestoreLogStore::new(app_data_dir.join("restore-runs")),
    );
    let preferences_service = PreferencesService::new();

    let terminal_path =
        r"D:\Downloads\Compressed\Microsoft.WindowsTerminalPreview_1.21.1772.0_x64\terminal-1.21.1772.0"
            .to_string();
    let preferences = AppPreferences {
        terminal: TerminalPreferences {
            windows_terminal_path: Some(terminal_path.clone()),
            codex_cli_path: None,
            command_templates: Vec::new(),
        },
        ide: IdePreferences {
            vscode_path: None,
            jetbrains_path: None,
            custom_editors: Vec::new(),
        },
        workspace: WorkspacePreferences::default(),
        startup: Default::default(),
        tray: TrayPreferences {
            close_to_tray: true,
            show_recent_workspaces: true,
        },
        diagnostics: DiagnosticsPreferences {
            show_adapter_sources: true,
            show_executable_paths: true,
        },
    };
    preferences_service.set_preferences_for_runtime(preferences)?;
    let capabilities = restore_service
        .get_restore_capabilities(&preferences_service)
        .await?;
    if !capabilities.terminal.available {
        return Err(format!(
            "Windows Terminal not available via configured path: {}",
            capabilities.terminal.message
        )
        .into());
    }

    let run_suffix = chrono::Utc::now().format("%Y%m%d%H%M%S").to_string();
    let workspace = workspace_service
        .upsert_workspace(UpsertWorkspaceInput {
            id: None,
            name: format!("Phase6c Verification {run_suffix}"),
            description: Some("Action-level cancel verification run".to_string()),
            icon: Some("square-terminal".to_string()),
            color: Some("#18a957".to_string()),
            sort_order: Some(0),
            is_default: Some(false),
            is_archived: Some(false),
        })
        .await?;
    let project_path = repo_project_path()?;
    let project = workspace_service
        .upsert_project(UpsertProjectInput {
            id: None,
            workspace_id: workspace.id.clone(),
            name: "BexoStudio Runtime Verify".to_string(),
            path: project_path.clone(),
            platform: "windows".to_string(),
            terminal_type: "windows_terminal".to_string(),
            ide_type: None,
            codex_profile_id: None,
            open_terminal: true,
            open_ide: false,
            auto_resume_codex: false,
            sort_order: Some(0),
        })
        .await?;
    let launch_task = workspace_service
        .upsert_launch_task(UpsertLaunchTaskInput {
            id: None,
            project_id: project.id.clone(),
            name: "Long Running PowerShell".to_string(),
            task_type: "terminal_command".to_string(),
            enabled: Some(true),
            command: "powershell.exe".to_string(),
            args: vec![
                "-NoLogo".to_string(),
                "-NoProfile".to_string(),
                "-Command".to_string(),
                "Start-Sleep -Seconds 45".to_string(),
            ],
            working_dir: Some(project_path.clone()),
            timeout_ms: Some(45_000),
            continue_on_failure: Some(false),
            retry_policy: None,
            sort_order: Some(0),
        })
        .await?;

    let snapshot = planner_service
        .create_snapshot(CreateSnapshotInput {
            workspace_id: workspace.id.clone(),
            name: format!("Phase6c Snapshot {run_suffix}"),
            description: Some("Verification snapshot with long-running launch task".to_string()),
        })
        .await?;
    let preview = planner_service
        .preview_restore(RestorePreviewInput {
            snapshot_id: snapshot.id.clone(),
            mode: "full".to_string(),
        })
        .await?;

    let target_project = preview
        .projects
        .first()
        .ok_or("restore preview did not include a project")?;
    let target_action = target_project
        .actions
        .iter()
        .find(|action| action.launch_task_id.as_deref() == Some(launch_task.id.as_str()))
        .ok_or("restore preview did not include the verification launch task action")?
        .id
        .clone();

    let restore_service_for_run = restore_service.clone();
    let preferences_service_for_run = preferences_service.clone();
    let snapshot_id_for_run = snapshot.id.clone();
    let restore_task = tokio::spawn(async move {
        restore_service_for_run
            .start_restore_run(
                StartRestoreRunInput {
                    snapshot_id: snapshot_id_for_run,
                    mode: "full".to_string(),
                },
                &preferences_service_for_run,
            )
            .await
    });

    let mut run_id = None;
    for _ in 0..60 {
        let runs = restore_service.list_restore_runs().await?;
        if let Some(run) = runs
            .iter()
            .find(|run| run.snapshot_id.as_deref() == Some(snapshot.id.as_str()))
        {
            run_id = Some(run.id.clone());
            break;
        }
        sleep(Duration::from_millis(250)).await;
    }
    let run_id = run_id.ok_or("restore run did not appear in time")?;

    let mut project_task_id = None;
    for _ in 0..120 {
        let detail = restore_service
            .get_restore_run_detail(run_id.clone())
            .await?;
        if let Some(task) = detail.tasks.iter().find(|task| {
            task.project_id.as_deref() == Some(project.id.as_str())
                && task
                    .actions
                    .iter()
                    .any(|action| action.id == target_action && action.status == "running")
        }) {
            project_task_id = Some(task.id.clone());
            break;
        }
        sleep(Duration::from_millis(250)).await;
    }
    let project_task_id =
        project_task_id.ok_or("target action did not enter running state in time")?;

    let cancel_result = restore_service
        .cancel_restore_action(
            None,
            run_id.clone(),
            project_task_id.clone(),
            target_action.clone(),
        )
        .await?;
    let detail = restore_task.await??;
    let final_task = detail
        .tasks
        .iter()
        .find(|task| task.id == project_task_id)
        .ok_or("final restore detail missing project task")?;
    let final_action = final_task
        .actions
        .iter()
        .find(|action| action.id == target_action)
        .ok_or("final restore detail missing target action")?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "runId": run_id,
            "workspaceId": workspace.id,
            "projectId": project.id,
            "snapshotId": snapshot.id,
            "configuredTerminalPath": terminal_path,
            "detectedTerminalExecutable": capabilities.terminal.executable_path,
            "detectedTerminalSource": capabilities.terminal.source,
            "cancelResult": cancel_result,
            "runStatus": detail.run.status,
            "projectStatus": final_task.status,
            "actionStatus": final_action.status,
            "actionReason": final_action.reason,
            "actionDiagnosticCode": final_action.diagnostic_code,
            "actionCancelRequestedAt": final_action.cancel_requested_at,
        }))?
    );

    Ok(())
}
