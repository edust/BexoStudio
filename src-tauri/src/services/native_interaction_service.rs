#![allow(dead_code)]

use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::domain::{
    NATIVE_INTERACTION_SHAPE_ANNOTATION_COMMITTED_EVENT_NAME,
    NATIVE_INTERACTION_SHAPE_ANNOTATION_UPDATED_EVENT_NAME,
    NATIVE_INTERACTION_STATE_UPDATED_EVENT_NAME,
};
use crate::error::{AppError, AppResult};
#[cfg(target_os = "windows")]
use crate::services::native_interaction_backend_windows;

#[derive(Clone)]
pub struct NativeInteractionService {
    state: Arc<Mutex<NativeInteractionState>>,
}

struct NativeInteractionState {
    backend_kind: Option<NativeInteractionBackendKind>,
    backend_handle: Option<NativeInteractionBackendHandle>,
    lifecycle_state: NativeInteractionLifecycleState,
    initialized_at: Option<Instant>,
    last_error: Option<AppError>,
    active_session: Option<NativeInteractionSessionSpec>,
    event_sink: Option<NativeInteractionEventSink>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeInteractionBackendKind {
    WindowsLayeredSelectionMvp,
}

impl NativeInteractionBackendKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::WindowsLayeredSelectionMvp => "windows_layered_selection_mvp",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeInteractionLifecycleState {
    Uninitialized,
    Ready,
    Prepared,
    Visible,
    Hidden,
    Failed,
}

impl NativeInteractionLifecycleState {
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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeInteractionSelectionRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeInteractionExclusionRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeInteractionSelectionHandle {
    Nw,
    N,
    Ne,
    E,
    Se,
    S,
    Sw,
    W,
}

impl NativeInteractionSelectionHandle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Nw => "nw",
            Self::N => "n",
            Self::Ne => "ne",
            Self::E => "e",
            Self::Se => "se",
            Self::S => "s",
            Self::Sw => "sw",
            Self::W => "w",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeInteractionHitRegion {
    None,
    SelectionBody,
    Handle(NativeInteractionSelectionHandle),
    ShapeBody,
    ShapeStart,
    ShapeEnd,
    ShapeHandle(NativeInteractionSelectionHandle),
}

impl NativeInteractionHitRegion {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::SelectionBody => "selection_body",
            Self::Handle(handle) => handle.as_str(),
            Self::ShapeBody => "shape_body",
            Self::ShapeStart => "shape_start",
            Self::ShapeEnd => "shape_end",
            Self::ShapeHandle(handle) => match handle {
                NativeInteractionSelectionHandle::Nw => "shape_nw",
                NativeInteractionSelectionHandle::N => "shape_n",
                NativeInteractionSelectionHandle::Ne => "shape_ne",
                NativeInteractionSelectionHandle::E => "shape_e",
                NativeInteractionSelectionHandle::Se => "shape_se",
                NativeInteractionSelectionHandle::S => "shape_s",
                NativeInteractionSelectionHandle::Sw => "shape_sw",
                NativeInteractionSelectionHandle::W => "shape_w",
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeInteractionDragMode {
    Creating,
    Moving,
    Resizing(NativeInteractionSelectionHandle),
    LineCreating,
    RectCreating,
    EllipseCreating,
    ArrowCreating,
    ShapeMoving,
    ShapeStartMoving,
    ShapeEndMoving,
    ShapeResizing(NativeInteractionSelectionHandle),
}

impl NativeInteractionDragMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Creating => "creating",
            Self::Moving => "moving",
            Self::Resizing(_) => "resizing",
            Self::LineCreating => "line_creating",
            Self::RectCreating => "rect_creating",
            Self::EllipseCreating => "ellipse_creating",
            Self::ArrowCreating => "arrow_creating",
            Self::ShapeMoving => "shape_moving",
            Self::ShapeStartMoving => "shape_start_moving",
            Self::ShapeEndMoving => "shape_end_moving",
            Self::ShapeResizing(_) => "shape_resizing",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeInteractionMode {
    Selection,
    LineAnnotation,
    RectAnnotation,
    EllipseAnnotation,
    ArrowAnnotation,
}

impl NativeInteractionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Selection => "selection",
            Self::LineAnnotation => "line_annotation",
            Self::RectAnnotation => "rect_annotation",
            Self::EllipseAnnotation => "ellipse_annotation",
            Self::ArrowAnnotation => "arrow_annotation",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeInteractionShapeAnnotationKind {
    Line,
    Rect,
    Ellipse,
    Arrow,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeInteractionSessionSpec {
    pub session_id: String,
    pub display_x: i32,
    pub display_y: i32,
    pub display_width: u32,
    pub display_height: u32,
    pub scale_factor: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeInteractionStateView {
    pub backend_kind: Option<NativeInteractionBackendKind>,
    pub lifecycle_state: &'static str,
    pub has_active_session: bool,
    pub selection: Option<NativeInteractionSelectionRect>,
    pub active_shape: Option<NativeInteractionEditableShape>,
    pub active_shape_draft: Option<NativeInteractionEditableShape>,
    pub hovered_hit_region: &'static str,
    pub drag_mode: Option<&'static str>,
    pub selection_revision: u64,
    pub active_shape_revision: u64,
    pub interaction_mode: &'static str,
    pub rect_draft: Option<NativeInteractionSelectionRect>,
}

#[derive(Debug, Clone)]
pub struct NativeInteractionRuntimeUpdateInput {
    pub session_id: String,
    pub visible: bool,
    pub exclusion_rects: Vec<NativeInteractionExclusionRect>,
    pub mode: NativeInteractionMode,
    pub selection: Option<NativeInteractionSelectionRect>,
    pub active_shape: Option<NativeInteractionEditableShape>,
    pub shape_candidates: Vec<NativeInteractionEditableShape>,
    pub annotation_color: Option<String>,
    pub annotation_stroke_width: Option<f64>,
}

#[derive(Debug)]
struct NativeInteractionBackendBootstrap {
    backend_handle: NativeInteractionBackendHandle,
    backend_kind: NativeInteractionBackendKind,
    window_create_ms: u128,
    initial_hide_ms: u128,
}

#[derive(Debug, Clone, Copy)]
struct NativeInteractionPrepareMetrics {
    total_ms: u128,
    window_x: i32,
    window_y: i32,
    window_width: u32,
    window_height: u32,
    present_ms: u128,
    copy_ms: u128,
    update_ms: u128,
    surface_recreated: bool,
}

#[derive(Debug, Clone)]
pub struct NativeInteractionSelectionSnapshot {
    pub selection: Option<NativeInteractionSelectionRect>,
    pub active_shape: Option<NativeInteractionEditableShape>,
    pub active_shape_draft: Option<NativeInteractionEditableShape>,
    pub hovered_hit_region: NativeInteractionHitRegion,
    pub drag_mode: Option<NativeInteractionDragMode>,
    pub selection_revision: u64,
    pub active_shape_revision: u64,
    pub interaction_mode: NativeInteractionMode,
    pub rect_draft: Option<NativeInteractionSelectionRect>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeInteractionStateUpdatedEvent {
    pub session_id: Option<String>,
    pub backend_kind: Option<NativeInteractionBackendKind>,
    pub lifecycle_state: String,
    pub has_active_session: bool,
    pub selection: Option<NativeInteractionSelectionRect>,
    pub active_shape: Option<NativeInteractionEditableShape>,
    pub active_shape_draft: Option<NativeInteractionEditableShape>,
    pub hovered_hit_region: String,
    pub drag_mode: Option<String>,
    pub selection_revision: u64,
    pub active_shape_revision: u64,
    pub interaction_mode: String,
    pub rect_draft: Option<NativeInteractionSelectionRect>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeInteractionShapeAnnotationCommittedEvent {
    pub session_id: String,
    pub kind: NativeInteractionShapeAnnotationKind,
    pub color: String,
    pub stroke_width: f64,
    pub start: NativeInteractionSelectionPoint,
    pub end: NativeInteractionSelectionPoint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeInteractionShapeAnnotationUpdatedEvent {
    pub session_id: String,
    pub id: String,
    pub kind: NativeInteractionShapeAnnotationKind,
    pub color: String,
    pub stroke_width: f64,
    pub start: NativeInteractionSelectionPoint,
    pub end: NativeInteractionSelectionPoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeInteractionSelectionPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeInteractionEditableShape {
    pub id: String,
    pub kind: NativeInteractionShapeAnnotationKind,
    pub color: String,
    pub stroke_width: f64,
    pub start: NativeInteractionSelectionPoint,
    pub end: NativeInteractionSelectionPoint,
}

#[derive(Clone)]
pub(crate) enum NativeInteractionBackendEvent {
    StateUpdated(NativeInteractionStateUpdatedEvent),
    ShapeAnnotationCommitted(NativeInteractionShapeAnnotationCommittedEvent),
    ShapeAnnotationUpdated(NativeInteractionShapeAnnotationUpdatedEvent),
    CancelRequested {
        session_id: String,
        shortcut: String,
    },
}

pub(crate) type NativeInteractionEventSink =
    Arc<dyn Fn(NativeInteractionBackendEvent) + Send + Sync>;

#[derive(Debug)]
struct NativeInteractionBackendHandle {
    raw: usize,
    kind: NativeInteractionBackendKind,
}

impl NativeInteractionBackendHandle {
    #[cfg(target_os = "windows")]
    fn from_windows_interaction_window(
        backend: native_interaction_backend_windows::NativeInteractionWindowsBackend,
    ) -> Self {
        Self {
            raw: Box::into_raw(Box::new(backend)) as usize,
            kind: NativeInteractionBackendKind::WindowsLayeredSelectionMvp,
        }
    }

    #[cfg(target_os = "windows")]
    unsafe fn drop_windows_interaction_window(raw: usize) {
        if raw == 0 {
            return;
        }
        drop(unsafe {
            Box::from_raw(
                raw as *mut native_interaction_backend_windows::NativeInteractionWindowsBackend,
            )
        });
    }
}

impl Drop for NativeInteractionBackendHandle {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")]
        if matches!(
            self.kind,
            NativeInteractionBackendKind::WindowsLayeredSelectionMvp
        ) {
            unsafe {
                Self::drop_windows_interaction_window(self.raw);
            }
            self.raw = 0;
        }
    }
}

impl Default for NativeInteractionState {
    fn default() -> Self {
        Self {
            backend_kind: None,
            backend_handle: None,
            lifecycle_state: NativeInteractionLifecycleState::Uninitialized,
            initialized_at: None,
            last_error: None,
            active_session: None,
            event_sink: None,
        }
    }
}

impl NativeInteractionService {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(NativeInteractionState::default())),
        }
    }

    pub fn initialize<R: tauri::Runtime>(&self, _app_handle: &AppHandle<R>) -> AppResult<()> {
        let bootstrap = bootstrap_native_interaction_backend()?;
        let event_sink = build_native_interaction_event_sink(_app_handle);
        let mut state = self.state.lock().map_err(|_| {
            AppError::new(
                "NATIVE_INTERACTION_STATE_LOCK_FAILED",
                "读取 native interaction 状态失败",
            )
        })?;

        state.backend_kind = Some(bootstrap.backend_kind);
        state.backend_handle = Some(bootstrap.backend_handle);
        state.lifecycle_state = NativeInteractionLifecycleState::Ready;
        state.initialized_at = Some(Instant::now());
        state.last_error = None;
        state.event_sink = Some(event_sink.clone());
        set_native_interaction_backend_event_sink(&mut state, event_sink)?;

        log::info!(
            target: "bexo::service::native_interaction",
            "native_interaction_service_initialized backend={} window_create_ms={} initial_hide_ms={}",
            bootstrap.backend_kind.as_str(),
            bootstrap.window_create_ms,
            bootstrap.initial_hide_ms
        );

        Ok(())
    }

    pub fn prepare_session(&self, session: NativeInteractionSessionSpec) -> AppResult<()> {
        validate_session_spec(&session)?;
        let mut state = self.state.lock().map_err(|_| {
            AppError::new(
                "NATIVE_INTERACTION_STATE_LOCK_FAILED",
                "读取 native interaction 状态失败",
            )
        })?;

        if matches!(
            state.lifecycle_state,
            NativeInteractionLifecycleState::Uninitialized
                | NativeInteractionLifecycleState::Failed
        ) {
            return Err(AppError::new(
                "NATIVE_INTERACTION_NOT_INITIALIZED",
                "native interaction 服务尚未初始化",
            ));
        }

        let prepare = prepare_native_interaction_backend(&mut state, &session)?;
        state.active_session = Some(session.clone());
        state.lifecycle_state = NativeInteractionLifecycleState::Prepared;
        state.last_error = None;
        let snapshot = snapshot_native_interaction_backend(&mut state)?;
        let event = build_state_updated_event(&state, &snapshot);

        log::info!(
            target: "bexo::service::native_interaction",
            "native_interaction_session_prepared session_id={} display={}x{}@{},{} scale_factor={} window={}x{}@{},{} present_ms={} copy_ms={} update_ms={} surface_recreated={} total_ms={}",
            session.session_id,
            session.display_width,
            session.display_height,
            session.display_x,
            session.display_y,
            session.scale_factor,
            prepare.window_width,
            prepare.window_height,
            prepare.window_x,
            prepare.window_y,
            prepare.present_ms,
            prepare.copy_ms,
            prepare.update_ms,
            prepare.surface_recreated,
            prepare.total_ms
        );

        log::info!(
            target: "bexo::service::native_interaction",
            "native_interaction_frame_presented session_id={} present_ms={} window={}x{}@{},{}",
            session.session_id,
            prepare.present_ms,
            prepare.window_width,
            prepare.window_height,
            prepare.window_x,
            prepare.window_y
        );

        emit_native_interaction_backend_event(
            &state,
            NativeInteractionBackendEvent::StateUpdated(event),
        );

        Ok(())
    }

    pub fn show_prepared_session(&self) -> AppResult<()> {
        let started_at = Instant::now();
        let mut state = self.state.lock().map_err(|_| {
            AppError::new(
                "NATIVE_INTERACTION_STATE_LOCK_FAILED",
                "读取 native interaction 状态失败",
            )
        })?;

        if !matches!(
            state.lifecycle_state,
            NativeInteractionLifecycleState::Prepared
                | NativeInteractionLifecycleState::Visible
                | NativeInteractionLifecycleState::Hidden
        ) {
            return Err(AppError::new(
                "NATIVE_INTERACTION_INVALID_STATE",
                "native interaction 当前不处于可显示状态",
            )
            .with_detail("state", state.lifecycle_state.as_str()));
        }

        let session_id = state
            .active_session
            .as_ref()
            .map(|session| session.session_id.clone())
            .ok_or_else(|| {
                AppError::new(
                    "NATIVE_INTERACTION_SESSION_NOT_PREPARED",
                    "native interaction 未准备可显示会话",
                )
            })?;

        show_native_interaction_backend(&mut state)?;
        state.lifecycle_state = NativeInteractionLifecycleState::Visible;
        let snapshot = snapshot_native_interaction_backend(&mut state)?;
        let event = build_state_updated_event(&state, &snapshot);
        log::info!(
            target: "bexo::service::native_interaction",
            "native_interaction_window_shown session_id={} backend={} total_ms={}",
            session_id,
            state
                .backend_kind
                .map(NativeInteractionBackendKind::as_str)
                .unwrap_or("unknown"),
            started_at.elapsed().as_millis()
        );
        emit_native_interaction_backend_event(
            &state,
            NativeInteractionBackendEvent::StateUpdated(event),
        );

        Ok(())
    }

    pub fn hide(&self) -> AppResult<()> {
        let started_at = Instant::now();
        let mut state = self.state.lock().map_err(|_| {
            AppError::new(
                "NATIVE_INTERACTION_STATE_LOCK_FAILED",
                "读取 native interaction 状态失败",
            )
        })?;

        if matches!(
            state.lifecycle_state,
            NativeInteractionLifecycleState::Uninitialized
                | NativeInteractionLifecycleState::Failed
        ) {
            return Ok(());
        }

        hide_native_interaction_backend(&mut state)?;
        state.lifecycle_state = NativeInteractionLifecycleState::Hidden;
        let snapshot = snapshot_native_interaction_backend(&mut state)?;
        let event = build_state_updated_event(&state, &snapshot);
        log::info!(
            target: "bexo::service::native_interaction",
            "native_interaction_window_hidden has_active_session={} total_ms={}",
            state.active_session.is_some(),
            started_at.elapsed().as_millis()
        );
        emit_native_interaction_backend_event(
            &state,
            NativeInteractionBackendEvent::StateUpdated(event),
        );
        Ok(())
    }

    pub fn clear(&self) -> AppResult<()> {
        let mut state = self.state.lock().map_err(|_| {
            AppError::new(
                "NATIVE_INTERACTION_STATE_LOCK_FAILED",
                "读取 native interaction 状态失败",
            )
        })?;

        if state.lifecycle_state == NativeInteractionLifecycleState::Uninitialized {
            return Ok(());
        }

        clear_native_interaction_backend(&mut state)?;
        state.active_session = None;
        if state.backend_kind.is_some() {
            state.lifecycle_state = NativeInteractionLifecycleState::Ready;
        }
        let snapshot = snapshot_native_interaction_backend(&mut state)?;
        let event = build_state_updated_event(&state, &snapshot);
        log::info!(
            target: "bexo::service::native_interaction",
            "native_interaction_service_cleared backend={}",
            state
                .backend_kind
                .map(NativeInteractionBackendKind::as_str)
                .unwrap_or("unknown")
        );
        emit_native_interaction_backend_event(
            &state,
            NativeInteractionBackendEvent::StateUpdated(event),
        );
        Ok(())
    }

    pub fn selection_snapshot(&self) -> AppResult<NativeInteractionSelectionSnapshot> {
        let mut state = self.state.lock().map_err(|_| {
            AppError::new(
                "NATIVE_INTERACTION_STATE_LOCK_FAILED",
                "读取 native interaction 状态失败",
            )
        })?;
        snapshot_native_interaction_backend(&mut state)
    }

    pub fn snapshot_state(&self) -> AppResult<NativeInteractionStateView> {
        let mut state = self.state.lock().map_err(|_| {
            AppError::new(
                "NATIVE_INTERACTION_STATE_LOCK_FAILED",
                "读取 native interaction 状态失败",
            )
        })?;
        let selection = snapshot_native_interaction_backend(&mut state)?;

        Ok(build_state_view(&state, &selection))
    }

    pub fn update_exclusion_rects(
        &self,
        session_id: String,
        exclusion_rects: Vec<NativeInteractionExclusionRect>,
    ) -> AppResult<()> {
        if session_id.trim().is_empty() {
            return Err(AppError::validation(
                "native interaction sessionId 不能为空",
            ));
        }

        let started_at = Instant::now();
        let mut state = self.state.lock().map_err(|_| {
            AppError::new(
                "NATIVE_INTERACTION_STATE_LOCK_FAILED",
                "读取 native interaction 状态失败",
            )
        })?;

        let active_session_id = match state.active_session.as_ref() {
            Some(value) => value.session_id.clone(),
            None => {
                return Err(AppError::new(
                    "NATIVE_INTERACTION_SESSION_NOT_PREPARED",
                    "native interaction 未准备可显示会话",
                ));
            }
        };

        if active_session_id != session_id {
            return Err(AppError::new(
                "NATIVE_INTERACTION_SESSION_MISMATCH",
                "native interaction 会话不匹配",
            )
            .with_detail("expectedSessionId", active_session_id)
            .with_detail("actualSessionId", session_id));
        }

        update_native_interaction_backend_exclusion_rects(&mut state, &exclusion_rects)?;

        log::debug!(
            target: "bexo::service::native_interaction",
            "native_interaction_exclusion_rects_updated session_id={} rects={} lifecycle_state={} total_ms={}",
            active_session_id,
            exclusion_rects.len(),
            state.lifecycle_state.as_str(),
            started_at.elapsed().as_millis()
        );

        Ok(())
    }

    pub fn update_runtime(
        &self,
        input: NativeInteractionRuntimeUpdateInput,
    ) -> AppResult<NativeInteractionStateView> {
        if input.session_id.trim().is_empty() {
            return Err(AppError::validation(
                "native interaction sessionId 不能为空",
            ));
        }

        let started_at = Instant::now();
        let mut state = self.state.lock().map_err(|_| {
            AppError::new(
                "NATIVE_INTERACTION_STATE_LOCK_FAILED",
                "读取 native interaction 状态失败",
            )
        })?;

        let active_session = match state.active_session.as_ref() {
            Some(value) => value,
            None => {
                log::warn!(
                    target: "bexo::service::native_interaction",
                    "native_interaction_runtime_rejected reason=session_not_prepared requested_session_id={} visible={} mode={} lifecycle_state={}",
                    input.session_id,
                    input.visible,
                    input.mode.as_str(),
                    state.lifecycle_state.as_str()
                );
                return Err(AppError::new(
                    "NATIVE_INTERACTION_SESSION_NOT_PREPARED",
                    "native interaction 未准备可显示会话",
                ));
            }
        };
        let active_session_id = active_session.session_id.clone();
        if active_session_id != input.session_id {
            log::warn!(
                target: "bexo::service::native_interaction",
                "native_interaction_runtime_rejected reason=session_mismatch requested_session_id={} active_session_id={} visible={} mode={} lifecycle_state={}",
                input.session_id,
                active_session_id,
                input.visible,
                input.mode.as_str(),
                state.lifecycle_state.as_str()
            );
            return Err(AppError::new(
                "NATIVE_INTERACTION_SESSION_MISMATCH",
                "native interaction 会话不匹配",
            )
            .with_detail("expectedSessionId", active_session_id)
            .with_detail("actualSessionId", input.session_id));
        }

        update_native_interaction_backend_runtime(&mut state, &input)?;
        if input.visible {
            show_native_interaction_backend(&mut state)?;
            state.lifecycle_state = NativeInteractionLifecycleState::Visible;
        } else {
            hide_native_interaction_backend(&mut state)?;
            state.lifecycle_state = NativeInteractionLifecycleState::Hidden;
        }
        let selection = snapshot_native_interaction_backend(&mut state)?;
        let view = build_state_view(&state, &selection);
        let event = build_state_updated_event(&state, &selection);

        log::debug!(
            target: "bexo::service::native_interaction",
            "native_interaction_runtime_updated session_id={} visible={} exclusion_rects={} mode={} lifecycle_state={} total_ms={}",
            active_session_id,
            input.visible,
            input.exclusion_rects.len(),
            input.mode.as_str(),
            view.lifecycle_state,
            started_at.elapsed().as_millis()
        );

        emit_native_interaction_backend_event(
            &state,
            NativeInteractionBackendEvent::StateUpdated(event),
        );

        Ok(view)
    }

    pub fn mark_initialization_failed(&self, error: AppError) {
        if let Ok(mut state) = self.state.lock() {
            state.lifecycle_state = NativeInteractionLifecycleState::Failed;
            state.last_error = Some(error);
        }
    }
}

fn build_state_view(
    state: &NativeInteractionState,
    snapshot: &NativeInteractionSelectionSnapshot,
) -> NativeInteractionStateView {
    NativeInteractionStateView {
        backend_kind: state.backend_kind,
        lifecycle_state: state.lifecycle_state.as_str(),
        has_active_session: state.active_session.is_some(),
        selection: snapshot.selection,
        active_shape: snapshot.active_shape.clone(),
        active_shape_draft: snapshot.active_shape_draft.clone(),
        hovered_hit_region: snapshot.hovered_hit_region.as_str(),
        drag_mode: snapshot.drag_mode.map(NativeInteractionDragMode::as_str),
        selection_revision: snapshot.selection_revision,
        active_shape_revision: snapshot.active_shape_revision,
        interaction_mode: snapshot.interaction_mode.as_str(),
        rect_draft: snapshot.rect_draft,
    }
}

fn build_state_updated_event(
    state: &NativeInteractionState,
    snapshot: &NativeInteractionSelectionSnapshot,
) -> NativeInteractionStateUpdatedEvent {
    NativeInteractionStateUpdatedEvent {
        session_id: state
            .active_session
            .as_ref()
            .map(|session| session.session_id.clone()),
        backend_kind: state.backend_kind,
        lifecycle_state: state.lifecycle_state.as_str().to_string(),
        has_active_session: state.active_session.is_some(),
        selection: snapshot.selection,
        active_shape: snapshot.active_shape.clone(),
        active_shape_draft: snapshot.active_shape_draft.clone(),
        hovered_hit_region: snapshot.hovered_hit_region.as_str().to_string(),
        drag_mode: snapshot
            .drag_mode
            .map(NativeInteractionDragMode::as_str)
            .map(str::to_string),
        selection_revision: snapshot.selection_revision,
        active_shape_revision: snapshot.active_shape_revision,
        interaction_mode: snapshot.interaction_mode.as_str().to_string(),
        rect_draft: snapshot.rect_draft,
    }
}

fn emit_native_interaction_backend_event(
    state: &NativeInteractionState,
    event: NativeInteractionBackendEvent,
) {
    let Some(event_sink) = state.event_sink.clone() else {
        return;
    };
    event_sink(event);
}

fn build_native_interaction_event_sink<R: tauri::Runtime>(
    app_handle: &AppHandle<R>,
) -> NativeInteractionEventSink {
    let app_handle = app_handle.clone();
    Arc::new(move |event| match event {
        NativeInteractionBackendEvent::StateUpdated(payload) => {
            if let Err(error) =
                app_handle.emit(NATIVE_INTERACTION_STATE_UPDATED_EVENT_NAME, payload.clone())
            {
                log::warn!(
                    target: "bexo::service::native_interaction",
                    "emit native interaction state updated event failed: {}",
                    error
                );
            } else {
                log::debug!(
                    target: "bexo::service::native_interaction",
                    "native_interaction_state_updated_emitted session_id={} revision={} drag_mode={} mode={}",
                    payload.session_id.as_deref().unwrap_or("none"),
                    payload.selection_revision,
                    payload.drag_mode.as_deref().unwrap_or("none"),
                    payload.interaction_mode
                );
            }
        }
        NativeInteractionBackendEvent::ShapeAnnotationCommitted(payload) => {
            if let Err(error) = app_handle.emit(
                NATIVE_INTERACTION_SHAPE_ANNOTATION_COMMITTED_EVENT_NAME,
                payload.clone(),
            ) {
                log::warn!(
                    target: "bexo::service::native_interaction",
                    "emit native interaction shape annotation committed event failed: {}",
                    error
                );
            } else {
                log::info!(
                    target: "bexo::service::native_interaction",
                    "native_interaction_shape_annotation_committed session_id={} kind={} color={} stroke_width={} start=({:.1},{:.1}) end=({:.1},{:.1})",
                    payload.session_id,
                    match payload.kind {
                        NativeInteractionShapeAnnotationKind::Line => "line",
                        NativeInteractionShapeAnnotationKind::Rect => "rect",
                        NativeInteractionShapeAnnotationKind::Ellipse => "ellipse",
                        NativeInteractionShapeAnnotationKind::Arrow => "arrow",
                    },
                    payload.color,
                    payload.stroke_width,
                    payload.start.x,
                    payload.start.y,
                    payload.end.x,
                    payload.end.y
                );
            }
        }
        NativeInteractionBackendEvent::ShapeAnnotationUpdated(payload) => {
            if let Err(error) = app_handle.emit(
                NATIVE_INTERACTION_SHAPE_ANNOTATION_UPDATED_EVENT_NAME,
                payload.clone(),
            ) {
                log::warn!(
                    target: "bexo::service::native_interaction",
                    "emit native interaction shape annotation updated event failed: {}",
                    error
                );
            } else {
                log::info!(
                    target: "bexo::service::native_interaction",
                    "native_interaction_shape_annotation_updated session_id={} id={} kind={} color={} stroke_width={} start=({:.1},{:.1}) end=({:.1},{:.1})",
                    payload.session_id,
                    payload.id,
                    match payload.kind {
                        NativeInteractionShapeAnnotationKind::Line => "line",
                        NativeInteractionShapeAnnotationKind::Rect => "rect",
                        NativeInteractionShapeAnnotationKind::Ellipse => "ellipse",
                        NativeInteractionShapeAnnotationKind::Arrow => "arrow",
                    },
                    payload.color,
                    payload.stroke_width,
                    payload.start.x,
                    payload.start.y,
                    payload.end.x,
                    payload.end.y
                );
            }
        }
        NativeInteractionBackendEvent::CancelRequested {
            session_id,
            shortcut,
        } => {
            let app_handle = app_handle.clone();
            std::thread::spawn(move || {
                let screenshot_service = app_handle.state::<crate::services::ScreenshotService>();
                match screenshot_service.cancel_active_session_from_escape(&app_handle) {
                    Ok(Some(cancelled_session_id)) => {
                        log::info!(
                            target: "bexo::service::native_interaction",
                            "native_interaction_cancel_requested session_id={} shortcut={} cancelled_session_id={}",
                            session_id,
                            shortcut,
                            cancelled_session_id
                        );
                    }
                    Ok(None) => {
                        log::debug!(
                            target: "bexo::service::native_interaction",
                            "native_interaction_cancel_requested_ignored session_id={} shortcut={} reason=no_active_session",
                            session_id,
                            shortcut
                        );
                    }
                    Err(error) => {
                        log::warn!(
                            target: "bexo::service::native_interaction",
                            "native_interaction_cancel_requested_failed session_id={} shortcut={} reason={}",
                            session_id,
                            shortcut,
                            error
                        );
                    }
                }
            });
        }
    })
}

fn validate_session_spec(session: &NativeInteractionSessionSpec) -> AppResult<()> {
    if session.session_id.trim().is_empty() {
        return Err(AppError::validation(
            "native interaction sessionId 不能为空",
        ));
    }
    if session.display_width == 0 || session.display_height == 0 {
        return Err(AppError::validation("native interaction 显示区域尺寸无效"));
    }
    if !(session.scale_factor.is_finite() && session.scale_factor > 0.0) {
        return Err(AppError::validation("native interaction 缩放因子无效"));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn prepare_native_interaction_backend(
    state: &mut NativeInteractionState,
    session: &NativeInteractionSessionSpec,
) -> AppResult<NativeInteractionPrepareMetrics> {
    let handle = state.backend_handle.as_mut().ok_or_else(|| {
        AppError::new(
            "NATIVE_INTERACTION_BACKEND_UNAVAILABLE",
            "native interaction backend 不可用",
        )
    })?;
    let backend = unsafe { handle.windows_interaction_window_mut()? };
    let prepare = backend.prepare_session(session)?;
    Ok(NativeInteractionPrepareMetrics {
        total_ms: prepare.total_ms,
        window_x: prepare.window_x,
        window_y: prepare.window_y,
        window_width: prepare.window_width,
        window_height: prepare.window_height,
        present_ms: prepare.present_ms,
        copy_ms: prepare.copy_ms,
        update_ms: prepare.update_ms,
        surface_recreated: prepare.surface_recreated,
    })
}

#[cfg(not(target_os = "windows"))]
fn prepare_native_interaction_backend(
    _state: &mut NativeInteractionState,
    _session: &NativeInteractionSessionSpec,
) -> AppResult<NativeInteractionPrepareMetrics> {
    Err(AppError::new(
        "NATIVE_INTERACTION_UNSUPPORTED",
        "native interaction 仅支持 Windows",
    ))
}

#[cfg(target_os = "windows")]
fn show_native_interaction_backend(state: &mut NativeInteractionState) -> AppResult<()> {
    let handle = state.backend_handle.as_mut().ok_or_else(|| {
        AppError::new(
            "NATIVE_INTERACTION_BACKEND_UNAVAILABLE",
            "native interaction backend 不可用",
        )
    })?;
    let backend = unsafe { handle.windows_interaction_window_mut()? };
    backend.show()
}

#[cfg(not(target_os = "windows"))]
fn show_native_interaction_backend(_state: &mut NativeInteractionState) -> AppResult<()> {
    Err(AppError::new(
        "NATIVE_INTERACTION_UNSUPPORTED",
        "native interaction 仅支持 Windows",
    ))
}

#[cfg(target_os = "windows")]
fn hide_native_interaction_backend(state: &mut NativeInteractionState) -> AppResult<()> {
    let handle = state.backend_handle.as_mut().ok_or_else(|| {
        AppError::new(
            "NATIVE_INTERACTION_BACKEND_UNAVAILABLE",
            "native interaction backend 不可用",
        )
    })?;
    let backend = unsafe { handle.windows_interaction_window_mut()? };
    backend.hide()
}

#[cfg(not(target_os = "windows"))]
fn hide_native_interaction_backend(_state: &mut NativeInteractionState) -> AppResult<()> {
    Err(AppError::new(
        "NATIVE_INTERACTION_UNSUPPORTED",
        "native interaction 仅支持 Windows",
    ))
}

#[cfg(target_os = "windows")]
fn clear_native_interaction_backend(state: &mut NativeInteractionState) -> AppResult<()> {
    let handle = state.backend_handle.as_mut().ok_or_else(|| {
        AppError::new(
            "NATIVE_INTERACTION_BACKEND_UNAVAILABLE",
            "native interaction backend 不可用",
        )
    })?;
    let backend = unsafe { handle.windows_interaction_window_mut()? };
    backend.clear()
}

#[cfg(target_os = "windows")]
fn update_native_interaction_backend_exclusion_rects(
    state: &mut NativeInteractionState,
    rects: &[NativeInteractionExclusionRect],
) -> AppResult<()> {
    let handle = state.backend_handle.as_mut().ok_or_else(|| {
        AppError::new(
            "NATIVE_INTERACTION_BACKEND_UNAVAILABLE",
            "native interaction backend 不可用",
        )
    })?;
    let backend = unsafe { handle.windows_interaction_window_mut()? };
    backend.update_exclusion_rects(rects)
}

#[cfg(target_os = "windows")]
fn update_native_interaction_backend_runtime(
    state: &mut NativeInteractionState,
    input: &NativeInteractionRuntimeUpdateInput,
) -> AppResult<()> {
    let handle = state.backend_handle.as_mut().ok_or_else(|| {
        AppError::new(
            "NATIVE_INTERACTION_BACKEND_UNAVAILABLE",
            "native interaction backend 不可用",
        )
    })?;
    let backend = unsafe { handle.windows_interaction_window_mut()? };
    backend.update_runtime(input)
}

#[cfg(not(target_os = "windows"))]
fn update_native_interaction_backend_runtime(
    _state: &mut NativeInteractionState,
    _input: &NativeInteractionRuntimeUpdateInput,
) -> AppResult<()> {
    Err(AppError::new(
        "NATIVE_INTERACTION_UNSUPPORTED",
        "native interaction 仅支持 Windows",
    ))
}

#[cfg(target_os = "windows")]
fn set_native_interaction_backend_event_sink(
    state: &mut NativeInteractionState,
    event_sink: NativeInteractionEventSink,
) -> AppResult<()> {
    let handle = state.backend_handle.as_mut().ok_or_else(|| {
        AppError::new(
            "NATIVE_INTERACTION_BACKEND_UNAVAILABLE",
            "native interaction backend 不可用",
        )
    })?;
    let backend = unsafe { handle.windows_interaction_window_mut()? };
    backend.set_event_sink(event_sink)
}

#[cfg(not(target_os = "windows"))]
fn set_native_interaction_backend_event_sink(
    _state: &mut NativeInteractionState,
    _event_sink: NativeInteractionEventSink,
) -> AppResult<()> {
    Err(AppError::new(
        "NATIVE_INTERACTION_UNSUPPORTED",
        "native interaction 仅支持 Windows",
    ))
}

#[cfg(not(target_os = "windows"))]
fn update_native_interaction_backend_exclusion_rects(
    _state: &mut NativeInteractionState,
    _rects: &[NativeInteractionExclusionRect],
) -> AppResult<()> {
    Err(AppError::new(
        "NATIVE_INTERACTION_UNSUPPORTED",
        "native interaction 仅支持 Windows",
    ))
}

#[cfg(not(target_os = "windows"))]
fn clear_native_interaction_backend(_state: &mut NativeInteractionState) -> AppResult<()> {
    Err(AppError::new(
        "NATIVE_INTERACTION_UNSUPPORTED",
        "native interaction 仅支持 Windows",
    ))
}

#[cfg(target_os = "windows")]
fn snapshot_native_interaction_backend(
    state: &mut NativeInteractionState,
) -> AppResult<NativeInteractionSelectionSnapshot> {
    let handle = state.backend_handle.as_mut().ok_or_else(|| {
        AppError::new(
            "NATIVE_INTERACTION_BACKEND_UNAVAILABLE",
            "native interaction backend 不可用",
        )
    })?;
    let backend = unsafe { handle.windows_interaction_window_mut()? };
    backend.snapshot_selection()
}

#[cfg(not(target_os = "windows"))]
fn snapshot_native_interaction_backend(
    _state: &mut NativeInteractionState,
) -> AppResult<NativeInteractionSelectionSnapshot> {
    Err(AppError::new(
        "NATIVE_INTERACTION_UNSUPPORTED",
        "native interaction 仅支持 Windows",
    ))
}

#[cfg(target_os = "windows")]
fn bootstrap_native_interaction_backend() -> AppResult<NativeInteractionBackendBootstrap> {
    let (backend, started) = native_interaction_backend_windows::initialize()?;
    Ok(NativeInteractionBackendBootstrap {
        backend_handle: NativeInteractionBackendHandle::from_windows_interaction_window(backend),
        backend_kind: NativeInteractionBackendKind::WindowsLayeredSelectionMvp,
        window_create_ms: started.window_create_ms,
        initial_hide_ms: started.initial_hide_ms,
    })
}

#[cfg(not(target_os = "windows"))]
fn bootstrap_native_interaction_backend() -> AppResult<NativeInteractionBackendBootstrap> {
    Err(AppError::new(
        "NATIVE_INTERACTION_UNSUPPORTED",
        "native interaction 仅支持 Windows",
    ))
}

#[cfg(target_os = "windows")]
impl NativeInteractionBackendHandle {
    unsafe fn windows_interaction_window_mut(
        &mut self,
    ) -> AppResult<&mut native_interaction_backend_windows::NativeInteractionWindowsBackend> {
        if self.raw == 0 {
            return Err(AppError::new(
                "NATIVE_INTERACTION_BACKEND_UNAVAILABLE",
                "native interaction backend 不可用",
            ));
        }
        if !matches!(
            self.kind,
            NativeInteractionBackendKind::WindowsLayeredSelectionMvp
        ) {
            return Err(AppError::new(
                "NATIVE_INTERACTION_BACKEND_KIND_INVALID",
                "native interaction backend 类型无效",
            ));
        }

        Ok(unsafe {
            &mut *(self.raw
                as *mut native_interaction_backend_windows::NativeInteractionWindowsBackend)
        })
    }
}
