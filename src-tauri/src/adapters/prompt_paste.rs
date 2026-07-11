use std::{
    sync::{Mutex, TryLockError},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use arboard::Clipboard;

use crate::error::{AppError, AppResult};

const OPERATION_LOCK_TIMEOUT: Duration = Duration::from_millis(1_500);
const MODIFIER_RELEASE_TIMEOUT: Duration = Duration::from_millis(1_200);
const MODIFIER_POLL_INTERVAL: Duration = Duration::from_millis(12);
const CLIPBOARD_RETRY_DELAYS_MS: [u64; 5] = [0, 24, 55, 115, 235];
const CLIPBOARD_SETTLE_DELAY: Duration = Duration::from_millis(24);

pub trait PromptPasteAdapter: Send + Sync {
    fn paste_text(&self, content: String) -> AppResult<()>;
}

#[derive(Debug, Default)]
pub struct SystemPromptPasteAdapter {
    operation_lock: Mutex<()>,
}

impl SystemPromptPasteAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl PromptPasteAdapter for SystemPromptPasteAdapter {
    fn paste_text(&self, content: String) -> AppResult<()> {
        ensure_platform_supported()?;
        let _operation_guard = acquire_operation_lock(&self.operation_lock)?;
        wait_for_modifier_release()?;
        write_clipboard_with_retry(content)?;
        thread::sleep(CLIPBOARD_SETTLE_DELAY);
        wait_for_modifier_release()?;
        send_ctrl_v()
    }
}

fn acquire_operation_lock(lock: &Mutex<()>) -> AppResult<std::sync::MutexGuard<'_, ()>> {
    let started_at = Instant::now();
    loop {
        match lock.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(TryLockError::Poisoned(_)) => {
                return Err(AppError::new(
                    "PROMPT_QUICK_PASTE_LOCK_FAILED",
                    "快速粘贴状态异常，请重启 Bexo Studio 后重试",
                ));
            }
            Err(TryLockError::WouldBlock) if started_at.elapsed() >= OPERATION_LOCK_TIMEOUT => {
                return Err(AppError::new(
                    "PROMPT_QUICK_PASTE_BUSY",
                    "上一次快速粘贴仍在处理中，请稍后重试",
                )
                .with_detail("timeoutMs", OPERATION_LOCK_TIMEOUT.as_millis().to_string()));
            }
            Err(TryLockError::WouldBlock) => thread::sleep(Duration::from_millis(12)),
        }
    }
}

fn write_clipboard_with_retry(content: String) -> AppResult<()> {
    let mut last_reason = String::new();
    for (attempt_index, base_delay_ms) in CLIPBOARD_RETRY_DELAYS_MS.iter().enumerate() {
        if *base_delay_ms > 0 {
            thread::sleep(Duration::from_millis(
                base_delay_ms + clipboard_retry_jitter_ms(attempt_index as u64),
            ));
        }

        match Clipboard::new().and_then(|mut clipboard| clipboard.set_text(content.clone())) {
            Ok(()) => return Ok(()),
            Err(error) => {
                last_reason = error.to_string();
                log::warn!(
                    target: "bexo::adapter::prompt_paste",
                    "clipboard write attempt failed attempt={} max_attempts={} reason={}",
                    attempt_index + 1,
                    CLIPBOARD_RETRY_DELAYS_MS.len(),
                    last_reason
                );
            }
        }
    }

    Err(AppError::new(
        "PROMPT_QUICK_PASTE_CLIPBOARD_FAILED",
        "写入剪贴板失败，可能正被其他程序占用",
    )
    .with_detail("attempts", CLIPBOARD_RETRY_DELAYS_MS.len().to_string())
    .with_detail("reason", last_reason)
    .retryable(true))
}

fn clipboard_retry_jitter_ms(attempt: u64) -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.subsec_nanos() as u64)
        .unwrap_or_default();
    (nanos ^ attempt.wrapping_mul(17)) % 13
}

#[cfg(windows)]
fn ensure_platform_supported() -> AppResult<()> {
    Ok(())
}

#[cfg(not(windows))]
fn ensure_platform_supported() -> AppResult<()> {
    Err(AppError::new(
        "PROMPT_QUICK_PASTE_PLATFORM_UNSUPPORTED",
        "当前平台暂不支持全局快速粘贴",
    ))
}

#[cfg(windows)]
fn wait_for_modifier_release() -> AppResult<()> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_CONTROL, VK_MENU, VK_SHIFT,
    };

    let started_at = Instant::now();
    loop {
        let modifiers_released = unsafe {
            [VK_CONTROL, VK_MENU, VK_SHIFT]
                .into_iter()
                .all(|virtual_key| GetAsyncKeyState(virtual_key as i32) as u16 & 0x8000 == 0)
        };
        if modifiers_released {
            return Ok(());
        }
        if started_at.elapsed() >= MODIFIER_RELEASE_TIMEOUT {
            return Err(AppError::new(
                "PROMPT_QUICK_PASTE_MODIFIERS_HELD",
                "请松开快捷键后重试快速粘贴",
            )
            .with_detail(
                "timeoutMs",
                MODIFIER_RELEASE_TIMEOUT.as_millis().to_string(),
            ));
        }
        thread::sleep(MODIFIER_POLL_INTERVAL);
    }
}

#[cfg(not(windows))]
fn wait_for_modifier_release() -> AppResult<()> {
    Ok(())
}

#[cfg(windows)]
fn send_ctrl_v() -> AppResult<()> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_KEYBOARD, KEYEVENTF_KEYUP, VK_CONTROL,
    };

    const VIRTUAL_KEY_V: u16 = 0x56;
    let mut inputs: [INPUT; 4] = unsafe { std::mem::zeroed() };
    unsafe {
        inputs[0].r#type = INPUT_KEYBOARD;
        inputs[0].Anonymous.ki.wVk = VK_CONTROL;
        inputs[1].r#type = INPUT_KEYBOARD;
        inputs[1].Anonymous.ki.wVk = VIRTUAL_KEY_V;
        inputs[2].r#type = INPUT_KEYBOARD;
        inputs[2].Anonymous.ki.wVk = VIRTUAL_KEY_V;
        inputs[2].Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
        inputs[3].r#type = INPUT_KEYBOARD;
        inputs[3].Anonymous.ki.wVk = VK_CONTROL;
        inputs[3].Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;

        let sent = SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        );
        if sent != inputs.len() as u32 {
            let reason = std::io::Error::last_os_error().to_string();
            let cleanup_sent = release_paste_keys();
            return Err(AppError::new(
                "PROMPT_QUICK_PASTE_INPUT_FAILED",
                "无法向当前窗口发送粘贴；若目标程序以管理员身份运行，请用相同权限启动 Bexo Studio",
            )
            .with_detail("sent", sent.to_string())
            .with_detail("expected", inputs.len().to_string())
            .with_detail("cleanupKeyUpsSent", cleanup_sent.to_string())
            .with_detail("reason", reason));
        }
    }
    Ok(())
}

#[cfg(windows)]
unsafe fn release_paste_keys() -> u32 {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_KEYBOARD, KEYEVENTF_KEYUP, VK_CONTROL,
    };

    const VIRTUAL_KEY_V: u16 = 0x56;
    let mut inputs: [INPUT; 2] = std::mem::zeroed();
    inputs[0].r#type = INPUT_KEYBOARD;
    inputs[0].Anonymous.ki.wVk = VIRTUAL_KEY_V;
    inputs[0].Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
    inputs[1].r#type = INPUT_KEYBOARD;
    inputs[1].Anonymous.ki.wVk = VK_CONTROL;
    inputs[1].Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
    SendInput(
        inputs.len() as u32,
        inputs.as_ptr(),
        std::mem::size_of::<INPUT>() as i32,
    )
}

#[cfg(not(windows))]
fn send_ctrl_v() -> AppResult<()> {
    Err(AppError::new(
        "PROMPT_QUICK_PASTE_PLATFORM_UNSUPPORTED",
        "当前平台暂不支持全局快速粘贴",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_jitter_is_bounded() {
        for attempt in 0..100 {
            assert!(clipboard_retry_jitter_ms(attempt) < 13);
        }
    }
}
