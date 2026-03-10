mod tray;
mod window;

use crate::{commands, logging};
use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;

pub(crate) use tray::refresh_tray_menu;

pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(logging::build_plugin())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_drag::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .on_window_event(window::handle_window_event)
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir().map_err(|error| {
                crate::error::AppError::new(
                    "APP_DATA_DIR_UNAVAILABLE",
                    "failed to resolve app data directory",
                )
                .with_detail("reason", error.to_string())
            })?;
            let database =
                crate::persistence::Database::new(app_data_dir.join("bexo-studio.sqlite3"));
            tauri::async_runtime::block_on(database.initialize())?;
            let restore_log_store =
                crate::logging::RestoreLogStore::new(app_data_dir.join("restore-runs"));
            let preferences_service = crate::services::PreferencesService::new();
            preferences_service.initialize(&app.handle())?;

            app.manage(preferences_service);
            app.manage(crate::services::WorkspaceService::new(database.clone()));
            app.manage(crate::services::ResourceBrowserService::new(
                database.clone(),
            ));
            app.manage(crate::services::ProfileService::new(database.clone()));
            app.manage(crate::services::PlannerService::new(
                database.clone(),
                restore_log_store.clone(),
            ));
            let restore_service = crate::services::RestoreService::new(database, restore_log_store);
            let recovered_runs =
                tauri::async_runtime::block_on(restore_service.recover_interrupted_runs())?;
            if !recovered_runs.is_empty() {
                log::warn!(
                    target: "bexo::app",
                    "recovered {} interrupted restore run(s) on startup",
                    recovered_runs.len()
                );
            }
            app.manage(restore_service);

            app.handle()
                .plugin(tauri_plugin_autostart::init(
                    MacosLauncher::LaunchAgent,
                    Some(vec!["--autostart"]),
                ))
                .map_err(|error| {
                    crate::error::AppError::plugin_init("autostart", error.to_string())
                })?;

            tray::create_tray(app)?;

            let launched_from_autostart = app
                .env()
                .args_os
                .iter()
                .any(|arg| arg.to_string_lossy() == "--autostart");
            let start_silently = app
                .state::<crate::services::PreferencesService>()
                .get_preferences()
                .map(|preferences| preferences.startup.start_silently)
                .unwrap_or(false);
            let should_show_main_window = !(launched_from_autostart && start_silently);

            if let Some(window) = app.get_webview_window("main") {
                if should_show_main_window {
                    let _ = window.show();
                    window::center_main_window_in_work_area(&window);
                    let _ = window.set_focus();
                } else {
                    let _ = window.hide();
                    log::info!(
                        target: "bexo::app",
                        "main window hidden on autostart due to startup.startSilently=true"
                    );
                }
            }

            log::info!(target: "bexo::app", "Bexo Studio bootstrap finished");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap::get_bootstrap_state,
            commands::preferences::get_app_preferences,
            commands::preferences::update_app_preferences,
            commands::preferences::get_codex_home_directory,
            commands::preferences::detect_editors_from_path,
            commands::workspace::list_workspaces,
            commands::workspace::upsert_workspace,
            commands::workspace::delete_workspace,
            commands::workspace::register_workspace_folder,
            commands::workspace::remove_workspace_registration,
            commands::workspace::open_workspace_terminal,
            commands::workspace::open_workspace_in_editor,
            commands::workspace::run_workspace_terminal_command,
            commands::workspace::run_workspace_terminal_commands,
            commands::workspace::upsert_project,
            commands::resource_browser::list_workspace_resource_children,
            commands::resource_browser::allow_workspace_resource_scope,
            commands::resource_browser::get_workspace_resource_git_statuses,
            commands::launch_task::list_launch_tasks,
            commands::launch_task::upsert_launch_task,
            commands::launch_task::delete_launch_task,
            commands::codex_profile::list_codex_profiles,
            commands::codex_profile::upsert_codex_profile,
            commands::snapshot::list_snapshots,
            commands::snapshot::create_snapshot,
            commands::snapshot::update_snapshot,
            commands::restore::preview_restore,
            commands::restore::start_restore_dry_run,
            commands::restore::get_restore_capabilities,
            commands::restore::start_restore_run,
            commands::restore::cancel_restore_run,
            commands::restore::cancel_restore_action,
            commands::restore::list_recent_restore_targets,
            commands::restore::restore_recent_target,
            commands::restore::list_restore_runs,
            commands::restore::get_restore_run_detail,
            commands::restore::open_log_directory
        ]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running Bexo Studio application");
}
