mod codex_auth_service;
mod codex_history_service;
mod desktop_duplication_capture;
mod hotkey_service;
#[cfg(target_os = "windows")]
mod native_interaction_backend_windows;
mod native_interaction_service;
#[cfg(target_os = "windows")]
mod native_preview_backend_windows;
mod native_preview_service;
mod oss_credential_service;
mod oss_service;
mod oss_transfer_service;
mod planner_service;
mod preferences_service;
mod profile_service;
mod prompt_quick_paste_service;
mod prompt_service;
mod prompt_transfer_service;
mod resource_browser_service;
mod restore_service;
mod screenshot_service;
mod wgc_capture;
mod windows_hook_hotkey;
mod workspace_service;
mod workspace_transfer_service;

#[cfg(target_os = "windows")]
// Passing custom WebView2 arguments replaces Wry's defaults, so keep them in this baseline.
pub(crate) const WINDOWS_WEBVIEW2_BROWSER_ARGS: &str =
    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --disable-gpu --disable-gpu-compositing --disable-software-rasterizer";

pub use codex_auth_service::CodexAuthService;
pub use codex_history_service::CodexHistoryService;
pub use hotkey_service::HotkeyService;
pub use native_interaction_service::{
    NativeInteractionBackendKind, NativeInteractionEditableShape, NativeInteractionExclusionRect,
    NativeInteractionMode, NativeInteractionRuntimeUpdateInput, NativeInteractionSelectionRect,
    NativeInteractionService, NativeInteractionStateUpdatedEvent, NativeInteractionStateView,
};
pub use native_preview_service::NativePreviewService;
pub use oss_credential_service::OssCredentialService;
pub use oss_service::OssService;
pub use oss_transfer_service::OssTransferService;
pub use planner_service::PlannerService;
pub use preferences_service::PreferencesService;
pub use profile_service::ProfileService;
pub use prompt_quick_paste_service::PromptQuickPasteService;
pub use prompt_service::PromptService;
pub use prompt_transfer_service::PromptTransferService;
pub use resource_browser_service::ResourceBrowserService;
pub use restore_service::RestoreService;
pub use screenshot_service::ScreenshotService;
pub use workspace_service::WorkspaceService;
pub use workspace_transfer_service::{WorkspaceImportApplyOutcome, WorkspaceTransferService};
