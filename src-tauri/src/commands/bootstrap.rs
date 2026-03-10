use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapState {
    pub app_name: &'static str,
    pub shell_ready: bool,
    pub tray_ready: bool,
    pub theme: &'static str,
    pub sections: Vec<crate::domain::RouteSection>,
}

#[tauri::command]
pub fn get_bootstrap_state() -> BootstrapState {
    BootstrapState {
        app_name: "Bexo Studio",
        shell_ready: true,
        tray_ready: true,
        theme: "dark-cyan",
        sections: crate::domain::primary_sections().into(),
    }
}
