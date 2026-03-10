use std::{
    env, fs,
    path::{Path, PathBuf},
    time::Duration,
};

use chrono::Utc;

use crate::{
    domain::AdapterAvailability,
    error::{AppError, AppResult},
};

use super::process::{find_first_executable, resolve_configured_executable, LaunchCommand};

const VSCODE_CONFIG_CANDIDATES: &[&str] = &[
    "code.cmd",
    "code.exe",
    "code.bat",
    "code-insiders.cmd",
    "code-insiders.exe",
    "code-insiders.bat",
    "codium.cmd",
    "codium.exe",
    "codium.bat",
];

const VSCODE_DISCOVERY_CANDIDATES: &[&str] = &[
    "code.cmd",
    "code.exe",
    "code",
    "code-insiders.cmd",
    "code-insiders.exe",
    "code-insiders",
    "codium.cmd",
    "codium.exe",
    "codium",
];

const JETBRAINS_BINARY_CANDIDATES: &[&str] = &[
    "idea64.exe",
    "idea.exe",
    "goland64.exe",
    "goland.exe",
    "pycharm64.exe",
    "pycharm.exe",
    "webstorm64.exe",
    "webstorm.exe",
    "phpstorm64.exe",
    "phpstorm.exe",
    "clion64.exe",
    "clion.exe",
    "rider64.exe",
    "rider.exe",
    "datagrip64.exe",
    "datagrip.exe",
    "rustrover64.exe",
    "rustrover.exe",
    "studio64.exe",
    "studio.exe",
];

const JETBRAINS_CONFIG_CANDIDATES: &[&str] = &[
    "idea64.exe",
    "idea.exe",
    "idea.cmd",
    "idea.bat",
    "goland64.exe",
    "goland.exe",
    "goland.cmd",
    "goland.bat",
    "pycharm64.exe",
    "pycharm.exe",
    "pycharm.cmd",
    "pycharm.bat",
    "webstorm64.exe",
    "webstorm.exe",
    "webstorm.cmd",
    "webstorm.bat",
    "phpstorm64.exe",
    "phpstorm.exe",
    "phpstorm.cmd",
    "phpstorm.bat",
    "clion64.exe",
    "clion.exe",
    "clion.cmd",
    "clion.bat",
    "rider64.exe",
    "rider.exe",
    "rider.cmd",
    "rider.bat",
    "datagrip64.exe",
    "datagrip.exe",
    "datagrip.cmd",
    "datagrip.bat",
    "rustrover64.exe",
    "rustrover.exe",
    "rustrover.cmd",
    "rustrover.bat",
    "studio64.exe",
    "studio.exe",
    "studio.cmd",
    "studio.bat",
];

const JETBRAINS_DISCOVERY_CANDIDATES: &[&str] = &[
    "idea64.exe",
    "idea.exe",
    "idea.cmd",
    "idea",
    "goland64.exe",
    "goland.exe",
    "goland.cmd",
    "goland",
    "pycharm64.exe",
    "pycharm.exe",
    "pycharm.cmd",
    "pycharm",
    "webstorm64.exe",
    "webstorm.exe",
    "webstorm.cmd",
    "webstorm",
    "phpstorm64.exe",
    "phpstorm.exe",
    "phpstorm.cmd",
    "phpstorm",
    "clion64.exe",
    "clion.exe",
    "clion.cmd",
    "clion",
    "rider64.exe",
    "rider.exe",
    "rider.cmd",
    "rider",
    "datagrip64.exe",
    "datagrip.exe",
    "datagrip.cmd",
    "datagrip",
    "rustrover64.exe",
    "rustrover.exe",
    "rustrover.cmd",
    "rustrover",
    "studio64.exe",
    "studio.exe",
    "studio.cmd",
    "studio",
];

const JETBRAINS_INSTALL_PREFIXES: &[&str] = &[
    "intellij",
    "goland",
    "pycharm",
    "webstorm",
    "phpstorm",
    "clion",
    "rider",
    "datagrip",
    "rustrover",
    "android studio",
];

pub trait IdeAdapter {
    fn detect(&self, configured_path: Option<&str>) -> AdapterAvailability;
    fn build_launch_plan(
        &self,
        executable_path: &str,
        project_path: &str,
    ) -> AppResult<LaunchCommand>;
}

#[derive(Debug, Default, Clone)]
pub struct VSCodeAdapter;

#[derive(Debug, Default, Clone)]
pub struct JetBrainsAdapter;

impl IdeAdapter for VSCodeAdapter {
    fn detect(&self, configured_path: Option<&str>) -> AdapterAvailability {
        let detected_at = Utc::now().to_rfc3339();
        if let Some(raw_path) = configured_path.filter(|value| !value.trim().is_empty()) {
            return match resolve_configured_executable(
                raw_path,
                VSCODE_CONFIG_CANDIDATES,
                "VSCODE_PATH_INVALID",
                "VS Code",
            ) {
                Ok(path) => AdapterAvailability {
                    key: "vscode".to_string(),
                    label: "VS Code".to_string(),
                    available: true,
                    status: "available".to_string(),
                    executable_path: Some(path.display().to_string()),
                    source: "user_config".to_string(),
                    message: format!("已使用用户配置的 VS Code 路径 · {}", detected_at),
                },
                Err(error) => AdapterAvailability {
                    key: "vscode".to_string(),
                    label: "VS Code".to_string(),
                    available: false,
                    status: "invalid".to_string(),
                    executable_path: Some(raw_path.trim().to_string()),
                    source: "user_config".to_string(),
                    message: format!("VS Code 配置路径无效：{} · {}", error.message, detected_at),
                },
            };
        }

        match detect_vscode_executable() {
            Some((path, source)) => AdapterAvailability {
                key: "vscode".to_string(),
                label: "VS Code".to_string(),
                available: true,
                status: "available".to_string(),
                executable_path: Some(path.display().to_string()),
                source: source.to_string(),
                message: if source == "PATH" {
                    format!("已探测到 VS Code CLI · {}", detected_at)
                } else {
                    format!("已在常见安装目录中探测到 VS Code CLI · {}", detected_at)
                },
            },
            None => AdapterAvailability {
                key: "vscode".to_string(),
                label: "VS Code".to_string(),
                available: false,
                status: "missing".to_string(),
                executable_path: None,
                source: "PATH".to_string(),
                message: format!("未在 PATH 中找到 code · {}", detected_at),
            },
        }
    }

    fn build_launch_plan(
        &self,
        executable_path: &str,
        project_path: &str,
    ) -> AppResult<LaunchCommand> {
        build_ide_launch_command(
            executable_path,
            vec!["-n".to_string(), project_path.to_string()],
        )
    }
}

impl IdeAdapter for JetBrainsAdapter {
    fn detect(&self, configured_path: Option<&str>) -> AdapterAvailability {
        let detected_at = Utc::now().to_rfc3339();
        if let Some(raw_path) = configured_path.filter(|value| !value.trim().is_empty()) {
            return match resolve_configured_executable(
                raw_path,
                JETBRAINS_CONFIG_CANDIDATES,
                "JETBRAINS_PATH_INVALID",
                "JetBrains IDE",
            ) {
                Ok(path) => AdapterAvailability {
                    key: "jetbrains".to_string(),
                    label: "JetBrains IDE".to_string(),
                    available: true,
                    status: "available".to_string(),
                    executable_path: Some(path.display().to_string()),
                    source: "user_config".to_string(),
                    message: format!("已使用用户配置的 JetBrains 路径 · {}", detected_at),
                },
                Err(error) => AdapterAvailability {
                    key: "jetbrains".to_string(),
                    label: "JetBrains IDE".to_string(),
                    available: false,
                    status: "invalid".to_string(),
                    executable_path: Some(raw_path.trim().to_string()),
                    source: "user_config".to_string(),
                    message: format!(
                        "JetBrains 配置路径无效：{} · {}",
                        error.message, detected_at
                    ),
                },
            };
        }

        match detect_jetbrains_executable() {
            Some((path, source)) => AdapterAvailability {
                key: "jetbrains".to_string(),
                label: "JetBrains IDE".to_string(),
                available: true,
                status: "available".to_string(),
                executable_path: Some(path.display().to_string()),
                source: source.to_string(),
                message: if source == "PATH" {
                    format!("已探测到 JetBrains CLI · {}", detected_at)
                } else {
                    format!("已在常见安装目录中探测到 JetBrains IDE · {}", detected_at)
                },
            },
            None => AdapterAvailability {
                key: "jetbrains".to_string(),
                label: "JetBrains IDE".to_string(),
                available: false,
                status: "missing".to_string(),
                executable_path: None,
                source: "PATH".to_string(),
                message: format!("未在 PATH 中找到 idea / idea64.exe · {}", detected_at),
            },
        }
    }

    fn build_launch_plan(
        &self,
        executable_path: &str,
        project_path: &str,
    ) -> AppResult<LaunchCommand> {
        build_ide_launch_command(executable_path, vec![project_path.to_string()])
    }
}

fn detect_vscode_executable() -> Option<(PathBuf, &'static str)> {
    if let Some(path) = find_first_executable(VSCODE_DISCOVERY_CANDIDATES) {
        return Some((path, "PATH"));
    }

    find_vscode_in_known_locations().map(|path| (path, "system_scan"))
}

fn detect_jetbrains_executable() -> Option<(PathBuf, &'static str)> {
    if let Some(path) = find_first_executable(JETBRAINS_DISCOVERY_CANDIDATES) {
        return Some((path, "PATH"));
    }

    find_jetbrains_in_known_locations().map(|path| (path, "system_scan"))
}

fn find_vscode_in_known_locations() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(local_app_data) = env_dir("LOCALAPPDATA") {
        candidates.push(
            local_app_data
                .join("Programs")
                .join("Microsoft VS Code")
                .join("bin")
                .join("code.cmd"),
        );
        candidates.push(
            local_app_data
                .join("Programs")
                .join("Microsoft VS Code Insiders")
                .join("bin")
                .join("code-insiders.cmd"),
        );
        candidates.push(
            local_app_data
                .join("Programs")
                .join("VSCodium")
                .join("bin")
                .join("codium.cmd"),
        );
    }

    if let Some(program_files) = env_dir("ProgramFiles") {
        candidates.push(
            program_files
                .join("Microsoft VS Code")
                .join("bin")
                .join("code.cmd"),
        );
        candidates.push(
            program_files
                .join("Microsoft VS Code Insiders")
                .join("bin")
                .join("code-insiders.cmd"),
        );
        candidates.push(
            program_files
                .join("VSCodium")
                .join("bin")
                .join("codium.cmd"),
        );
    }

    if let Some(program_files_x86) = env_dir("ProgramFiles(x86)") {
        candidates.push(
            program_files_x86
                .join("Microsoft VS Code")
                .join("bin")
                .join("code.cmd"),
        );
    }

    candidates.into_iter().find(|path| path.is_file())
}

fn find_jetbrains_in_known_locations() -> Option<PathBuf> {
    if let Some(local_app_data) = env_dir("LOCALAPPDATA") {
        if let Some(path) = find_jetbrains_in_toolbox_apps(
            &local_app_data
                .join("JetBrains")
                .join("Toolbox")
                .join("apps"),
        ) {
            return Some(path);
        }

        if let Some(path) = find_jetbrains_in_prefixed_children(&local_app_data.join("Programs")) {
            return Some(path);
        }

        if let Some(path) =
            find_jetbrains_in_prefixed_children(&local_app_data.join("Programs").join("JetBrains"))
        {
            return Some(path);
        }

        if let Some(path) = find_executable_in_bin_directory(
            &local_app_data.join("Programs").join("Android Studio"),
            JETBRAINS_BINARY_CANDIDATES,
        ) {
            return Some(path);
        }
    }

    if let Some(program_files) = env_dir("ProgramFiles") {
        if let Some(path) = find_jetbrains_in_prefixed_children(&program_files.join("JetBrains")) {
            return Some(path);
        }

        if let Some(path) = find_executable_in_bin_directory(
            &program_files.join("Android").join("Android Studio"),
            JETBRAINS_BINARY_CANDIDATES,
        ) {
            return Some(path);
        }
    }

    if let Some(program_files_x86) = env_dir("ProgramFiles(x86)") {
        if let Some(path) =
            find_jetbrains_in_prefixed_children(&program_files_x86.join("JetBrains"))
        {
            return Some(path);
        }
    }

    None
}

fn find_jetbrains_in_toolbox_apps(apps_root: &Path) -> Option<PathBuf> {
    for product_dir in read_directories(apps_root) {
        for channel_dir in read_directories(&product_dir) {
            for build_dir in read_directories(&channel_dir) {
                if let Some(path) =
                    find_executable_in_bin_directory(&build_dir, JETBRAINS_BINARY_CANDIDATES)
                {
                    return Some(path);
                }
            }
        }
    }

    None
}

fn find_jetbrains_in_prefixed_children(root: &Path) -> Option<PathBuf> {
    for child in read_directories(root) {
        let Some(directory_name) = child.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        let directory_name = directory_name.to_ascii_lowercase();
        if !JETBRAINS_INSTALL_PREFIXES
            .iter()
            .any(|prefix| directory_name.starts_with(prefix) || directory_name.contains(prefix))
        {
            continue;
        }

        if let Some(path) = find_executable_in_bin_directory(&child, JETBRAINS_BINARY_CANDIDATES) {
            return Some(path);
        }

        for nested_child in read_directories(&child) {
            if let Some(path) =
                find_executable_in_bin_directory(&nested_child, JETBRAINS_BINARY_CANDIDATES)
            {
                return Some(path);
            }
        }
    }

    None
}

fn read_directories(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };

    let mut directories = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            entry
                .file_type()
                .ok()
                .filter(|kind| kind.is_dir())
                .map(|_| entry.path())
        })
        .collect::<Vec<_>>();

    directories.sort_by(|left, right| right.cmp(left));
    directories
}

fn find_executable_in_bin_directory(root: &Path, candidates: &[&str]) -> Option<PathBuf> {
    let bin_directory = root.join("bin");
    candidates
        .iter()
        .map(|candidate| bin_directory.join(candidate))
        .find(|candidate| candidate.is_file())
}

fn env_dir(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
}

fn build_ide_launch_command(executable_path: &str, args: Vec<String>) -> AppResult<LaunchCommand> {
    if executable_path.trim().is_empty() {
        return Err(AppError::new(
            "IDE_ADAPTER_UNAVAILABLE",
            "ide executable path is required",
        ));
    }

    Ok(LaunchCommand {
        executable_path: PathBuf::from(executable_path),
        args,
        current_dir: None,
        envs: Vec::new(),
        timeout: Duration::from_millis(900),
        tracking: None,
        retain_after_timeout: false,
    })
}
