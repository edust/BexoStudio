use crate::error::{AppError, AppResult};
use std::{ffi::c_void, ptr, thread, time::Instant};
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> u8;
}
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventSourceFlagsState(state: i32) -> u64;
    fn CGEventCreateKeyboardEvent(source: *const c_void, key: u16, down: bool) -> *mut c_void;
    fn CGEventSetFlags(event: *mut c_void, flags: u64);
    fn CGEventPost(tap: u32, event: *mut c_void);
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(item: *const c_void);
}
pub(super) fn ensure_platform_supported() -> AppResult<()> {
    if unsafe { AXIsProcessTrusted() } == 0 {
        return Err(AppError::new(
            "PROMPT_QUICK_PASTE_ACCESSIBILITY_REQUIRED",
            "请在系统设置 → 隐私与安全性 → 辅助功能中允许 Bexo Studio，然后重试快速粘贴",
        ));
    }
    Ok(())
}
pub(super) fn wait_for_modifier_release() -> AppResult<()> {
    let start = Instant::now();
    // Combined session state, Shift / Control / Option / Command; ignore Caps Lock.
    while unsafe { CGEventSourceFlagsState(0) } & ((1 << 17) | (1 << 18) | (1 << 19) | (1 << 20))
        != 0
    {
        if start.elapsed() >= super::MODIFIER_RELEASE_TIMEOUT {
            return Err(AppError::new(
                "PROMPT_QUICK_PASTE_MODIFIER_TIMEOUT",
                "请松开修饰键后重试快速粘贴",
            ));
        }
        thread::sleep(super::MODIFIER_POLL_INTERVAL);
    }
    Ok(())
}
pub(super) fn send_ctrl_v() -> AppResult<()> {
    ensure_platform_supported()?;
    // kVK_ANSI_V = 9. Allocate both events before posting, so failure cannot leave a key pressed.
    let down = unsafe { CGEventCreateKeyboardEvent(ptr::null(), 9, true) };
    let up = unsafe { CGEventCreateKeyboardEvent(ptr::null(), 9, false) };
    if down.is_null() || up.is_null() {
        unsafe {
            if !down.is_null() {
                CFRelease(down);
            }
            if !up.is_null() {
                CFRelease(up);
            }
        }
        return Err(AppError::new(
            "PROMPT_QUICK_PASTE_INPUT_FAILED",
            "创建 macOS 粘贴键盘事件失败",
        ));
    }
    unsafe {
        CGEventSetFlags(down, 1 << 20);
        CGEventSetFlags(up, 1 << 20);
        CGEventPost(0, down);
        CGEventPost(0, up);
        CFRelease(down);
        CFRelease(up);
    }
    Ok(())
}
