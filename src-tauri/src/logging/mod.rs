mod restore_log;

pub use restore_log::RestoreLogStore;

use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};

pub fn build_plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_log::Builder::new()
        .target(Target::new(TargetKind::Stdout))
        .target(Target::new(TargetKind::LogDir {
            file_name: Some("bexo-studio".to_string()),
        }))
        .level(log::LevelFilter::Info)
        .rotation_strategy(RotationStrategy::KeepOne)
        .timezone_strategy(TimezoneStrategy::UseLocal)
        .build()
}
