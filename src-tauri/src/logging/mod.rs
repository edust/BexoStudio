mod restore_log;

pub use restore_log::RestoreLogStore;

#[cfg(any(debug_assertions, test))]
use std::path::Path;
use std::path::PathBuf;

use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};

const LOG_MAX_FILE_SIZE_BYTES: u128 = 1_048_576;
const LOG_FILES_TO_KEEP: usize = 5;
#[cfg(any(debug_assertions, test))]
const DEVELOPMENT_TARGET_DIR_NAME: &str = "target";
#[cfg(any(debug_assertions, test))]
const TAURI_CRATE_DIR_NAME: &str = "src-tauri";
#[cfg(any(debug_assertions, test))]
const DEVELOPMENT_LOG_DIR_NAME: &str = "runtime-logs";

pub fn build_plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let mut builder = tauri_plugin_log::Builder::new()
        .target(Target::new(TargetKind::Stdout))
        .target(Target::new(TargetKind::LogDir {
            file_name: Some("bexo-studio".to_string()),
        }))
        .level(log::LevelFilter::Info)
        .max_file_size(LOG_MAX_FILE_SIZE_BYTES)
        .rotation_strategy(RotationStrategy::KeepSome(LOG_FILES_TO_KEEP))
        .timezone_strategy(TimezoneStrategy::UseLocal);

    if let Some(path) = resolve_development_log_dir() {
        builder = builder.target(Target::new(TargetKind::Folder {
            path,
            file_name: Some("log".to_string()),
        }));
    }

    builder.build()
}

fn resolve_development_log_dir() -> Option<PathBuf> {
    #[cfg(debug_assertions)]
    {
        let executable = std::env::current_exe().ok()?;
        resolve_development_log_dir_from_executable(&executable)
    }

    #[cfg(not(debug_assertions))]
    {
        None
    }
}

#[cfg(any(debug_assertions, test))]
fn resolve_development_log_dir_from_executable(executable: &Path) -> Option<PathBuf> {
    let target_dir = executable
        .ancestors()
        .find(|path| path_file_name_eq(path, DEVELOPMENT_TARGET_DIR_NAME))?;
    let tauri_crate_dir = target_dir.parent()?;
    if !path_file_name_eq(tauri_crate_dir, TAURI_CRATE_DIR_NAME) {
        return None;
    }

    tauri_crate_dir
        .parent()
        .map(|repository_root| repository_root.join(DEVELOPMENT_LOG_DIR_NAME))
}

#[cfg(any(debug_assertions, test))]
fn path_file_name_eq(path: &Path, expected: &str) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(expected))
}

#[cfg(test)]
mod tests {
    use super::resolve_development_log_dir_from_executable;
    use std::path::PathBuf;

    #[test]
    fn development_log_dir_is_derived_from_executable_in_tauri_target() {
        let executable = PathBuf::from("repository")
            .join("src-tauri")
            .join("target")
            .join("debug")
            .join("bexo-studio.exe");

        assert_eq!(
            resolve_development_log_dir_from_executable(&executable),
            Some(PathBuf::from("repository").join("runtime-logs"))
        );
    }

    #[test]
    fn installed_executable_does_not_create_a_working_directory_log_target() {
        let executable = PathBuf::from("install")
            .join("Bexo Studio")
            .join("bexo-studio.exe");

        assert_eq!(
            resolve_development_log_dir_from_executable(&executable),
            None
        );
    }

    #[test]
    fn unrelated_target_directory_is_not_treated_as_the_tauri_build_output() {
        let executable = PathBuf::from("install")
            .join("target")
            .join("release")
            .join("bexo-studio.exe");

        assert_eq!(
            resolve_development_log_dir_from_executable(&executable),
            None
        );
    }
}
