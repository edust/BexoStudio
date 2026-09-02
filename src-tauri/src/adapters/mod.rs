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
