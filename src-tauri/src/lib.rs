mod adapters;
mod app;
mod commands;
mod domain;
mod error;
mod logging;
mod persistence;
mod services;

pub mod automation_support {
    pub use crate::domain::{
        AppPreferences, CreateSnapshotInput, DiagnosticsPreferences, HotkeyPreferences,
        IdePreferences, RestorePreviewInput, ScreenshotSelectionInput, StartRestoreRunInput,
        StartupPreferences, TerminalPreferences, TrayPreferences, UpsertLaunchTaskInput,
        UpsertProjectInput, UpsertWorkspaceInput, WorkspacePreferences,
    };
    pub use crate::logging::RestoreLogStore;
    pub use crate::persistence::Database;
    pub use crate::services::{
        HotkeyService, PlannerService, PreferencesService, ProfileService, ResourceBrowserService,
        RestoreService, ScreenshotService, WorkspaceService,
    };
}

pub fn run() {
    app::run()
}
