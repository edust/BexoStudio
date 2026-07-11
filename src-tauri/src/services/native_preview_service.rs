#![allow(dead_code)]

#[cfg(target_os = "windows")]
use std::{sync::mpsc, thread, time::Duration};
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use tauri::AppHandle;

use crate::error::{AppError, AppResult};
#[cfg(target_os = "windows")]
use crate::services::native_preview_backend_windows;

#[derive(Clone)]
pub struct NativePreviewService {
    state: Arc<Mutex<NativePreviewState>>,
}

#[derive(Debug)]
struct NativePreviewState {
    backend_kind: Option<NativePreviewBackendKind>,
    backend_runtime: Option<NativePreviewBackendRuntime>,
    runtime_mode: NativePreviewRuntimeMode,
    lifecycle_state: NativePreviewLifecycleState,
    initialized_at: Option<Instant>,
    last_error: Option<AppError>,
    active_session: Option<NativePreviewSessionSpec>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePreviewBackendKind {
    WindowsDirectCompositionSkeleton,
}

impl NativePreviewBackendKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::WindowsDirectCompositionSkeleton => "windows_directcomposition_skeleton",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativePreviewRuntimeMode {
    Uninitialized,
    PhaseBScaffold,
}

impl NativePreviewRuntimeMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Uninitialized => "uninitialized",
            Self::PhaseBScaffold => "phase_b_scaffold",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePreviewLifecycleState {
    Uninitialized,
    Ready,
    Prepared,
    Visible,
    Hidden,
    Failed,
}

impl NativePreviewLifecycleState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Uninitialized => "uninitialized",
            Self::Ready => "ready",
            Self::Prepared => "prepared",
            Self::Visible => "visible",
            Self::Hidden => "hidden",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativePreviewSessionSpec {
    pub session_id: String,
    pub display_id: u32,
    pub display_x: i32,
    pub display_y: i32,
    pub display_width: u32,
    pub display_height: u32,
    pub capture_width: u32,
    pub capture_height: u32,
    pub scale_factor: f32,
    pub preview_width: u32,
    pub preview_height: u32,
    pub source_kind: NativePreviewSourceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePreviewSourceKind {
    DesktopDuplicationCache,
    ScreenshotSessionFrame,
}

impl NativePreviewSourceKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::DesktopDuplicationCache => "desktop_duplication_cache",
            Self::ScreenshotSessionFrame => "screenshot_session_frame",
        }
    }
}

#[derive(Debug, Clone)]
pub struct NativePreviewStateView {
    pub backend_kind: Option<NativePreviewBackendKind>,
    pub runtime_mode: &'static str,
    pub lifecycle_state: &'static str,
    pub has_active_session: bool,
}

#[derive(Debug)]
struct NativePreviewBackendBootstrap {
    backend_runtime: NativePreviewBackendRuntime,
    backend_kind: NativePreviewBackendKind,
    runtime_mode: NativePreviewRuntimeMode,
    composition_stack: &'static str,
    device_create_ms: u128,
    factory_resolve_ms: u128,
    window_create_ms: u128,
    swap_chain_create_ms: u128,
    composition_create_ms: u128,
    prime_present_ms: u128,
}

#[derive(Debug, Clone, Copy)]
struct NativePreviewPrepareMetrics {
    resize_ms: u128,
    frame_commit_ms: u128,
    total_ms: u128,
    window_x: i32,
    window_y: i32,
    window_width: u32,
    window_height: u32,
}

#[derive(Debug)]
struct NativePreviewBackendRuntime {
    #[cfg(target_os = "windows")]
    command_sender: mpsc::Sender<NativePreviewOwnerCommand>,
    #[cfg(target_os = "windows")]
    owner_thread: Option<thread::JoinHandle<()>>,
}

#[cfg(target_os = "windows")]
enum NativePreviewOwnerCommand {
    Prepare {
        session: NativePreviewSessionSpec,
        bgra_top_down: Arc<Vec<u8>>,
        reply: mpsc::SyncSender<AppResult<NativePreviewPrepareMetrics>>,
    },
    Show {
        reply: mpsc::SyncSender<AppResult<()>>,
    },
    ShowBelowWindow {
        anchor_hwnd_raw: isize,
        reply: mpsc::SyncSender<AppResult<()>>,
    },
    Hide {
        reply: mpsc::SyncSender<AppResult<()>>,
    },
    SyncZOrderBelowWindow {
        anchor_hwnd_raw: isize,
        reply: mpsc::SyncSender<AppResult<()>>,
    },
    Shutdown,
}

#[cfg(target_os = "windows")]
const NATIVE_PREVIEW_REQUEST_TIMEOUT: Duration = Duration::from_secs(2);
#[cfg(target_os = "windows")]
const NATIVE_PREVIEW_PREPARE_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(target_os = "windows")]
const NATIVE_PREVIEW_STARTUP_TIMEOUT: Duration = Duration::from_secs(8);
#[cfg(target_os = "windows")]
const NATIVE_PREVIEW_MESSAGE_PUMP_INTERVAL: Duration = Duration::from_millis(8);

impl Default for NativePreviewState {
    fn default() -> Self {
        Self {
            backend_kind: None,
            backend_runtime: None,
            runtime_mode: NativePreviewRuntimeMode::Uninitialized,
            lifecycle_state: NativePreviewLifecycleState::Uninitialized,
            initialized_at: None,
            last_error: None,
            active_session: None,
        }
    }
}

impl NativePreviewService {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(NativePreviewState::default())),
        }
    }

    pub fn initialize<R: tauri::Runtime>(&self, _app_handle: &AppHandle<R>) -> AppResult<()> {
        let bootstrap = bootstrap_native_preview_backend()?;
        let mut state = self.state.lock().map_err(|_| {
            AppError::new(
                "NATIVE_PREVIEW_STATE_LOCK_FAILED",
                "读取 native preview 状态失败",
            )
        })?;

        state.backend_kind = Some(bootstrap.backend_kind);
        state.backend_runtime = Some(bootstrap.backend_runtime);
        state.runtime_mode = bootstrap.runtime_mode;
        state.lifecycle_state = NativePreviewLifecycleState::Ready;
        state.initialized_at = Some(Instant::now());
        state.last_error = None;

        log::info!(
            target: "bexo::service::native_preview",
            "native_preview_service_initialized backend={} mode={} composition_stack={} device_create_ms={} factory_resolve_ms={} window_create_ms={} swap_chain_create_ms={} composition_create_ms={} prime_present_ms={}",
            bootstrap.backend_kind.as_str(),
            bootstrap.runtime_mode.as_str(),
            bootstrap.composition_stack,
            bootstrap.device_create_ms,
            bootstrap.factory_resolve_ms,
            bootstrap.window_create_ms,
            bootstrap.swap_chain_create_ms,
            bootstrap.composition_create_ms,
            bootstrap.prime_present_ms
        );

        Ok(())
    }

    pub fn prepare_session(&self, session: NativePreviewSessionSpec) -> AppResult<()> {
        validate_session_spec(&session)?;
        let mut state = self.state.lock().map_err(|_| {
            AppError::new(
                "NATIVE_PREVIEW_STATE_LOCK_FAILED",
                "读取 native preview 状态失败",
            )
        })?;

        if state.lifecycle_state == NativePreviewLifecycleState::Uninitialized {
            return Err(AppError::new(
                "NATIVE_PREVIEW_NOT_INITIALIZED",
                "native preview 服务尚未初始化",
            ));
        }

        state.active_session = Some(session.clone());
        state.lifecycle_state = NativePreviewLifecycleState::Prepared;
        state.last_error = None;

        log::info!(
            target: "bexo::service::native_preview",
            "native_preview_session_prepared session_id={} display_id={} display={}x{} preview={}x{} source_kind={}",
            session.session_id,
            session.display_id,
            session.display_width,
            session.display_height,
            session.preview_width,
            session.preview_height,
            session.source_kind.as_str()
        );

        Ok(())
    }

    pub fn prepare_session_frame(
        &self,
        session: NativePreviewSessionSpec,
        bgra_top_down: Arc<Vec<u8>>,
    ) -> AppResult<()> {
        validate_session_spec(&session)?;
        let mut state = self.state.lock().map_err(|_| {
            AppError::new(
                "NATIVE_PREVIEW_STATE_LOCK_FAILED",
                "读取 native preview 状态失败",
            )
        })?;

        if matches!(
            state.lifecycle_state,
            NativePreviewLifecycleState::Uninitialized | NativePreviewLifecycleState::Failed
        ) {
            return Err(AppError::new(
                "NATIVE_PREVIEW_NOT_INITIALIZED",
                "native preview 服务尚未初始化",
            )
            .with_detail("state", state.lifecycle_state.as_str()));
        }

        let backend_kind = state.backend_kind.ok_or_else(|| {
            AppError::new(
                "NATIVE_PREVIEW_BACKEND_UNAVAILABLE",
                "native preview backend 不可用",
            )
        })?;
        let prepare =
            prepare_native_preview_backend_frame(&mut state, session.clone(), bgra_top_down)?;

        state.active_session = Some(session.clone());
        state.lifecycle_state = NativePreviewLifecycleState::Prepared;
        state.last_error = None;

        log::info!(
            target: "bexo::service::native_preview",
            "native_preview_frame_committed session_id={} backend={} source_kind={} capture={}x{} preview={}x{} window={}x{}@{},{} resize_ms={} frame_commit_ms={} total_ms={}",
            session.session_id,
            backend_kind.as_str(),
            session.source_kind.as_str(),
            session.capture_width,
            session.capture_height,
            session.preview_width,
            session.preview_height,
            prepare.window_width,
            prepare.window_height,
            prepare.window_x,
            prepare.window_y,
            prepare.resize_ms,
            prepare.frame_commit_ms,
            prepare.total_ms
        );

        Ok(())
    }

    pub fn show_prepared_session(&self) -> AppResult<()> {
        let started_at = Instant::now();
        let mut state = self.state.lock().map_err(|_| {
            AppError::new(
                "NATIVE_PREVIEW_STATE_LOCK_FAILED",
                "读取 native preview 状态失败",
            )
        })?;

        if !matches!(
            state.lifecycle_state,
            NativePreviewLifecycleState::Prepared
                | NativePreviewLifecycleState::Visible
                | NativePreviewLifecycleState::Hidden
        ) {
            return Err(AppError::new(
                "NATIVE_PREVIEW_INVALID_STATE",
                "native preview 当前不处于可显示状态",
            )
            .with_detail("state", state.lifecycle_state.as_str()));
        }

        let session_id = state
            .active_session
            .as_ref()
            .map(|session| session.session_id.clone())
            .ok_or_else(|| {
                AppError::new(
                    "NATIVE_PREVIEW_SESSION_NOT_PREPARED",
                    "native preview 未准备可显示会话",
                )
            })?;

        show_native_preview_backend(&mut state)?;
        state.lifecycle_state = NativePreviewLifecycleState::Visible;
        log::info!(
            target: "bexo::service::native_preview",
            "native_preview_window_shown session_id={} backend={} total_ms={}",
            session_id,
            state
                .backend_kind
                .map(NativePreviewBackendKind::as_str)
                .unwrap_or("unknown"),
            started_at.elapsed().as_millis()
        );

        Ok(())
    }

    pub fn show_prepared_session_below_window(&self, anchor_hwnd_raw: isize) -> AppResult<()> {
        let started_at = Instant::now();
        let mut state = self.state.lock().map_err(|_| {
            AppError::new(
                "NATIVE_PREVIEW_STATE_LOCK_FAILED",
                "读取 native preview 状态失败",
            )
        })?;

        if !matches!(
            state.lifecycle_state,
            NativePreviewLifecycleState::Prepared
                | NativePreviewLifecycleState::Visible
                | NativePreviewLifecycleState::Hidden
        ) {
            return Err(AppError::new(
                "NATIVE_PREVIEW_INVALID_STATE",
                "native preview 当前不处于可显示状态",
            )
            .with_detail("state", state.lifecycle_state.as_str()));
        }

        let session_id = state
            .active_session
            .as_ref()
            .map(|session| session.session_id.clone())
            .ok_or_else(|| {
                AppError::new(
                    "NATIVE_PREVIEW_SESSION_NOT_PREPARED",
                    "native preview 未准备可显示会话",
                )
            })?;

        show_native_preview_backend_below_window(&mut state, anchor_hwnd_raw)?;
        state.lifecycle_state = NativePreviewLifecycleState::Visible;
        log::info!(
            target: "bexo::service::native_preview",
            "native_preview_window_shown session_id={} backend={} anchor=overlay total_ms={}",
            session_id,
            state
                .backend_kind
                .map(NativePreviewBackendKind::as_str)
                .unwrap_or("unknown"),
            started_at.elapsed().as_millis()
        );

        Ok(())
    }

    pub fn hide(&self) -> AppResult<()> {
        let started_at = Instant::now();
        let mut state = self.state.lock().map_err(|_| {
            AppError::new(
                "NATIVE_PREVIEW_STATE_LOCK_FAILED",
                "读取 native preview 状态失败",
            )
        })?;

        if matches!(
            state.lifecycle_state,
            NativePreviewLifecycleState::Uninitialized | NativePreviewLifecycleState::Failed
        ) {
            return Ok(());
        }

        hide_native_preview_backend(&mut state)?;
        state.lifecycle_state = NativePreviewLifecycleState::Hidden;
        log::info!(
            target: "bexo::service::native_preview",
            "native_preview_window_hidden has_active_session={} total_ms={}",
            state.active_session.is_some(),
            started_at.elapsed().as_millis()
        );

        Ok(())
    }

    pub fn sync_z_order_below_window(&self, anchor_hwnd_raw: isize) -> AppResult<()> {
        let started_at = Instant::now();
        let mut state = self.state.lock().map_err(|_| {
            AppError::new(
                "NATIVE_PREVIEW_STATE_LOCK_FAILED",
                "读取 native preview 状态失败",
            )
        })?;

        if !matches!(state.lifecycle_state, NativePreviewLifecycleState::Visible) {
            return Ok(());
        }

        let session_id = state
            .active_session
            .as_ref()
            .map(|session| session.session_id.clone())
            .unwrap_or_else(|| "unknown".to_string());
        sync_native_preview_backend_below_window(&mut state, anchor_hwnd_raw)?;
        log::info!(
            target: "bexo::service::native_preview",
            "native_preview_z_order_synced session_id={} anchor=overlay total_ms={}",
            session_id,
            started_at.elapsed().as_millis()
        );

        Ok(())
    }

    pub fn clear(&self) -> AppResult<()> {
        let mut state = self.state.lock().map_err(|_| {
            AppError::new(
                "NATIVE_PREVIEW_STATE_LOCK_FAILED",
                "读取 native preview 状态失败",
            )
        })?;

        if state.lifecycle_state == NativePreviewLifecycleState::Uninitialized {
            return Ok(());
        }

        state.active_session = None;
        let _backend_is_ready = state.backend_runtime.is_some();
        if state.backend_kind.is_some() {
            state.lifecycle_state = NativePreviewLifecycleState::Ready;
        }
        log::info!(
            target: "bexo::service::native_preview",
            "native_preview_service_cleared backend={}",
            state
                .backend_kind
                .map(NativePreviewBackendKind::as_str)
                .unwrap_or("unknown")
        );

        Ok(())
    }

    pub fn snapshot_state(&self) -> AppResult<NativePreviewStateView> {
        let state = self.state.lock().map_err(|_| {
            AppError::new(
                "NATIVE_PREVIEW_STATE_LOCK_FAILED",
                "读取 native preview 状态失败",
            )
        })?;

        Ok(NativePreviewStateView {
            backend_kind: state.backend_kind,
            runtime_mode: state.runtime_mode.as_str(),
            lifecycle_state: state.lifecycle_state.as_str(),
            has_active_session: state.active_session.is_some(),
        })
    }

    pub fn mark_initialization_failed(&self, error: AppError) {
        if let Ok(mut state) = self.state.lock() {
            state.lifecycle_state = NativePreviewLifecycleState::Failed;
            state.last_error = Some(error);
        }
    }
}

fn validate_session_spec(session: &NativePreviewSessionSpec) -> AppResult<()> {
    if session.session_id.trim().is_empty() {
        return Err(AppError::validation("native preview sessionId 不能为空"));
    }
    if session.display_width == 0 || session.display_height == 0 {
        return Err(AppError::validation("native preview 显示区域尺寸无效"));
    }
    if session.capture_width == 0 || session.capture_height == 0 {
        return Err(AppError::validation("native preview 捕获尺寸无效"));
    }
    if session.preview_width == 0 || session.preview_height == 0 {
        return Err(AppError::validation("native preview 预览尺寸无效"));
    }
    if !(session.scale_factor.is_finite() && session.scale_factor > 0.0) {
        return Err(AppError::validation("native preview 缩放因子无效"));
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn prepare_native_preview_backend_frame(
    state: &NativePreviewState,
    session: NativePreviewSessionSpec,
    bgra_top_down: Arc<Vec<u8>>,
) -> AppResult<NativePreviewPrepareMetrics> {
    let runtime = state.backend_runtime.as_ref().ok_or_else(|| {
        AppError::new(
            "NATIVE_PREVIEW_BACKEND_UNAVAILABLE",
            "native preview backend 不可用",
        )
    })?;
    runtime.prepare_session(session, bgra_top_down)
}

#[cfg(not(target_os = "windows"))]
fn prepare_native_preview_backend_frame(
    _state: &NativePreviewState,
    _session: NativePreviewSessionSpec,
    _bgra_top_down: Arc<Vec<u8>>,
) -> AppResult<NativePreviewPrepareMetrics> {
    Err(AppError::new(
        "NATIVE_PREVIEW_UNSUPPORTED",
        "native preview 仅支持 Windows",
    ))
}

#[cfg(target_os = "windows")]
fn show_native_preview_backend(state: &NativePreviewState) -> AppResult<()> {
    let runtime = state.backend_runtime.as_ref().ok_or_else(|| {
        AppError::new(
            "NATIVE_PREVIEW_BACKEND_UNAVAILABLE",
            "native preview backend 不可用",
        )
    })?;
    runtime.show()
}

#[cfg(target_os = "windows")]
fn show_native_preview_backend_below_window(
    state: &NativePreviewState,
    anchor_hwnd_raw: isize,
) -> AppResult<()> {
    let runtime = state.backend_runtime.as_ref().ok_or_else(|| {
        AppError::new(
            "NATIVE_PREVIEW_BACKEND_UNAVAILABLE",
            "native preview backend 不可用",
        )
    })?;
    runtime.show_below_window(anchor_hwnd_raw)
}

#[cfg(not(target_os = "windows"))]
fn show_native_preview_backend_below_window(
    _state: &NativePreviewState,
    _anchor_hwnd_raw: isize,
) -> AppResult<()> {
    Err(AppError::new(
        "NATIVE_PREVIEW_UNSUPPORTED",
        "native preview 仅支持 Windows",
    ))
}

#[cfg(not(target_os = "windows"))]
fn show_native_preview_backend(_state: &NativePreviewState) -> AppResult<()> {
    Err(AppError::new(
        "NATIVE_PREVIEW_UNSUPPORTED",
        "native preview 仅支持 Windows",
    ))
}

#[cfg(target_os = "windows")]
fn hide_native_preview_backend(state: &NativePreviewState) -> AppResult<()> {
    let runtime = state.backend_runtime.as_ref().ok_or_else(|| {
        AppError::new(
            "NATIVE_PREVIEW_BACKEND_UNAVAILABLE",
            "native preview backend 不可用",
        )
    })?;
    runtime.hide()
}

#[cfg(target_os = "windows")]
fn sync_native_preview_backend_below_window(
    state: &NativePreviewState,
    anchor_hwnd_raw: isize,
) -> AppResult<()> {
    let runtime = state.backend_runtime.as_ref().ok_or_else(|| {
        AppError::new(
            "NATIVE_PREVIEW_BACKEND_UNAVAILABLE",
            "native preview backend 不可用",
        )
    })?;
    runtime.sync_z_order_below_window(anchor_hwnd_raw)
}

#[cfg(not(target_os = "windows"))]
fn sync_native_preview_backend_below_window(
    _state: &NativePreviewState,
    _anchor_hwnd_raw: isize,
) -> AppResult<()> {
    Err(AppError::new(
        "NATIVE_PREVIEW_UNSUPPORTED",
        "native preview 仅支持 Windows",
    ))
}

#[cfg(not(target_os = "windows"))]
fn hide_native_preview_backend(_state: &NativePreviewState) -> AppResult<()> {
    Err(AppError::new(
        "NATIVE_PREVIEW_UNSUPPORTED",
        "native preview 仅支持 Windows",
    ))
}

#[cfg(target_os = "windows")]
fn bootstrap_native_preview_backend() -> AppResult<NativePreviewBackendBootstrap> {
    let (command_sender, command_receiver) = mpsc::channel();
    let (startup_sender, startup_receiver) = mpsc::sync_channel(1);
    let owner_thread = thread::Builder::new()
        .name("bexo-native-preview-owner".to_string())
        .spawn(move || run_native_preview_owner(command_receiver, startup_sender))
        .map_err(|error| {
            AppError::new(
                "NATIVE_PREVIEW_OWNER_START_FAILED",
                "启动 native preview owner thread 失败",
            )
            .with_detail("reason", error.to_string())
        })?;

    let started = match startup_receiver.recv_timeout(NATIVE_PREVIEW_STARTUP_TIMEOUT) {
        Ok(Ok(started)) => started,
        Ok(Err(error)) => {
            if owner_thread.join().is_err() {
                log::warn!(
                    target: "bexo::service::native_preview",
                    "native_preview_owner_panicked_after_startup_failure"
                );
            }
            return Err(error);
        }
        Err(error) => {
            let _ = command_sender.send(NativePreviewOwnerCommand::Shutdown);
            return Err(AppError::new(
                "NATIVE_PREVIEW_OWNER_START_TIMEOUT",
                "native preview owner thread 启动超时",
            )
            .with_detail(
                "timeoutMs",
                NATIVE_PREVIEW_STARTUP_TIMEOUT.as_millis().to_string(),
            )
            .with_detail("reason", error.to_string())
            .retryable(false));
        }
    };

    Ok(NativePreviewBackendBootstrap {
        backend_runtime: NativePreviewBackendRuntime {
            command_sender,
            owner_thread: Some(owner_thread),
        },
        backend_kind: NativePreviewBackendKind::WindowsDirectCompositionSkeleton,
        runtime_mode: NativePreviewRuntimeMode::PhaseBScaffold,
        composition_stack: "win32+d3d11+dxgi+swapchain+dcomp",
        device_create_ms: started.device_create_ms,
        factory_resolve_ms: started.factory_resolve_ms,
        window_create_ms: started.window_create_ms,
        swap_chain_create_ms: started.swap_chain_create_ms,
        composition_create_ms: started.composition_create_ms,
        prime_present_ms: started.prime_present_ms,
    })
}

#[cfg(not(target_os = "windows"))]
fn bootstrap_native_preview_backend() -> AppResult<NativePreviewBackendBootstrap> {
    Err(AppError::new(
        "NATIVE_PREVIEW_UNSUPPORTED",
        "native preview 仅支持 Windows",
    ))
}

#[cfg(target_os = "windows")]
impl NativePreviewBackendRuntime {
    fn prepare_session(
        &self,
        session: NativePreviewSessionSpec,
        bgra_top_down: Arc<Vec<u8>>,
    ) -> AppResult<NativePreviewPrepareMetrics> {
        self.request("prepare_session", NATIVE_PREVIEW_PREPARE_TIMEOUT, |reply| {
            NativePreviewOwnerCommand::Prepare {
                session,
                bgra_top_down,
                reply,
            }
        })
    }

    fn show(&self) -> AppResult<()> {
        self.request("show", NATIVE_PREVIEW_REQUEST_TIMEOUT, |reply| {
            NativePreviewOwnerCommand::Show { reply }
        })
    }

    fn show_below_window(&self, anchor_hwnd_raw: isize) -> AppResult<()> {
        self.request(
            "show_below_window",
            NATIVE_PREVIEW_REQUEST_TIMEOUT,
            |reply| NativePreviewOwnerCommand::ShowBelowWindow {
                anchor_hwnd_raw,
                reply,
            },
        )
    }

    fn hide(&self) -> AppResult<()> {
        self.request("hide", NATIVE_PREVIEW_REQUEST_TIMEOUT, |reply| {
            NativePreviewOwnerCommand::Hide { reply }
        })
    }

    fn sync_z_order_below_window(&self, anchor_hwnd_raw: isize) -> AppResult<()> {
        self.request(
            "sync_z_order_below_window",
            NATIVE_PREVIEW_REQUEST_TIMEOUT,
            |reply| NativePreviewOwnerCommand::SyncZOrderBelowWindow {
                anchor_hwnd_raw,
                reply,
            },
        )
    }

    fn request<T>(
        &self,
        operation: &'static str,
        timeout: Duration,
        build_command: impl FnOnce(mpsc::SyncSender<AppResult<T>>) -> NativePreviewOwnerCommand,
    ) -> AppResult<T>
    where
        T: Send + 'static,
    {
        let (reply_sender, reply_receiver) = mpsc::sync_channel(1);
        self.command_sender
            .send(build_command(reply_sender))
            .map_err(|error| {
                AppError::new(
                    "NATIVE_PREVIEW_OWNER_UNAVAILABLE",
                    "native preview owner thread 不可用",
                )
                .with_detail("operation", operation)
                .with_detail("reason", error.to_string())
            })?;

        reply_receiver.recv_timeout(timeout).map_err(|error| {
            let code = match error {
                mpsc::RecvTimeoutError::Timeout => "NATIVE_PREVIEW_OWNER_TIMEOUT",
                mpsc::RecvTimeoutError::Disconnected => "NATIVE_PREVIEW_OWNER_UNAVAILABLE",
            };
            AppError::new(code, "native preview owner request 失败")
                .with_detail("operation", operation)
                .with_detail("timeoutMs", timeout.as_millis().to_string())
                .with_detail("reason", error.to_string())
                .retryable(false)
        })?
    }
}

#[cfg(target_os = "windows")]
impl Drop for NativePreviewBackendRuntime {
    fn drop(&mut self) {
        let _ = self
            .command_sender
            .send(NativePreviewOwnerCommand::Shutdown);
        if let Some(owner_thread) = self.owner_thread.take() {
            if owner_thread.join().is_err() {
                log::warn!(
                    target: "bexo::service::native_preview",
                    "native_preview_owner_panicked_during_shutdown"
                );
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn run_native_preview_owner(
    command_receiver: mpsc::Receiver<NativePreviewOwnerCommand>,
    startup_sender: mpsc::SyncSender<
        AppResult<native_preview_backend_windows::NativePreviewWindowsBackendStarted>,
    >,
) {
    let (mut backend, started) = match native_preview_backend_windows::initialize() {
        Ok(result) => result,
        Err(error) => {
            let _ = startup_sender.send(Err(error));
            return;
        }
    };

    if startup_sender.send(Ok(started)).is_err() {
        return;
    }

    loop {
        if !native_preview_backend_windows::pump_messages() {
            break;
        }
        let command = match command_receiver.recv_timeout(NATIVE_PREVIEW_MESSAGE_PUMP_INTERVAL) {
            Ok(command) => command,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        match command {
            NativePreviewOwnerCommand::Prepare {
                session,
                bgra_top_down,
                reply,
            } => {
                let result = backend
                    .prepare_session(&session, bgra_top_down.as_slice())
                    .map(|prepare| NativePreviewPrepareMetrics {
                        resize_ms: prepare.resize_ms,
                        frame_commit_ms: prepare.frame_commit_ms,
                        total_ms: prepare.total_ms,
                        window_x: prepare.window_x,
                        window_y: prepare.window_y,
                        window_width: prepare.window_width,
                        window_height: prepare.window_height,
                    });
                let _ = reply.send(result);
            }
            NativePreviewOwnerCommand::Show { reply } => {
                let _ = reply.send(backend.show());
            }
            NativePreviewOwnerCommand::ShowBelowWindow {
                anchor_hwnd_raw,
                reply,
            } => {
                let _ = reply.send(backend.show_below_window(anchor_hwnd_raw));
            }
            NativePreviewOwnerCommand::Hide { reply } => {
                let _ = reply.send(backend.hide());
            }
            NativePreviewOwnerCommand::SyncZOrderBelowWindow {
                anchor_hwnd_raw,
                reply,
            } => {
                let _ = reply.send(backend.sync_z_order_below_window(anchor_hwnd_raw));
            }
            NativePreviewOwnerCommand::Shutdown => break,
        }
    }
}
