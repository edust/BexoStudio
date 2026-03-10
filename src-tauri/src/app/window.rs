use tauri::{Manager, PhysicalPosition, Position, Runtime, WebviewWindow, Window, WindowEvent};

use crate::services::PreferencesService;

pub fn handle_window_event<R: Runtime>(window: &Window<R>, event: &WindowEvent) {
    if window.label() != "main" {
        return;
    }

    if let WindowEvent::CloseRequested { api, .. } = event {
        let close_to_tray = window
            .state::<PreferencesService>()
            .get_preferences()
            .map(|preferences| preferences.tray.close_to_tray)
            .unwrap_or(true);

        if !close_to_tray {
            return;
        }

        api.prevent_close();
        if let Err(error) = window.hide() {
            log::error!(
                target: "bexo::window",
                "failed to hide main window on close request: {error}"
            );
        } else {
            log::info!(target: "bexo::window", "main window hidden to tray");
        }
    }
}

pub fn center_main_window_in_work_area<R: Runtime>(window: &WebviewWindow<R>) {
    if window.label() != "main" {
        return;
    }

    let is_maximized = match window.is_maximized() {
        Ok(value) => value,
        Err(error) => {
            log::warn!(
                target: "bexo::window",
                "failed to read main window maximize state before centering: {error}"
            );
            false
        }
    };
    if is_maximized {
        return;
    }

    let is_fullscreen = match window.is_fullscreen() {
        Ok(value) => value,
        Err(error) => {
            log::warn!(
                target: "bexo::window",
                "failed to read main window fullscreen state before centering: {error}"
            );
            false
        }
    };
    if is_fullscreen {
        return;
    }

    let monitor = match window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten())
    {
        Some(monitor) => monitor,
        None => {
            log::warn!(
                target: "bexo::window",
                "failed to resolve monitor work area for main window centering"
            );
            return;
        }
    };

    let outer_size = match window.outer_size() {
        Ok(size) => size,
        Err(error) => {
            log::warn!(
                target: "bexo::window",
                "failed to read main window outer size before centering: {error}"
            );
            return;
        }
    };

    let work_area = monitor.work_area();
    let work_width = i64::from(work_area.size.width);
    let work_height = i64::from(work_area.size.height);
    let window_width = i64::from(outer_size.width);
    let window_height = i64::from(outer_size.height);
    let centered_x = i64::from(work_area.position.x) + ((work_width - window_width).max(0) / 2_i64);
    let centered_y =
        i64::from(work_area.position.y) + ((work_height - window_height).max(0) / 2_i64);
    let target_position =
        Position::Physical(PhysicalPosition::new(centered_x as i32, centered_y as i32));

    if let Err(error) = window.set_position(target_position) {
        log::warn!(
            target: "bexo::window",
            "failed to center main window within monitor work area: {error}"
        );
    }
}
