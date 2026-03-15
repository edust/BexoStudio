mod restore_log;

pub use restore_log::RestoreLogStore;

use std::path::PathBuf;

use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};

pub fn build_plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let fixed_log_dir = resolve_fixed_log_dir();
    tauri_plugin_log::Builder::new()
        .target(Target::new(TargetKind::Stdout))
        .target(Target::new(TargetKind::Folder {
            path: fixed_log_dir,
            file_name: Some("log".to_string()),
        }))
        .target(Target::new(TargetKind::LogDir {
            file_name: Some("bexo-studio".to_string()),
        }))
        .level(log::LevelFilter::Info)
        .rotation_strategy(RotationStrategy::KeepOne)
        .timezone_strategy(TimezoneStrategy::UseLocal)
        .build()
}

fn resolve_fixed_log_dir() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    if cwd
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("src-tauri"))
    {
        return cwd
            .parent()
            .map(|value| value.join("runtime-logs"))
            .unwrap_or_else(|| cwd.join("runtime-logs"));
    }

    cwd.join("runtime-logs")
}
