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
    TerminalAdapter, TerminalLaunchInput, WindowsTerminalAdapter, WindowsTerminalTabLaunchInput,
};
