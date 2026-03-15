use std::sync::Arc;

use crate::domain::HotkeyAction;

pub type HotkeyTriggeredCallback = Arc<dyn Fn(HotkeyAction, String) + Send + Sync + 'static>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyShortcutKind {
    Basic,
    WindowsHook,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsHookHotkeyBinding {
    pub action: HotkeyAction,
    pub shortcut: String,
}

pub fn requires_windows_hook(shortcut: &str) -> bool {
    let normalized = normalize_token(shortcut);
    if normalized.is_empty() {
        return false;
    }

    normalized.contains("left")
        || normalized.contains("right")
        || normalized.contains("lctrl")
        || normalized.contains("rctrl")
        || normalized.contains("lalt")
        || normalized.contains("ralt")
        || normalized.contains("lwin")
        || normalized.contains("rwin")
        || normalized.contains("lshift")
        || normalized.contains("rshift")
}

pub fn classify_supported_shortcut(shortcut: &str) -> Result<HotkeyShortcutKind, String> {
    if requires_windows_hook(shortcut) {
        #[cfg(windows)]
        {
            windows_impl::validate_windows_hook_shortcut(shortcut)?;
            return Ok(HotkeyShortcutKind::WindowsHook);
        }

        #[cfg(not(windows))]
        {
            return Err("当前平台暂不支持左右侧修饰键热键".to_string());
        }
    }

    shortcut
        .parse::<tauri_plugin_global_shortcut::Shortcut>()
        .map(|_| HotkeyShortcutKind::Basic)
        .map_err(|error| format!("热键格式无效：{error}"))
}

fn normalize_token(input: &str) -> String {
    input
        .trim()
        .to_lowercase()
        .replace(' ', "")
        .replace('_', "")
        .replace('-', "")
}

#[cfg(windows)]
pub use windows_impl::WindowsHookHotkeyManager;

#[cfg(not(windows))]
#[derive(Debug, Default)]
pub struct WindowsHookHotkeyManager;

#[cfg(not(windows))]
impl WindowsHookHotkeyManager {
    pub fn new() -> Self {
        Self
    }

    pub fn apply_bindings(
        &self,
        bindings: &[WindowsHookHotkeyBinding],
        _callback: HotkeyTriggeredCallback,
    ) -> Result<(), String> {
        if bindings.is_empty() {
            return Ok(());
        }

        Err("当前平台暂不支持 Windows hook 热键".to_string())
    }

    pub fn clear_bindings(&self) {}
}

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use std::cell::RefCell;
    use std::sync::{mpsc, Mutex};
    use std::thread::JoinHandle;
    use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        VK_0, VK_1, VK_2, VK_3, VK_4, VK_5, VK_6, VK_7, VK_8, VK_9, VK_BACK, VK_DELETE, VK_ESCAPE,
        VK_F1, VK_F10, VK_F11, VK_F12, VK_F13, VK_F14, VK_F15, VK_F16, VK_F17, VK_F18, VK_F19,
        VK_F2, VK_F20, VK_F21, VK_F22, VK_F23, VK_F24, VK_F3, VK_F4, VK_F5, VK_F6, VK_F7, VK_F8,
        VK_F9, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_RCONTROL, VK_RETURN, VK_RMENU,
        VK_RSHIFT, VK_RWIN, VK_SPACE, VK_TAB,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, PeekMessageW, PostThreadMessageW,
        SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, KBDLLHOOKSTRUCT, LLKHF_INJECTED,
        MSG, PM_NOREMOVE, WH_KEYBOARD_LL, WM_APP, WM_KEYDOWN, WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN,
        WM_SYSKEYUP,
    };

    const LOG_PREFIX: &str = "bexo::service::hotkey_hook";
    const WM_CONFIG_UPDATE: u32 = WM_APP + 0x47;

    #[derive(Clone, Debug)]
    struct Chord {
        groups: Vec<Vec<u8>>,
        trigger_keys: Vec<u8>,
    }

    impl Chord {
        fn contains_vk(&self, vk: u8) -> bool {
            self.groups.iter().any(|group| group.contains(&vk))
        }

        fn should_consume_vk(&self, vk: u8, active_before: bool, active_after: bool) -> bool {
            if !self.trigger_keys.contains(&vk) {
                return false;
            }
            active_before || active_after
        }
    }

    #[derive(Clone, Debug)]
    struct ParsedBinding {
        action: HotkeyAction,
        shortcut: String,
        chord: Chord,
    }

    #[derive(Clone)]
    struct ParsedHookConfig {
        bindings: Vec<ParsedBinding>,
        callback: HotkeyTriggeredCallback,
    }

    struct LocalState {
        config: Option<ParsedHookConfig>,
        pressed: [bool; 256],
        pending_config: Option<std::sync::Arc<Mutex<Option<ParsedHookConfig>>>>,
    }

    impl Default for LocalState {
        fn default() -> Self {
            Self {
                config: None,
                pressed: [false; 256],
                pending_config: None,
            }
        }
    }

    thread_local! {
        static LOCAL: RefCell<LocalState> = RefCell::new(LocalState::default());
    }

    fn empty_callback() -> HotkeyTriggeredCallback {
        Arc::new(|_, _| {})
    }

    fn empty_config() -> ParsedHookConfig {
        ParsedHookConfig {
            bindings: Vec::new(),
            callback: empty_callback(),
        }
    }

    fn chord_active(chord: &Chord, pressed: &[bool; 256]) -> bool {
        for group in &chord.groups {
            if !group.iter().any(|vk| pressed[*vk as usize]) {
                return false;
            }
        }
        true
    }

    fn is_key_event_down(w_param: WPARAM) -> bool {
        w_param as u32 == WM_KEYDOWN || w_param as u32 == WM_SYSKEYDOWN
    }

    fn is_key_event_up(w_param: WPARAM) -> bool {
        w_param as u32 == WM_KEYUP || w_param as u32 == WM_SYSKEYUP
    }

    unsafe extern "system" fn keyboard_proc(
        code: i32,
        w_param: WPARAM,
        l_param: LPARAM,
    ) -> LRESULT {
        if code < 0 {
            return unsafe { CallNextHookEx(0, code, w_param, l_param) };
        }

        let is_down = is_key_event_down(w_param);
        let is_up = is_key_event_up(w_param);
        if !is_down && !is_up {
            return unsafe { CallNextHookEx(0, code, w_param, l_param) };
        }

        let kb = unsafe { &*(l_param as *const KBDLLHOOKSTRUCT) };
        if (kb.flags as u32 & (LLKHF_INJECTED as u32)) != 0 {
            return unsafe { CallNextHookEx(0, code, w_param, l_param) };
        }

        let vk = (kb.vkCode & 0xFF) as u8;
        let mut consume = false;
        let mut triggered: Vec<(HotkeyAction, String)> = Vec::new();
        let mut callback: Option<HotkeyTriggeredCallback> = None;

        LOCAL.with(|cell| {
            let mut state = cell.borrow_mut();
            let Some((bindings, next_callback)) = state
                .config
                .as_ref()
                .map(|config| (config.bindings.clone(), config.callback.clone()))
            else {
                return;
            };

            callback = Some(next_callback);

            let vk_index = vk as usize;
            let was_pressed = state.pressed[vk_index];
            let before_states = bindings
                .iter()
                .map(|binding| chord_active(&binding.chord, &state.pressed))
                .collect::<Vec<_>>();

            if is_down {
                state.pressed[vk_index] = true;
            } else if is_up {
                state.pressed[vk_index] = false;
            }

            for (index, binding) in bindings.iter().enumerate() {
                let active_before = before_states[index];
                let active_after = chord_active(&binding.chord, &state.pressed);
                if active_after && !active_before && is_down && !was_pressed {
                    triggered.push((binding.action, binding.shortcut.clone()));
                }
                consume |= binding.chord.contains_vk(vk)
                    && binding
                        .chord
                        .should_consume_vk(vk, active_before, active_after);
            }
        });

        if let Some(callback) = callback {
            for (action, shortcut) in triggered {
                callback(action, shortcut);
            }
        }

        if consume {
            return 1;
        }

        unsafe { CallNextHookEx(0, code, w_param, l_param) }
    }

    fn is_modifier_vk(vk: u8) -> bool {
        vk == VK_LCONTROL as u8
            || vk == VK_RCONTROL as u8
            || vk == VK_LMENU as u8
            || vk == VK_RMENU as u8
            || vk == VK_LWIN as u8
            || vk == VK_RWIN as u8
            || vk == VK_LSHIFT as u8
            || vk == VK_RSHIFT as u8
    }

    fn pick_trigger_keys(groups: &[Vec<u8>]) -> Vec<u8> {
        if groups.is_empty() {
            return Vec::new();
        }

        let all_modifiers = groups
            .iter()
            .all(|group| group.iter().all(|vk| is_modifier_vk(*vk)));
        if all_modifiers {
            if groups.len() == 1 {
                return groups[0].clone();
            }
            return Vec::new();
        }

        if let Some(group) = groups
            .iter()
            .rev()
            .find(|group| group.iter().any(|vk| !is_modifier_vk(*vk)))
        {
            return group.clone();
        }

        groups.last().cloned().unwrap_or_default()
    }

    fn parse_key_group(token: &str) -> Result<Vec<u8>, String> {
        let normalized = normalize_token(token);
        if normalized.is_empty() {
            return Err("热键包含空片段".to_string());
        }

        let group = match normalized.as_str() {
            "ctrl" | "control" => vec![VK_LCONTROL as u8, VK_RCONTROL as u8],
            "lctrl" | "leftctrl" | "leftcontrol" => vec![VK_LCONTROL as u8],
            "rctrl" | "rightctrl" | "rightcontrol" => vec![VK_RCONTROL as u8],
            "alt" => vec![VK_LMENU as u8, VK_RMENU as u8],
            "lalt" | "leftalt" => vec![VK_LMENU as u8],
            "ralt" | "rightalt" => vec![VK_RMENU as u8],
            "win" | "windows" | "super" => vec![VK_LWIN as u8, VK_RWIN as u8],
            "lwin" | "leftwin" | "leftwindows" | "lsuper" => vec![VK_LWIN as u8],
            "rwin" | "rightwin" | "rightwindows" | "rsuper" => vec![VK_RWIN as u8],
            "shift" => vec![VK_LSHIFT as u8, VK_RSHIFT as u8],
            "lshift" | "leftshift" => vec![VK_LSHIFT as u8],
            "rshift" | "rightshift" => vec![VK_RSHIFT as u8],
            "space" => vec![VK_SPACE as u8],
            "tab" => vec![VK_TAB as u8],
            "enter" | "return" => vec![VK_RETURN as u8],
            "esc" | "escape" => vec![VK_ESCAPE as u8],
            "backspace" => vec![VK_BACK as u8],
            "delete" | "del" => vec![VK_DELETE as u8],
            _ => match normalized.as_str() {
                "0" => vec![VK_0 as u8],
                "1" => vec![VK_1 as u8],
                "2" => vec![VK_2 as u8],
                "3" => vec![VK_3 as u8],
                "4" => vec![VK_4 as u8],
                "5" => vec![VK_5 as u8],
                "6" => vec![VK_6 as u8],
                "7" => vec![VK_7 as u8],
                "8" => vec![VK_8 as u8],
                "9" => vec![VK_9 as u8],
                "f1" => vec![VK_F1 as u8],
                "f2" => vec![VK_F2 as u8],
                "f3" => vec![VK_F3 as u8],
                "f4" => vec![VK_F4 as u8],
                "f5" => vec![VK_F5 as u8],
                "f6" => vec![VK_F6 as u8],
                "f7" => vec![VK_F7 as u8],
                "f8" => vec![VK_F8 as u8],
                "f9" => vec![VK_F9 as u8],
                "f10" => vec![VK_F10 as u8],
                "f11" => vec![VK_F11 as u8],
                "f12" => vec![VK_F12 as u8],
                "f13" => vec![VK_F13 as u8],
                "f14" => vec![VK_F14 as u8],
                "f15" => vec![VK_F15 as u8],
                "f16" => vec![VK_F16 as u8],
                "f17" => vec![VK_F17 as u8],
                "f18" => vec![VK_F18 as u8],
                "f19" => vec![VK_F19 as u8],
                "f20" => vec![VK_F20 as u8],
                "f21" => vec![VK_F21 as u8],
                "f22" => vec![VK_F22 as u8],
                "f23" => vec![VK_F23 as u8],
                "f24" => vec![VK_F24 as u8],
                _ => {
                    if normalized.len() == 1 {
                        let ch = normalized.chars().next().unwrap_or(' ');
                        if ch.is_ascii_alphabetic() {
                            return Ok(vec![ch.to_ascii_uppercase() as u8]);
                        }
                    }
                    return Err(format!("不支持的按键片段：{token}"));
                }
            },
        };

        Ok(group)
    }

    fn parse_chord(input: &str) -> Result<Option<Chord>, String> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }

        let mut groups = Vec::new();
        for part in trimmed.split('+') {
            groups.push(parse_key_group(part)?);
        }

        if groups.is_empty() {
            return Ok(None);
        }

        Ok(Some(Chord {
            trigger_keys: pick_trigger_keys(&groups),
            groups,
        }))
    }

    pub(super) fn validate_windows_hook_shortcut(shortcut: &str) -> Result<(), String> {
        let chord = parse_chord(shortcut)?.ok_or_else(|| "高级热键不能为空".to_string())?;

        let has_alt = chord.groups.iter().any(|group| {
            group
                .iter()
                .any(|vk| *vk == VK_LMENU as u8 || *vk == VK_RMENU as u8)
        });
        let has_win = chord.groups.iter().any(|group| {
            group
                .iter()
                .any(|vk| *vk == VK_LWIN as u8 || *vk == VK_RWIN as u8)
        });
        let has_non_modifier = chord
            .groups
            .iter()
            .any(|group| group.iter().any(|vk| !is_modifier_vk(*vk)));

        if chord.groups.len() > 1 && has_alt && has_non_modifier {
            return Err(
                "Windows 高级热键暂不支持 Alt + 字母/功能键组合（容易触发菜单），请改用 Ctrl/Shift 或仅使用单键 RAlt/LAlt。"
                    .to_string(),
            );
        }

        if chord.groups.len() > 1 && has_win && has_non_modifier {
            return Err(
                "Windows 高级热键暂不支持 Win + 字母/功能键组合（可能触发开始菜单），请改用 Ctrl/Shift 或仅使用修饰键组合。"
                    .to_string(),
            );
        }

        Ok(())
    }

    fn parse_bindings(
        bindings: &[WindowsHookHotkeyBinding],
        callback: HotkeyTriggeredCallback,
    ) -> Result<ParsedHookConfig, String> {
        let mut parsed = Vec::with_capacity(bindings.len());
        for binding in bindings {
            validate_windows_hook_shortcut(binding.shortcut.as_str())?;
            let chord = parse_chord(binding.shortcut.as_str())?
                .ok_or_else(|| format!("高级热键不能为空：{}", binding.shortcut))?;
            parsed.push(ParsedBinding {
                action: binding.action,
                shortcut: binding.shortcut.clone(),
                chord,
            });
        }

        Ok(ParsedHookConfig {
            bindings: parsed,
            callback,
        })
    }

    fn apply_pending_config() {
        LOCAL.with(|cell| {
            let mut state = cell.borrow_mut();
            let Some(pending) = state.pending_config.as_ref() else {
                return;
            };
            let next = pending.lock().expect("pending config lock poisoned").take();
            let Some(next) = next else {
                return;
            };
            state.config = Some(next);
            state.pressed = [false; 256];
            let binding_count = state
                .config
                .as_ref()
                .map(|config| config.bindings.len())
                .unwrap_or(0);
            log::info!(
                target: LOG_PREFIX,
                "windows hook config applied binding_count={}",
                binding_count
            );
        });
    }

    fn run_hook_thread(
        pending: std::sync::Arc<Mutex<Option<ParsedHookConfig>>>,
        ready: mpsc::Sender<u32>,
    ) {
        LOCAL.with(|cell| {
            let mut state = cell.borrow_mut();
            state.config = Some(empty_config());
            state.pending_config = Some(pending);
        });

        let module = unsafe { GetModuleHandleW(std::ptr::null()) };
        unsafe {
            let mut queue_probe: MSG = std::mem::zeroed();
            let _ = PeekMessageW(&mut queue_probe, 0, 0, 0, PM_NOREMOVE);
        }
        let hook = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), module, 0) };
        if hook == 0 {
            let _ = ready.send(0);
            log::error!(target: LOG_PREFIX, "install windows keyboard hook failed");
            return;
        }
        let thread_id = unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() };
        let _ = ready.send(thread_id);
        log::info!(target: LOG_PREFIX, "windows hook installed thread_id={}", thread_id);

        unsafe {
            let mut msg: MSG = std::mem::zeroed();
            loop {
                let ret = GetMessageW(&mut msg, 0, 0, 0);
                if ret == 0 || ret == -1 {
                    break;
                }
                if msg.message == WM_CONFIG_UPDATE {
                    apply_pending_config();
                    continue;
                }
                if msg.message == WM_QUIT {
                    break;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        unsafe {
            UnhookWindowsHookEx(hook);
        }
        log::info!(target: LOG_PREFIX, "windows hook stopped");
    }

    pub struct WindowsHookHotkeyManager {
        thread_id: u32,
        join: Mutex<Option<JoinHandle<()>>>,
        pending: std::sync::Arc<Mutex<Option<ParsedHookConfig>>>,
    }

    impl WindowsHookHotkeyManager {
        pub fn new() -> Self {
            let pending = std::sync::Arc::new(Mutex::new(None));
            let (tx, rx) = mpsc::channel();
            let pending_clone = pending.clone();
            let join = std::thread::spawn(move || run_hook_thread(pending_clone, tx));
            let thread_id = rx.recv().unwrap_or(0);

            Self {
                thread_id,
                join: Mutex::new(Some(join)),
                pending,
            }
        }

        pub fn apply_bindings(
            &self,
            bindings: &[WindowsHookHotkeyBinding],
            callback: HotkeyTriggeredCallback,
        ) -> Result<(), String> {
            if self.thread_id == 0 {
                return Err("Windows hook 热键线程未就绪".to_string());
            }

            let parsed = parse_bindings(bindings, callback)?;
            {
                let mut guard = self.pending.lock().expect("pending config lock poisoned");
                *guard = Some(parsed);
            }

            let posted = unsafe { PostThreadMessageW(self.thread_id, WM_CONFIG_UPDATE, 0, 0) };
            if posted == 0 {
                return Err("Windows hook 配置更新投递失败".to_string());
            }

            Ok(())
        }

        pub fn clear_bindings(&self) {
            if self.thread_id == 0 {
                return;
            }

            {
                let mut guard = self.pending.lock().expect("pending config lock poisoned");
                *guard = Some(empty_config());
            }

            unsafe {
                let _ = PostThreadMessageW(self.thread_id, WM_CONFIG_UPDATE, 0, 0);
            }
        }
    }

    impl Drop for WindowsHookHotkeyManager {
        fn drop(&mut self) {
            if self.thread_id != 0 {
                unsafe {
                    let _ = PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0);
                }
            }

            if let Ok(mut guard) = self.join.lock() {
                if let Some(join) = guard.take() {
                    let _ = join.join();
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::validate_windows_hook_shortcut;

        #[test]
        fn validate_windows_hook_shortcut_accepts_ralt() {
            assert!(validate_windows_hook_shortcut("RAlt").is_ok());
        }

        #[test]
        fn validate_windows_hook_shortcut_rejects_alt_letter_combo() {
            let error = validate_windows_hook_shortcut("RAlt+A").expect_err("should fail");
            assert!(error.contains("Alt + 字母/功能键"));
        }
    }
}
