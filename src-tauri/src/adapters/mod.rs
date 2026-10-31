mod codex;
mod ide;
mod oss;
mod process;
mod prompt_paste;
mod terminal;

pub use codex::{CodexAdapter, CodexLaunchInput, DefaultCodexAdapter};
pub use ide::{IdeAdapter, JetBrainsAdapter, VSCodeAdapter};
#[allow(unused_imports)]
pub use oss::{
    AlibabaOssAdapter, ObjectStorageAdapter, OssCredentialStore, OssMultipartAbortRequest,
    OssMultipartCompletePart, OssMultipartCompleteRequest, OssMultipartInitiateRequest,
    OssMultipartUpload, OssMultipartUploadPartRequest, OssObjectCopyRequest,
    OssObjectDeleteRequest, OssObjectDownload, OssObjectGetRequest, OssObjectHeadRequest,
    OssObjectListRequest, OssObjectPutRequest, OssUploadedPart,
};
pub use process::{
    find_first_executable, resolve_configured_executable, run_launch_command, ActionProcessKey,
    ChildProcessRegistry, LaunchCommand, ProcessLaunchResult, ProcessTrackingContext,
};
pub use prompt_paste::{PromptPasteAdapter, SystemPromptPasteAdapter};
pub use terminal::{
    build_windows_shell_command_line, TerminalAdapter, TerminalLaunchInput, WindowsShellLaunchPlan,
    WindowsTerminalAdapter, WindowsTerminalTabLaunchInput,
};

#[cfg(target_os = "macos")]
mod terminal_macos;
#[cfg(not(target_os = "macos"))]
pub use terminal::WindowsTerminalAdapter as PlatformTerminalAdapter;
#[cfg(target_os = "macos")]
pub use terminal_macos::MacTerminalAdapter as PlatformTerminalAdapter;

pub fn file_manager_executable() -> &'static str {
    if cfg!(target_os = "macos") {
        "/usr/bin/open"
    } else {
        "explorer.exe"
    }
}

pub fn build_platform_shell_command_line(command: &str, args: &[String]) -> String {
    #[cfg(target_os = "macos")]
    {
        if args.is_empty() { return command.trim().to_string(); }
        return std::iter::once(command).chain(args.iter().map(String::as_str))
            .map(|arg| if matches!(arg, "&&" | "||" | "|" | ";" | ">" | ">>" | "<" | "2>" | "2>>" | "&") { arg.to_string() } else { terminal_macos::quote_shell(arg) })
            .collect::<Vec<_>>().join(" ");
    }
    #[cfg(not(target_os = "macos"))]
    build_windows_shell_command_line(command, args)
}
