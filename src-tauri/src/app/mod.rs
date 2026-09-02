mod tray;
mod window;

use std::{ffi::OsStr, fs, path::PathBuf};

use crate::{commands, domain::SCREENSHOT_OVERLAY_WINDOW_LABEL, logging};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use tauri::{
    http::{header::CONTENT_TYPE, Response, StatusCode},
    Manager,
};
#[cfg(not(windows))]
use tauri_plugin_autostart::MacosLauncher;

pub(crate) use tray::refresh_tray_menu;

const SCREENSHOT_PREVIEW_PROTOCOL: &str = "bexo-preview";
const SCREENSHOT_PREVIEW_TEMP_DIR_NAME: &str = "bexo-screenshot-preview";
const SCREENSHOT_PREVIEW_MAX_ENCODED_PATH_BYTES: usize = 8_192;

pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if args_indicate_autostart(&_argv) {
                log::info!(
                    target: "bexo::app",
                    "blocked autostart second instance without focusing the running instance"
                );
            } else {
                log::info!(
                    target: "bexo::app",
                    "blocked second instance launch and redirected focus to the running instance"
                );
                focus_main_window(app);
            }
        }))
        .register_uri_scheme_protocol(SCREENSHOT_PREVIEW_PROTOCOL, |context, request| {
            serve_screenshot_preview(context, request)
        })
        .plugin(logging::build_plugin())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_drag::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_denylist(&[SCREENSHOT_OVERLAY_WINDOW_LABEL])
                .build(),
        )
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
            let initial_preferences = preferences_service.initialize(&app.handle())?;
            let native_preview_service = crate::services::NativePreviewService::new();
            if let Err(error) = native_preview_service.initialize(&app.handle()) {
                log::warn!(
                    target: "bexo::app",
                    "initialize native preview service failed: {}",
                    error
                );
                native_preview_service.mark_initialization_failed(error);
            }
            let native_interaction_service = crate::services::NativeInteractionService::new();
            if let Err(error) = native_interaction_service.initialize(&app.handle()) {
                log::warn!(
                    target: "bexo::app",
                    "initialize native interaction service failed: {}",
                    error
                );
                native_interaction_service.mark_initialization_failed(error);
            }
            let screenshot_service = crate::services::ScreenshotService::new();
            let hotkey_service = crate::services::HotkeyService::new();
            let prompt_service = crate::services::PromptService::new(database.clone());
            let prompt_transfer_service =
                crate::services::PromptTransferService::new(database.clone());
            let prompt_quick_paste_service = crate::services::PromptQuickPasteService::new();
            if let Err(error) = screenshot_service.prewarm_overlay_window(&app.handle()) {
                log::warn!(
                    target: "bexo::app",
                    "prewarm screenshot overlay failed: {}",
                    error
                );
            }
            if let Err(error) = screenshot_service.initialize_live_capture(&app.handle()) {
                log::warn!(
                    target: "bexo::app",
                    "initialize live screenshot capture failed: {}",
                    error
                );
            }
            app.manage(native_preview_service);
            app.manage(native_interaction_service);
            app.manage(screenshot_service);
            app.manage(preferences_service.clone());
            app.manage(prompt_service);
            app.manage(prompt_transfer_service);
            app.manage(prompt_quick_paste_service);
            if let Err(error) = hotkey_service.initialize(&app.handle(), &initial_preferences) {
                log::error!(
                    target: "bexo::app",
                    "initialize hotkey service failed: {}",
                    error
                );
            }

            app.manage(hotkey_service);
            app.manage(crate::services::WorkspaceService::new(database.clone()));
            app.manage(crate::services::WorkspaceTransferService::new(database.clone()));
            app.manage(crate::services::ResourceBrowserService::new(
                database.clone(),
            ));
            app.manage(crate::services::CodexHistoryService::new(database.clone()));
            app.manage(crate::services::CodexAuthService::new(
                database.clone(),
                preferences_service.clone(),
            ));
            app.manage(crate::services::ProfileService::new(database.clone()));
            app.manage(crate::services::OssService::new(database.clone())?);
            let oss_transfer_service = crate::services::OssTransferService::new(database.clone())?;
            let recovered_oss_tasks =
                tauri::async_runtime::block_on(oss_transfer_service.recover_interrupted_tasks())?;
            if !recovered_oss_tasks.is_empty() {
                log::warn!(
                    target: "bexo::app",
                    "recovered {} interrupted OSS transfer task(s)",
                    recovered_oss_tasks.len()
                );
            }
            app.manage(oss_transfer_service);
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

            #[cfg(not(windows))]
            app.handle()
                .plugin(tauri_plugin_autostart::init(
                    MacosLauncher::LaunchAgent,
                    Some(vec!["--autostart"]),
                ))
                .map_err(|error| {
                    crate::error::AppError::plugin_init("autostart", error.to_string())
                })?;

            if let Err(error) = app
                .state::<crate::services::PreferencesService>()
                .sync_launch_at_login(&app.handle())
            {
                log::warn!(
                    target: "bexo::app",
                    "sync autostart state from saved preferences failed code={} message={} details={:?}",
                    error.code,
                    error.message,
                    error.details
                );
            }

            tray::create_tray(app)?;

            let launched_from_autostart = args_indicate_autostart(&app.env().args_os);
            let start_silently = app
                .state::<crate::services::PreferencesService>()
                .get_preferences()
                .map(|preferences| preferences.startup.start_silently)
                .unwrap_or(false);
            let should_show_main_window = !(launched_from_autostart && start_silently);
            let current_exe = std::env::current_exe()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|error| format!("unavailable: {error}"));

            log::info!(
                target: "bexo::app",
                "startup visibility resolved launched_from_autostart={} start_silently={} should_show_main_window={} current_exe={}",
                launched_from_autostart,
                start_silently,
                should_show_main_window,
                current_exe
            );

            if let Some(window) = app.get_webview_window("main") {
                if should_show_main_window {
                    window::center_main_window_in_work_area(&window);
                    focus_main_window(&app.handle());
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
            commands::native_interaction::get_native_interaction_state,
            commands::native_interaction::update_native_interaction_exclusion_rects,
            commands::native_interaction::update_native_interaction_runtime,
            commands::preferences::get_app_preferences,
            commands::preferences::update_app_preferences,
            commands::preferences::get_hotkey_health,
            commands::preferences::retry_hotkey_registration,
            commands::preferences::get_codex_home_directory,
            commands::preferences::detect_editors_from_path,
            commands::codex_history::open_codex_history_window,
            commands::codex_history::list_codex_history_sessions,
            commands::codex_history::list_all_codex_history_sessions,
            commands::codex_history::get_codex_history_messages,
            commands::codex_auth::list_codex_auth_profiles,
            commands::codex_auth::get_codex_auth_profile_detail,
            commands::codex_auth::import_current_codex_auth_profile,
            commands::codex_auth::upsert_codex_auth_profile,
            commands::codex_auth::delete_codex_auth_profile,
            commands::codex_auth::switch_codex_auth_profile,
            commands::codex_auth::query_codex_auth_quota,
            commands::codex_auth::refresh_all_codex_auth_quotas,
            commands::prompt::list_prompts,
            commands::prompt::upsert_prompt,
            commands::prompt::delete_prompt,
            commands::prompt::reorder_prompts,
            commands::prompt_transfer::export_prompt_list,
            commands::prompt_transfer::preview_prompt_list_import,
            commands::prompt_transfer::apply_prompt_list_import,
            commands::workspace::list_workspaces,
            commands::workspace::upsert_workspace,
            commands::workspace::update_workspace_description,
            commands::workspace::reorder_workspaces,
            commands::workspace::delete_workspace,
            commands::workspace::register_workspace_folder,
            commands::workspace::remove_workspace_registration,
            commands::workspace::open_workspace_terminal,
            commands::workspace::open_workspace_terminal_at_path,
            commands::workspace::open_workspace_in_editor,
            commands::workspace::run_workspace_terminal_command,
            commands::workspace::run_workspace_terminal_commands,
            commands::workspace::upsert_project,
            commands::workspace_transfer::export_workspace_list,
            commands::workspace_transfer::preview_workspace_list_import,
            commands::workspace_transfer::apply_workspace_list_import,
            commands::resource_browser::list_workspace_resource_children,
            commands::resource_browser::allow_workspace_resource_scope,
            commands::resource_browser::get_workspace_resource_git_statuses,
            commands::launch_task::list_launch_tasks,
            commands::launch_task::upsert_launch_task,
            commands::launch_task::reorder_launch_tasks,
            commands::launch_task::delete_launch_task,
            commands::codex_profile::list_codex_profiles,
            commands::codex_profile::upsert_codex_profile,
            commands::oss::list_oss_accounts,
            commands::oss::upsert_oss_account,
            commands::oss::delete_oss_account,
            commands::oss::list_oss_targets,
            commands::oss::upsert_oss_target,
            commands::oss::delete_oss_target,
            commands::oss::list_oss_objects,
            commands::oss::head_oss_object,
            commands::oss::get_oss_object_download_url,
            commands::oss::create_oss_folder,
            commands::oss::test_oss_target,
            commands::oss::delete_oss_objects,
            commands::oss::copy_oss_object,
            commands::oss_transfer::list_oss_transfer_tasks,
            commands::oss_transfer::start_oss_upload,
            commands::oss_transfer::start_oss_download,
            commands::oss_transfer::resume_oss_transfer,
            commands::oss_transfer::cancel_oss_transfer,
            commands::oss_transfer::confirm_oss_transfer,
            commands::snapshot::list_snapshots,
            commands::snapshot::create_snapshot,
            commands::snapshot::update_snapshot,
            commands::screenshot::start_screenshot_session,
            commands::screenshot::get_screenshot_session,
            commands::screenshot::get_screenshot_preview_rgba,
            commands::screenshot::get_screenshot_selection_render,
            commands::screenshot::copy_screenshot_selection,
            commands::screenshot::save_screenshot_selection,
            commands::screenshot::cancel_screenshot_session,
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

fn serve_screenshot_preview<R: tauri::Runtime>(
    context: tauri::UriSchemeContext<'_, R>,
    request: tauri::http::Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    let app_handle = context.app_handle();
    let build_error_response = |status: StatusCode, body: &'static str| {
        Response::builder()
            .status(status)
            .header(CONTENT_TYPE, "text/plain; charset=utf-8")
            .header("Cache-Control", "no-store")
            .body(body.as_bytes().to_vec())
            .expect("build preview protocol error response")
    };
    let build_image_response = |content_type: &'static str, body: Vec<u8>| {
        Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, content_type)
            .header("Cache-Control", "no-store")
            .body(body)
            .expect("build preview protocol success response")
    };

    if context.webview_label() != SCREENSHOT_OVERLAY_WINDOW_LABEL {
        log::warn!(
            target: "bexo::app",
            "preview protocol denied non-overlay webview label={}",
            context.webview_label()
        );
        return build_error_response(StatusCode::FORBIDDEN, "preview protocol is overlay-only");
    }

    let encoded_path = request.uri().path().trim_start_matches('/');
    if encoded_path.is_empty() {
        return build_error_response(StatusCode::BAD_REQUEST, "missing preview path");
    }
    if encoded_path.len() > SCREENSHOT_PREVIEW_MAX_ENCODED_PATH_BYTES {
        return build_error_response(StatusCode::PAYLOAD_TOO_LARGE, "preview path is too large");
    }

    let decoded_path = match URL_SAFE_NO_PAD.decode(encoded_path) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(path) => path,
            Err(error) => {
                log::warn!(
                    target: "bexo::app",
                    "preview protocol rejected invalid utf8 payload reason={}",
                    error
                );
                return build_error_response(StatusCode::BAD_REQUEST, "invalid preview path");
            }
        },
        Err(error) => {
            log::warn!(
                target: "bexo::app",
                "preview protocol rejected invalid base64 payload reason={}",
                error
            );
            return build_error_response(StatusCode::BAD_REQUEST, "invalid preview path");
        }
    };

    if let Some(session_id) = decoded_path.strip_prefix("session:") {
        let screenshot_service = app_handle.state::<crate::services::ScreenshotService>();
        let body = match screenshot_service.get_preview_protocol_bmp(session_id) {
            Ok(bytes) => bytes,
            Err(error) => {
                log::warn!(
                    target: "bexo::app",
                    "preview protocol raw session failed session_id={} reason={}",
                    session_id,
                    error
                );
                return build_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "build raw preview image failed",
                );
            }
        };

        log::info!(
            target: "bexo::app",
            "preview protocol served raw session session_id={} bytes={} content_type=image/bmp webview={}",
            session_id,
            body.len(),
            context.webview_label()
        );

        return build_image_response("image/bmp", body);
    }

    let preview_root = context
        .app_handle()
        .path()
        .temp_dir()
        .map(|path| path.join(SCREENSHOT_PREVIEW_TEMP_DIR_NAME));
    let Ok(preview_root) = preview_root else {
        return build_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "resolve preview temp dir failed",
        );
    };

    let requested_path = PathBuf::from(decoded_path);
    let canonical_root = match fs::canonicalize(&preview_root) {
        Ok(path) => path,
        Err(error) => {
            log::warn!(
                target: "bexo::app",
                "preview protocol root missing path={} reason={}",
                preview_root.display(),
                error
            );
            return build_error_response(StatusCode::NOT_FOUND, "preview root not found");
        }
    };
    let canonical_file = match fs::canonicalize(&requested_path) {
        Ok(path) => path,
        Err(error) => {
            log::warn!(
                target: "bexo::app",
                "preview protocol file missing path={} reason={}",
                requested_path.display(),
                error
            );
            return build_error_response(StatusCode::NOT_FOUND, "preview file not found");
        }
    };

    if !canonical_file.starts_with(&canonical_root) {
        log::warn!(
            target: "bexo::app",
            "preview protocol denied out-of-scope file path={} root={}",
            canonical_file.display(),
            canonical_root.display()
        );
        return build_error_response(StatusCode::FORBIDDEN, "preview file out of scope");
    }

    let body = match fs::read(&canonical_file) {
        Ok(bytes) => bytes,
        Err(error) => {
            log::warn!(
                target: "bexo::app",
                "preview protocol read failed path={} reason={}",
                canonical_file.display(),
                error
            );
            return build_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "read preview file failed",
            );
        }
    };

    let content_type = match canonical_file
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("bmp") => "image/bmp",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        _ => "application/octet-stream",
    };

    log::info!(
        target: "bexo::app",
        "preview protocol served file path={} bytes={} content_type={} webview={}",
        canonical_file.display(),
        body.len(),
        content_type,
        context.webview_label()
    );

    build_image_response(content_type, body)
}

fn focus_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let Some(window) = app.get_webview_window("main") else {
        log::warn!(
            target: "bexo::app",
            "main window not found while attempting to focus existing instance"
        );
        return;
    };

    match window.is_minimized() {
        Ok(true) => {
            if let Err(error) = window.unminimize() {
                log::warn!(
                    target: "bexo::app",
                    "failed to unminimize main window: {error}"
                );
            }
        }
        Ok(false) => {}
        Err(error) => {
            log::warn!(
                target: "bexo::app",
                "failed to query main window minimized state: {error}"
            );
        }
    }

    if let Err(error) = window.show() {
        log::warn!(target: "bexo::app", "failed to show main window: {error}");
    }
    if let Err(error) = window.set_focus() {
        log::warn!(target: "bexo::app", "failed to focus main window: {error}");
    }
}

fn args_indicate_autostart<T: AsRef<OsStr>>(args: &[T]) -> bool {
    args.iter()
        .any(|arg| arg.as_ref().to_string_lossy() == "--autostart")
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::args_indicate_autostart;

    #[test]
    fn args_indicate_autostart_detects_plugin_argument() {
        let args = vec![
            OsString::from("bexo-studio.exe"),
            OsString::from("--autostart"),
        ];

        assert!(args_indicate_autostart(&args));
    }

    #[test]
    fn args_indicate_autostart_ignores_manual_launch() {
        let args = vec![OsString::from("bexo-studio.exe")];

        assert!(!args_indicate_autostart(&args));
    }
}
