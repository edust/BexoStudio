use tauri::{
    menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Manager, Runtime,
};

use crate::{
    domain::RecentRestoreTarget,
    error::{AppError, AppResult},
    services::{PreferencesService, RestoreService},
};

const TRAY_ID: &str = "main-tray";
const RECENT_ITEM_PREFIX: &str = "recent-restore:";

pub fn create_tray(app: &App) -> AppResult<()> {
    let menu = tauri::async_runtime::block_on(build_tray_menu_from_state(&app.handle()))?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Bexo Studio")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let menu_id = event.id.as_ref().to_string();
            if let Some(snapshot_id) = menu_id.strip_prefix(RECENT_ITEM_PREFIX) {
                let app_handle = app.clone();
                let snapshot_id = snapshot_id.to_string();
                tauri::async_runtime::spawn(async move {
                    let restore_service = app_handle.state::<RestoreService>();
                    let preferences_service = app_handle.state::<PreferencesService>();
                    match restore_service
                        .restore_recent_target(snapshot_id.clone(), None, &preferences_service)
                        .await
                    {
                        Ok(_) => {
                            if let Err(error) = refresh_tray_menu(&app_handle).await {
                                log::error!(target: "bexo::tray", "failed to refresh tray after recent restore: {error}");
                            }
                            if let Err(error) = show_main_window(&app_handle) {
                                log::error!(target: "bexo::tray", "failed to focus main window after recent restore: {error}");
                            }
                        }
                        Err(error) => {
                            log::error!(
                                target: "bexo::tray",
                                "restore_recent_target failed for snapshot {}: {}",
                                snapshot_id,
                                error
                            );
                        }
                    }
                });
                return;
            }

            match menu_id.as_str() {
                "show" => {
                    if let Err(error) = show_main_window(app) {
                        log::error!(target: "bexo::tray", "failed to show main window: {error}");
                    }
                }
                "hide" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.hide();
                    }
                }
                "quit" => app.exit(0),
                _ => log::warn!(target: "bexo::tray", "unhandled tray menu event: {:?}", event.id),
            }
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                let app_handle = tray.app_handle();
                if let Err(error) = show_main_window(&app_handle) {
                    log::error!(target: "bexo::tray", "failed to focus main window: {error}");
                }
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder
        .build(app)
        .map_err(|error| AppError::tray_setup(error.to_string()))?;

    Ok(())
}

pub async fn refresh_tray_menu<R: Runtime>(app_handle: &AppHandle<R>) -> AppResult<()> {
    let menu = build_tray_menu_from_state(app_handle).await?;
    let tray = app_handle
        .tray_by_id(TRAY_ID)
        .ok_or_else(|| AppError::tray_setup("main tray icon was not found"))?;
    tray.set_menu(Some(menu))
        .map_err(|error| AppError::tray_setup(error.to_string()))?;
    Ok(())
}

async fn build_tray_menu_from_state<R: Runtime>(app_handle: &AppHandle<R>) -> AppResult<Menu<R>> {
    let preferences_service = app_handle.state::<PreferencesService>();
    let restore_service = app_handle.state::<RestoreService>();
    let preferences = preferences_service.get_preferences()?;
    let recent_targets = if preferences.tray.show_recent_workspaces {
        restore_service.list_recent_restore_targets().await?
    } else {
        Vec::new()
    };

    build_tray_menu(
        app_handle,
        &recent_targets,
        preferences.tray.show_recent_workspaces,
    )
}

fn build_tray_menu<R: Runtime>(
    app_handle: &AppHandle<R>,
    recent_targets: &[RecentRestoreTarget],
    show_recent_workspaces: bool,
) -> AppResult<Menu<R>> {
    let show_item = MenuItem::with_id(app_handle, "show", "显示主窗口", true, None::<&str>)
        .map_err(|error| AppError::tray_setup(error.to_string()))?;
    let hide_item = MenuItem::with_id(app_handle, "hide", "隐藏到托盘", true, None::<&str>)
        .map_err(|error| AppError::tray_setup(error.to_string()))?;
    let quit_item = MenuItem::with_id(app_handle, "quit", "退出 Bexo Studio", true, None::<&str>)
        .map_err(|error| AppError::tray_setup(error.to_string()))?;
    let separator_before_quit = PredefinedMenuItem::separator(app_handle)
        .map_err(|error| AppError::tray_setup(error.to_string()))?;

    let recent_submenu = if show_recent_workspaces {
        Some(build_recent_submenu(app_handle, recent_targets)?)
    } else {
        None
    };
    let separator_before_recent = if recent_submenu.is_some() {
        Some(
            PredefinedMenuItem::separator(app_handle)
                .map_err(|error| AppError::tray_setup(error.to_string()))?,
        )
    } else {
        None
    };

    let mut items: Vec<&dyn IsMenuItem<R>> = vec![&show_item, &hide_item];
    if let Some(separator) = &separator_before_recent {
        items.push(separator);
    }
    if let Some(submenu) = &recent_submenu {
        items.push(submenu);
    }
    items.push(&separator_before_quit);
    items.push(&quit_item);

    Menu::with_items(app_handle, &items).map_err(|error| AppError::tray_setup(error.to_string()))
}

fn build_recent_submenu<R: Runtime>(
    app_handle: &AppHandle<R>,
    recent_targets: &[RecentRestoreTarget],
) -> AppResult<Submenu<R>> {
    let mut recent_items = if recent_targets.is_empty() {
        vec![MenuItem::with_id(
            app_handle,
            "recent-empty",
            "暂无可恢复工作区",
            false,
            None::<&str>,
        )
        .map_err(|error| AppError::tray_setup(error.to_string()))?]
    } else {
        recent_targets
            .iter()
            .map(|target| {
                MenuItem::with_id(
                    app_handle,
                    format!("{RECENT_ITEM_PREFIX}{}", target.id),
                    format_recent_target_label(target),
                    true,
                    None::<&str>,
                )
                .map_err(|error| AppError::tray_setup(error.to_string()))
            })
            .collect::<AppResult<Vec<_>>>()?
    };

    let recent_refs: Vec<&dyn IsMenuItem<R>> = recent_items
        .iter()
        .map(|item| item as &dyn IsMenuItem<R>)
        .collect();

    let submenu = Submenu::with_items(app_handle, "最近工作区", true, &recent_refs)
        .map_err(|error| AppError::tray_setup(error.to_string()))?;

    recent_items.clear();
    Ok(submenu)
}

fn format_recent_target_label(target: &RecentRestoreTarget) -> String {
    format!("{} · {}", target.workspace_name, target.snapshot_name)
}

fn show_main_window<R: tauri::Runtime>(app_handle: &AppHandle<R>) -> AppResult<()> {
    let window = app_handle
        .get_webview_window("main")
        .ok_or_else(|| AppError::window_not_found("main"))?;

    let _ = window.unminimize();
    window
        .show()
        .map_err(|error| AppError::window_action(error.to_string()))?;
    window
        .set_focus()
        .map_err(|error| AppError::window_action(error.to_string()))?;

    Ok(())
}
