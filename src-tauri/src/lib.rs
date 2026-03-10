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
        AppPreferences, CreateSnapshotInput, DiagnosticsPreferences, IdePreferences,
        RestorePreviewInput, StartRestoreRunInput, TerminalPreferences, TrayPreferences,
        UpsertLaunchTaskInput, UpsertProjectInput, UpsertWorkspaceInput, WorkspacePreferences,
    };
    pub use crate::logging::RestoreLogStore;
    pub use crate::persistence::Database;
    pub use crate::services::{
        PlannerService, PreferencesService, ProfileService, ResourceBrowserService,
        RestoreService, WorkspaceService,
    };
}

pub fn run() {
    app::run()
}
