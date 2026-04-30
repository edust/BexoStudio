mod codex;
mod ide;
mod process;
mod terminal;

pub use codex::{CodexAdapter, CodexLaunchInput, DefaultCodexAdapter};
pub use ide::{IdeAdapter, JetBrainsAdapter, VSCodeAdapter};
pub use process::{
    find_first_executable, resolve_configured_executable, run_launch_command, ActionProcessKey,
    ChildProcessRegistry, LaunchCommand, ProcessLaunchResult, ProcessTrackingContext,
};
pub use terminal::{
    build_windows_command_shell_run_args, build_windows_command_shell_startup_command,
    build_windows_shell_command_line, TerminalAdapter, TerminalLaunchInput, WindowsTerminalAdapter,
    WindowsTerminalTabLaunchInput,
};
