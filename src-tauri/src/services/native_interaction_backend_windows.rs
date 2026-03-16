#![cfg(target_os = "windows")]

use std::{
    mem::{size_of, zeroed},
    ptr::{copy_nonoverlapping, null},
    sync::{Arc, Mutex},
    time::Instant,
};

use windows::core::w;
use windows_sys::Win32::{
    Foundation::{GetLastError, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM},
    Graphics::Gdi::{
        CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC,
        SelectObject, AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
        BLENDFUNCTION, DIB_RGB_COLORS, HDC, HGDIOBJ,
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture},
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowLongPtrW, LoadCursorW,
        RegisterClassExW, SetCursor, SetWindowLongPtrW, SetWindowPos, ShowWindow,
        UpdateLayeredWindow, CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, HWND_TOPMOST, IDC_ARROW,
        IDC_CROSS, IDC_SIZEALL, IDC_SIZENESW, IDC_SIZENS, IDC_SIZENWSE, IDC_SIZEWE,
        SWP_HIDEWINDOW, SWP_NOACTIVATE, SW_HIDE, SW_SHOWNOACTIVATE, ULW_ALPHA,
        WM_ERASEBKGND, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCCREATE, WM_NCDESTROY,
        WNDCLASSEXW, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
    },
};

use crate::{
    error::{AppError, AppResult},
    services::native_interaction_service::{
        NativeInteractionBackendEvent, NativeInteractionDragMode, NativeInteractionEventSink,
        NativeInteractionExclusionRect, NativeInteractionHitRegion, NativeInteractionMode,
        NativeInteractionRuntimeUpdateInput, NativeInteractionShapeAnnotationCommittedEvent,
        NativeInteractionShapeAnnotationKind,
        NativeInteractionSelectionHandle, NativeInteractionSelectionPoint,
        NativeInteractionSelectionRect, NativeInteractionSelectionSnapshot,
        NativeInteractionSessionSpec,
    },
};

const WINDOW_CLASS_NAME: windows::core::PCWSTR = w!("BexoStudioNativeInteractionWindow");
const WINDOW_TITLE: windows::core::PCWSTR = w!("Bexo Studio Native Interaction");
const INITIAL_WINDOW_WIDTH: i32 = 1;
const INITIAL_WINDOW_HEIGHT: i32 = 1;
const MASK_ALPHA: u8 = 112;
const BORDER_COLOR: [u8; 4] = [0x8F, 0xD0, 0x00, 0xFF];
const HANDLE_COLOR: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xF0];
const SELECTION_HOLE_ALPHA: u8 = 1;
const RECT_ANNOTATION_FILL_ALPHA: u8 = 28;
const MIN_SELECTION_SIZE_LOGICAL: f64 = 8.0;
const HANDLE_SIZE_LOGICAL: f64 = 10.0;
const BORDER_THICKNESS_LOGICAL: f64 = 2.0;
const NATIVE_INTERACTION_EVENT_THROTTLE_MS: u128 = 16;

pub struct NativeInteractionWindowsBackend {
    hwnd: HWND,
    visible: bool,
    current_window_x: i32,
    current_window_y: i32,
    current_window_width: u32,
    current_window_height: u32,
    shared_state: Arc<Mutex<InteractionWindowSharedState>>,
    userdata_ptr: usize,
}

pub struct NativeInteractionWindowsBackendStarted {
    pub window_create_ms: u128,
    pub initial_hide_ms: u128,
}

pub struct NativeInteractionPrepareResult {
    pub present_ms: u128,
    pub copy_ms: u128,
    pub update_ms: u128,
    pub surface_recreated: bool,
    pub total_ms: u128,
    pub window_x: i32,
    pub window_y: i32,
    pub window_width: u32,
    pub window_height: u32,
}

#[derive(Debug, Clone, Copy)]
struct InteractionPresentMetrics {
    copy_ms: u128,
    update_ms: u128,
    total_ms: u128,
    surface_recreated: bool,
}

#[derive(Debug, Clone)]
struct InteractionWindowSession {
    session_id: String,
    physical_x: i32,
    physical_y: i32,
    physical_width: u32,
    physical_height: u32,
    scale_factor: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PhysicalRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeInteractionCursorKind {
    Crosshair,
    Move,
    ResizeNs,
    ResizeWe,
    ResizeNwse,
    ResizeNesw,
}

struct InteractionWindowSharedState {
    session: Option<InteractionWindowSession>,
    selection_physical: Option<PhysicalRect>,
    interaction_mode: NativeInteractionMode,
    exclusion_rects_physical: Vec<PhysicalRect>,
    rect_annotation_color_hex: String,
    rect_annotation_color: [u8; 4],
    rect_annotation_stroke_width_physical: i32,
    rect_annotation_draft_physical: Option<PhysicalRect>,
    hovered_hit_region: NativeInteractionHitRegion,
    drag_mode: Option<NativeInteractionDragMode>,
    selection_revision: u64,
    rect_draft_revision: u64,
    drag_origin_point: Option<(i32, i32)>,
    drag_origin_selection: Option<PhysicalRect>,
    drag_started_at: Option<Instant>,
    pixel_buffer: Vec<u8>,
    base_mask_buffer: Vec<u8>,
    layered_surface: Option<LayeredWindowSurface>,
    drag_present_samples: u32,
    drag_present_total_ms: u128,
    drag_present_max_ms: u128,
    last_cursor_kind: NativeInteractionCursorKind,
    last_state_event_ms: Option<Instant>,
    event_sink: Option<NativeInteractionEventSink>,
}

impl Default for InteractionWindowSharedState {
    fn default() -> Self {
        Self {
            session: None,
            selection_physical: None,
            interaction_mode: NativeInteractionMode::Selection,
            exclusion_rects_physical: Vec::new(),
            rect_annotation_color_hex: "#00d08f".to_string(),
            rect_annotation_color: BORDER_COLOR,
            rect_annotation_stroke_width_physical: 2,
            rect_annotation_draft_physical: None,
            hovered_hit_region: NativeInteractionHitRegion::None,
            drag_mode: None,
            selection_revision: 0,
            rect_draft_revision: 0,
            drag_origin_point: None,
            drag_origin_selection: None,
            drag_started_at: None,
            pixel_buffer: Vec::new(),
            base_mask_buffer: Vec::new(),
            layered_surface: None,
            drag_present_samples: 0,
            drag_present_total_ms: 0,
            drag_present_max_ms: 0,
            last_cursor_kind: NativeInteractionCursorKind::Crosshair,
            last_state_event_ms: None,
            event_sink: None,
        }
    }
}

#[derive(Debug)]
struct LayeredWindowSurface {
    screen_dc: HDC,
    mem_dc: HDC,
    bitmap: HGDIOBJ,
    previous_bitmap: HGDIOBJ,
    bitmap_bits: *mut u8,
    width: u32,
    height: u32,
}

impl LayeredWindowSurface {
    fn create(width: u32, height: u32) -> AppResult<Self> {
        unsafe {
            let screen_dc = GetDC(0);
            if screen_dc == 0 {
                return Err(last_error(
                    "NATIVE_INTERACTION_GET_DC_FAILED",
                    "获取屏幕 DC 失败",
                ));
            }
            let mem_dc = CreateCompatibleDC(screen_dc);
            if mem_dc == 0 {
                ReleaseDC(0, screen_dc);
                return Err(last_error(
                    "NATIVE_INTERACTION_CREATE_DC_FAILED",
                    "创建内存 DC 失败",
                ));
            }

            let mut bmi: BITMAPINFO = zeroed();
            bmi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = width as i32;
            bmi.bmiHeader.biHeight = -(height as i32);
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = BI_RGB;

            let mut dib_bits = null::<core::ffi::c_void>() as *mut core::ffi::c_void;
            let dib = CreateDIBSection(mem_dc, &bmi, DIB_RGB_COLORS, &mut dib_bits, 0, 0);
            if dib == 0 {
                DeleteDC(mem_dc);
                ReleaseDC(0, screen_dc);
                return Err(last_error(
                    "NATIVE_INTERACTION_CREATE_DIB_FAILED",
                    "创建 DIBSection 失败",
                ));
            }

            let previous = SelectObject(mem_dc, dib as HGDIOBJ);
            if previous == 0 {
                DeleteObject(dib as HGDIOBJ);
                DeleteDC(mem_dc);
                ReleaseDC(0, screen_dc);
                return Err(last_error(
                    "NATIVE_INTERACTION_SELECT_BITMAP_FAILED",
                    "选择 DIBSection 到内存 DC 失败",
                ));
            }

            Ok(Self {
                screen_dc,
                mem_dc,
                bitmap: dib as HGDIOBJ,
                previous_bitmap: previous,
                bitmap_bits: dib_bits as *mut u8,
                width,
                height,
            })
        }
    }

    fn matches_size(&self, width: u32, height: u32) -> bool {
        self.width == width && self.height == height
    }
}

impl Drop for LayeredWindowSurface {
    fn drop(&mut self) {
        unsafe {
            if self.mem_dc != 0 {
                if self.previous_bitmap != 0 {
                    SelectObject(self.mem_dc, self.previous_bitmap);
                    self.previous_bitmap = 0;
                }
                if self.bitmap != 0 {
                    DeleteObject(self.bitmap);
                    self.bitmap = 0;
                }
                DeleteDC(self.mem_dc);
                self.mem_dc = 0;
            }
            if self.screen_dc != 0 {
                ReleaseDC(0, self.screen_dc);
                self.screen_dc = 0;
            }
            self.bitmap_bits = std::ptr::null_mut();
        }
    }
}

pub fn initialize() -> AppResult<(
    NativeInteractionWindowsBackend,
    NativeInteractionWindowsBackendStarted,
)> {
    let shared_state = Arc::new(Mutex::new(InteractionWindowSharedState::default()));
    let window_started_at = Instant::now();
    let hwnd = create_native_interaction_window()?;
    let userdata_ptr = Box::into_raw(Box::new(shared_state.clone())) as usize;
    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, userdata_ptr as isize);
    }
    let window_create_ms = window_started_at.elapsed().as_millis();

    let hide_started_at = Instant::now();
    unsafe {
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            INITIAL_WINDOW_WIDTH,
            INITIAL_WINDOW_HEIGHT,
            SWP_HIDEWINDOW | SWP_NOACTIVATE,
        );
        ShowWindow(hwnd, SW_HIDE);
    }
    let initial_hide_ms = hide_started_at.elapsed().as_millis();

    Ok((
        NativeInteractionWindowsBackend {
            hwnd,
            visible: false,
            current_window_x: 0,
            current_window_y: 0,
            current_window_width: INITIAL_WINDOW_WIDTH as u32,
            current_window_height: INITIAL_WINDOW_HEIGHT as u32,
            shared_state,
            userdata_ptr,
        },
        NativeInteractionWindowsBackendStarted {
            window_create_ms,
            initial_hide_ms,
        },
    ))
}

impl NativeInteractionWindowsBackend {
    pub fn set_event_sink(&mut self, event_sink: NativeInteractionEventSink) -> AppResult<()> {
        let mut shared = self.lock_state()?;
        shared.event_sink = Some(event_sink);
        Ok(())
    }

    pub fn prepare_session(
        &mut self,
        session: &NativeInteractionSessionSpec,
    ) -> AppResult<NativeInteractionPrepareResult> {
        let started_at = Instant::now();
        let (window_x, window_y, window_width, window_height) =
            resolve_physical_window_geometry(session)?;
        resize_window(
            self.hwnd,
            window_x,
            window_y,
            window_width,
            window_height,
            self.visible,
        )?;
        self.current_window_x = window_x;
        self.current_window_y = window_y;
        self.current_window_width = window_width;
        self.current_window_height = window_height;

        {
            let mut shared = self.lock_state()?;
            shared.session = Some(InteractionWindowSession {
                session_id: session.session_id.clone(),
                physical_x: window_x,
                physical_y: window_y,
                physical_width: window_width,
                physical_height: window_height,
                scale_factor: session.scale_factor,
            });
            shared.selection_physical = None;
            shared.interaction_mode = NativeInteractionMode::Selection;
            shared.exclusion_rects_physical.clear();
            shared.rect_annotation_color = BORDER_COLOR;
            shared.rect_annotation_stroke_width_physical =
                logical_to_physical(BORDER_THICKNESS_LOGICAL, session.scale_factor).max(1);
            shared.rect_annotation_draft_physical = None;
            shared.hovered_hit_region = NativeInteractionHitRegion::None;
            shared.drag_mode = None;
            shared.drag_origin_point = None;
            shared.drag_origin_selection = None;
            shared.drag_started_at = None;
            shared.selection_revision = 0;
            shared.rect_draft_revision = 0;
            shared.drag_present_samples = 0;
            shared.drag_present_total_ms = 0;
            shared.drag_present_max_ms = 0;
            shared.last_cursor_kind = NativeInteractionCursorKind::Crosshair;
            shared.last_state_event_ms = None;
            ensure_pixel_buffer(&mut shared, window_width, window_height)?;
        }

        let present_metrics = present_interaction_surface(self.hwnd, &self.shared_state)?;
        let present_ms = present_metrics.total_ms;

        Ok(NativeInteractionPrepareResult {
            present_ms,
            copy_ms: present_metrics.copy_ms,
            update_ms: present_metrics.update_ms,
            surface_recreated: present_metrics.surface_recreated,
            total_ms: started_at.elapsed().as_millis(),
            window_x,
            window_y,
            window_width,
            window_height,
        })
    }

    pub fn show(&mut self) -> AppResult<()> {
        unsafe {
            SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                self.current_window_x,
                self.current_window_y,
                self.current_window_width as i32,
                self.current_window_height as i32,
                SWP_NOACTIVATE,
            );
            ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
        }
        self.visible = true;
        Ok(())
    }

    pub fn hide(&mut self) -> AppResult<()> {
        if !self.visible {
            return Ok(());
        }
        unsafe {
            ReleaseCapture();
            ShowWindow(self.hwnd, SW_HIDE);
        }
        self.visible = false;
        if let Ok(mut shared) = self.lock_state() {
            shared.rect_annotation_draft_physical = None;
            shared.drag_mode = None;
            shared.drag_origin_point = None;
            shared.drag_origin_selection = None;
            shared.drag_started_at = None;
            shared.hovered_hit_region = NativeInteractionHitRegion::None;
            shared.last_cursor_kind = NativeInteractionCursorKind::Crosshair;
        }
        Ok(())
    }

    pub fn clear(&mut self) -> AppResult<()> {
        let mut shared = self.lock_state()?;
        shared.selection_physical = None;
        shared.interaction_mode = NativeInteractionMode::Selection;
        shared.exclusion_rects_physical.clear();
        shared.rect_annotation_draft_physical = None;
        shared.hovered_hit_region = NativeInteractionHitRegion::None;
        shared.drag_mode = None;
        shared.drag_origin_point = None;
        shared.drag_origin_selection = None;
        shared.drag_started_at = None;
        shared.selection_revision = 0;
        shared.rect_draft_revision = 0;
        shared.drag_present_samples = 0;
        shared.drag_present_total_ms = 0;
        shared.drag_present_max_ms = 0;
        shared.last_cursor_kind = NativeInteractionCursorKind::Crosshair;
        Ok(())
    }

    pub fn snapshot_selection(&mut self) -> AppResult<NativeInteractionSelectionSnapshot> {
        let shared = self.lock_state()?;
        Ok(snapshot_from_shared(&shared))
    }

    pub fn update_exclusion_rects(
        &mut self,
        rects: &[NativeInteractionExclusionRect],
    ) -> AppResult<()> {
        let mut shared = self.lock_state()?;
        let Some(session) = shared.session.clone() else {
            return Ok(());
        };
        shared.exclusion_rects_physical = rects
            .iter()
            .filter_map(|rect| logical_to_physical_rect(*rect, &session))
            .collect();
        drop(shared);
        let _ = present_interaction_surface(self.hwnd, &self.shared_state)?;
        Ok(())
    }

    pub fn update_runtime(&mut self, input: &NativeInteractionRuntimeUpdateInput) -> AppResult<()> {
        let mut shared = self.lock_state()?;
        let Some(session) = shared.session.clone() else {
            return Ok(());
        };
        shared.interaction_mode = input.mode;
        if let Some(selection) = input.selection.and_then(|value| logical_selection_to_physical(value, &session)) {
            if shared.selection_physical != Some(selection) {
                shared.selection_physical = Some(selection);
                shared.selection_revision = shared.selection_revision.saturating_add(1);
            }
        }
        shared.exclusion_rects_physical = input
            .exclusion_rects
            .iter()
            .filter_map(|rect| logical_to_physical_rect(*rect, &session))
            .collect();
        if let Some(color_hex) = input.annotation_color.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
            if let Some(color) = parse_color_rgba(Some(color_hex)) {
                shared.rect_annotation_color = color;
                shared.rect_annotation_color_hex = color_hex.to_string();
            }
        }
        if let Some(stroke_width) = input.annotation_stroke_width {
            shared.rect_annotation_stroke_width_physical = logical_to_physical(
                stroke_width.clamp(1.0, 32.0),
                session.scale_factor,
            )
            .max(1);
        }
        if !matches!(
            input.mode,
            NativeInteractionMode::RectAnnotation | NativeInteractionMode::EllipseAnnotation
        ) && shared.rect_annotation_draft_physical.take().is_some()
        {
            shared.rect_draft_revision = shared.rect_draft_revision.saturating_add(1);
        }
        drop(shared);
        let _ = present_interaction_surface(self.hwnd, &self.shared_state)?;
        Ok(())
    }

    fn lock_state(&self) -> AppResult<std::sync::MutexGuard<'_, InteractionWindowSharedState>> {
        self.shared_state.lock().map_err(|_| {
            AppError::new(
                "NATIVE_INTERACTION_STATE_LOCK_FAILED",
                "读取 Native Interaction 共享状态失败",
            )
        })
    }
}

impl Drop for NativeInteractionWindowsBackend {
    fn drop(&mut self) {
        unsafe {
            if self.hwnd != 0 {
                ShowWindow(self.hwnd, SW_HIDE);
                ReleaseCapture();
                if self.userdata_ptr != 0 {
                    SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
                    drop(Box::from_raw(
                        self.userdata_ptr as *mut Arc<Mutex<InteractionWindowSharedState>>,
                    ));
                    self.userdata_ptr = 0;
                }
                DestroyWindow(self.hwnd);
                self.hwnd = 0;
            }
        }
    }
}

unsafe extern "system" fn native_interaction_window_proc(
    hwnd: HWND,
    message: u32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    match message {
        WM_NCCREATE => 1,
        WM_ERASEBKGND => 1,
        WM_MOUSEMOVE => {
            let (x, y) = mouse_point_from_lparam(l_param);
            if let Some(shared) = shared_state_from_hwnd(hwnd) {
                let _ = handle_mouse_move(hwnd, shared, x, y);
            }
            0
        }
        WM_LBUTTONDOWN => {
            let (x, y) = mouse_point_from_lparam(l_param);
            if let Some(shared) = shared_state_from_hwnd(hwnd) {
                let _ = handle_left_button_down(hwnd, shared, x, y);
            }
            0
        }
        WM_LBUTTONUP => {
            let (x, y) = mouse_point_from_lparam(l_param);
            if let Some(shared) = shared_state_from_hwnd(hwnd) {
                let _ = handle_left_button_up(hwnd, shared, x, y);
            }
            0
        }
        WM_NCDESTROY => {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            DefWindowProcW(hwnd, message, w_param, l_param)
        }
        _ => DefWindowProcW(hwnd, message, w_param, l_param),
    }
}

fn handle_left_button_down(
    hwnd: HWND,
    shared: &Arc<Mutex<InteractionWindowSharedState>>,
    x: i32,
    y: i32,
) -> AppResult<()> {
    let mut state = shared.lock().map_err(|_| {
        AppError::new(
            "NATIVE_INTERACTION_STATE_LOCK_FAILED",
            "读取 Native Interaction 共享状态失败",
        )
    })?;
    let Some(session) = state.session.clone() else {
        return Ok(());
    };
    let (x, y) = clamp_point_to_bounds(x, y, &session);
    let hit_region = hit_test_selection(&state, x, y);
    let drag_mode = match state.interaction_mode {
        NativeInteractionMode::Selection => match hit_region {
            NativeInteractionHitRegion::Handle(handle) => {
                NativeInteractionDragMode::Resizing(handle)
            }
            NativeInteractionHitRegion::SelectionBody => NativeInteractionDragMode::Moving,
            NativeInteractionHitRegion::None => NativeInteractionDragMode::Creating,
        },
        NativeInteractionMode::RectAnnotation | NativeInteractionMode::EllipseAnnotation => {
            let Some(selection) = state.selection_physical else {
                return Ok(());
            };
            if !point_in_box(
                x,
                y,
                selection.x.floor() as i32,
                selection.y.floor() as i32,
                selection.width.ceil() as i32,
                selection.height.ceil() as i32,
            ) {
                return Ok(());
            }
            if matches!(state.interaction_mode, NativeInteractionMode::EllipseAnnotation) {
                NativeInteractionDragMode::EllipseCreating
            } else {
                NativeInteractionDragMode::RectCreating
            }
        }
    };

    if matches!(
        drag_mode,
        NativeInteractionDragMode::RectCreating | NativeInteractionDragMode::EllipseCreating
    ) {
        let rect = build_physical_rect(x, y, x + 1, y + 1);
        if state.rect_annotation_draft_physical != Some(rect) {
            state.rect_annotation_draft_physical = Some(rect);
            state.rect_draft_revision = state.rect_draft_revision.saturating_add(1);
        }
        state.hovered_hit_region = NativeInteractionHitRegion::None;
    } else {
        state.hovered_hit_region = hit_region;
        if matches!(drag_mode, NativeInteractionDragMode::Creating) {
            state.selection_physical = Some(build_physical_rect(x, y, x + 1, y + 1));
            state.selection_revision = state.selection_revision.saturating_add(1);
        }
        state.rect_annotation_draft_physical = None;
    }

    state.drag_mode = Some(drag_mode);
    state.drag_origin_point = Some((x, y));
    state.drag_origin_selection = match drag_mode {
        NativeInteractionDragMode::RectCreating | NativeInteractionDragMode::EllipseCreating => {
            state.rect_annotation_draft_physical
        }
        _ => state.selection_physical,
    };
    state.drag_started_at = Some(Instant::now());
    state.drag_present_samples = 0;
    state.drag_present_total_ms = 0;
    state.drag_present_max_ms = 0;
    apply_cursor_for_shared_state(&mut state);
    let snapshot = snapshot_from_shared(&state);
    let event_sink = state.event_sink.clone();
    let throttled_event = emit_state_updated_from_shared(&mut state, true);
    let session_id = session.session_id.clone();
    drop(state);

    unsafe {
        SetCapture(hwnd);
    }
    let _ = present_interaction_surface(hwnd, shared)?;
    if let Some(event) = throttled_event {
        emit_backend_event(event_sink, event);
    }
    log::info!(
        target: "bexo::service::native_interaction",
        "native_interaction_drag_started session_id={} drag_mode={} hit_region={} point=({}, {}) selection={} mode={}",
        session_id,
        drag_mode.as_str(),
        snapshot.hovered_hit_region.as_str(),
        x,
        y,
        format_selection(snapshot.selection),
        snapshot.interaction_mode.as_str()
    );
    Ok(())
}

fn handle_mouse_move(
    hwnd: HWND,
    shared: &Arc<Mutex<InteractionWindowSharedState>>,
    x: i32,
    y: i32,
) -> AppResult<()> {
    let mut state = shared.lock().map_err(|_| {
        AppError::new(
            "NATIVE_INTERACTION_STATE_LOCK_FAILED",
            "读取 Native Interaction 共享状态失败",
        )
    })?;
    let Some(session) = state.session.clone() else {
        return Ok(());
    };
    let (x, y) = clamp_point_to_bounds(x, y, &session);
    let mut state_changed = false;

    if let Some(drag_mode) = state.drag_mode {
        let origin_point = state.drag_origin_point.unwrap_or((x, y));
        let origin_selection = state.drag_origin_selection.unwrap_or(PhysicalRect {
            x: f64::from(origin_point.0),
            y: f64::from(origin_point.1),
            width: 1.0,
            height: 1.0,
        });
        match drag_mode {
            NativeInteractionDragMode::Creating => {
                let next_selection = build_physical_rect(origin_point.0, origin_point.1, x, y);
                if state.selection_physical != Some(next_selection) {
                    state.selection_physical = Some(next_selection);
                    state.selection_revision = state.selection_revision.saturating_add(1);
                    state_changed = true;
                }
            }
            NativeInteractionDragMode::Moving => {
                let next_selection = move_selection_rect(
                    origin_selection,
                    x - origin_point.0,
                    y - origin_point.1,
                    &session,
                );
                if state.selection_physical != Some(next_selection) {
                    state.selection_physical = Some(next_selection);
                    state.selection_revision = state.selection_revision.saturating_add(1);
                    state_changed = true;
                }
            }
            NativeInteractionDragMode::Resizing(handle) => {
                let next_selection = resize_selection_rect(origin_selection, handle, x, y, &session);
                if state.selection_physical != Some(next_selection) {
                    state.selection_physical = Some(next_selection);
                    state.selection_revision = state.selection_revision.saturating_add(1);
                    state_changed = true;
                }
            }
            NativeInteractionDragMode::RectCreating | NativeInteractionDragMode::EllipseCreating => {
                let bounds = state.selection_physical.unwrap_or(origin_selection);
                let next_rect = clamp_rect_to_bounds(
                    build_physical_rect(origin_point.0, origin_point.1, x, y),
                    bounds,
                );
                if state.rect_annotation_draft_physical != Some(next_rect) {
                    state.rect_annotation_draft_physical = Some(next_rect);
                    state.rect_draft_revision = state.rect_draft_revision.saturating_add(1);
                    state_changed = true;
                }
            }
        }
        let next_hit = if matches!(state.interaction_mode, NativeInteractionMode::Selection) {
            hit_test_selection(&state, x, y)
        } else {
            NativeInteractionHitRegion::None
        };
        if state.hovered_hit_region != next_hit {
            state.hovered_hit_region = next_hit;
            state_changed = true;
        }
    } else if matches!(state.interaction_mode, NativeInteractionMode::Selection) {
        let next_hit = hit_test_selection(&state, x, y);
        if state.hovered_hit_region != next_hit {
            state.hovered_hit_region = next_hit;
            state_changed = true;
        }
    } else if state.hovered_hit_region != NativeInteractionHitRegion::None {
        state.hovered_hit_region = NativeInteractionHitRegion::None;
        state_changed = true;
    }

    apply_cursor_for_shared_state(&mut state);
    let snapshot = state_changed.then(|| snapshot_from_shared(&state));
    let event_sink = state.event_sink.clone();
    let throttled_event = if state_changed {
        emit_state_updated_from_shared(&mut state, false)
    } else {
        None
    };
    let session_id = session.session_id.clone();
    drop(state);

    if state_changed {
        let _ = present_interaction_surface(hwnd, shared)?;
        if let Some(event) = throttled_event {
            emit_backend_event(event_sink, event);
        }
        if let Some(snapshot) = snapshot {
            log::debug!(
                target: "bexo::service::native_interaction",
                "native_interaction_selection_updated session_id={} drag_mode={} selection={} revision={} mode={}",
                session_id,
                snapshot.drag_mode.map(NativeInteractionDragMode::as_str).unwrap_or("none"),
                format_selection(snapshot.selection),
                snapshot.selection_revision,
                snapshot.interaction_mode.as_str()
            );
        }
    }

    Ok(())
}

fn handle_left_button_up(
    hwnd: HWND,
    shared: &Arc<Mutex<InteractionWindowSharedState>>,
    x: i32,
    y: i32,
) -> AppResult<()> {
    let mut state = shared.lock().map_err(|_| {
        AppError::new(
            "NATIVE_INTERACTION_STATE_LOCK_FAILED",
            "读取 Native Interaction 共享状态失败",
        )
    })?;
    let Some(session) = state.session.clone() else {
        return Ok(());
    };
    if state.drag_mode.is_none() {
        return Ok(());
    }
    let (x, y) = clamp_point_to_bounds(x, y, &session);
    let event_sink = state.event_sink.clone();
    let mut backend_event = None;
    if matches!(state.interaction_mode, NativeInteractionMode::Selection) {
        state.hovered_hit_region = hit_test_selection(&state, x, y);
    } else {
        state.hovered_hit_region = NativeInteractionHitRegion::None;
    }
    let drag_mode = state.drag_mode.take();
    state.drag_origin_point = None;
    state.drag_origin_selection = None;
    let drag_elapsed_ms = state
        .drag_started_at
        .take()
        .map(|value| value.elapsed().as_millis())
        .unwrap_or(0);
    let drag_present_samples = state.drag_present_samples;
    let drag_present_total_ms = state.drag_present_total_ms;
    let drag_present_max_ms = state.drag_present_max_ms;
    let drag_present_avg_ms = if drag_present_samples == 0 {
        0
    } else {
        drag_present_total_ms / u128::from(drag_present_samples)
    };
    if let Some(
        NativeInteractionDragMode::RectCreating | NativeInteractionDragMode::EllipseCreating,
    ) = drag_mode
    {
        if let Some(draft) = state.rect_annotation_draft_physical.take() {
            state.rect_draft_revision = state.rect_draft_revision.saturating_add(1);
            let logical = physical_to_logical(draft, &session);
            if logical.width >= 2.0 && logical.height >= 2.0 {
                backend_event = Some(NativeInteractionBackendEvent::ShapeAnnotationCommitted(
                    NativeInteractionShapeAnnotationCommittedEvent {
                        session_id: session.session_id.clone(),
                        kind: if matches!(state.interaction_mode, NativeInteractionMode::EllipseAnnotation) {
                            NativeInteractionShapeAnnotationKind::Ellipse
                        } else {
                            NativeInteractionShapeAnnotationKind::Rect
                        },
                        color: state.rect_annotation_color_hex.clone(),
                        stroke_width: (f64::from(state.rect_annotation_stroke_width_physical)
                            / f64::from(session.scale_factor.max(0.0001)))
                            .max(1.0),
                        start: NativeInteractionSelectionPoint {
                            x: logical.x,
                            y: logical.y,
                        },
                        end: NativeInteractionSelectionPoint {
                            x: logical.x + logical.width,
                            y: logical.y + logical.height,
                        },
                    },
                ));
            }
        }
    }
    apply_cursor_for_shared_state(&mut state);
    let snapshot = snapshot_from_shared(&state);
    let state_event = emit_state_updated_from_shared(&mut state, true);
    let session_id = session.session_id.clone();
    drop(state);

    unsafe {
        ReleaseCapture();
    }
    let _ = present_interaction_surface(hwnd, shared)?;
    if let Some(event) = state_event {
        emit_backend_event(event_sink.clone(), event);
    }
    if let Some(event) = backend_event {
        emit_backend_event(event_sink, event);
    }
    log::info!(
        target: "bexo::service::native_interaction",
        "native_interaction_drag_committed session_id={} drag_mode={} selection={} revision={} mode={} total_ms={} present_samples={} avg_present_ms={} max_present_ms={}",
        session_id,
        drag_mode.map(NativeInteractionDragMode::as_str).unwrap_or("none"),
        format_selection(snapshot.selection),
        snapshot.selection_revision,
        snapshot.interaction_mode.as_str(),
        drag_elapsed_ms,
        drag_present_samples,
        drag_present_avg_ms,
        drag_present_max_ms
    );
    Ok(())
}

fn present_interaction_surface(
    hwnd: HWND,
    shared: &Arc<Mutex<InteractionWindowSharedState>>,
) -> AppResult<InteractionPresentMetrics> {
    let started_at = Instant::now();
    let mut state = shared.lock().map_err(|_| {
        AppError::new(
            "NATIVE_INTERACTION_STATE_LOCK_FAILED",
            "读取 Native Interaction 共享状态失败",
        )
    })?;
    let Some(session) = state.session.clone() else {
        return Ok(InteractionPresentMetrics {
            copy_ms: 0,
            update_ms: 0,
            total_ms: 0,
            surface_recreated: false,
        });
    };
    ensure_pixel_buffer(&mut state, session.physical_width, session.physical_height)?;
    let surface_recreated =
        ensure_layered_surface(&mut state, session.physical_width, session.physical_height)?;
    render_selection_overlay(&mut state, &session);
    let pixel_buffer_ptr = state.pixel_buffer.as_ptr();
    let pixel_buffer_len = state.pixel_buffer.len();
    let pixel_buffer = unsafe { std::slice::from_raw_parts(pixel_buffer_ptr, pixel_buffer_len) };
    let metrics = update_layered_window_surface(
        hwnd,
        session.physical_x,
        session.physical_y,
        session.physical_width,
        session.physical_height,
        pixel_buffer,
        state.layered_surface.as_mut().ok_or_else(|| {
            AppError::new(
                "NATIVE_INTERACTION_SURFACE_UNAVAILABLE",
                "Native Interaction GDI surface 不可用",
            )
        })?,
    )?;
    if state.drag_mode.is_some() {
        state.drag_present_samples = state.drag_present_samples.saturating_add(1);
        state.drag_present_total_ms = state.drag_present_total_ms.saturating_add(metrics.total_ms);
        state.drag_present_max_ms = state.drag_present_max_ms.max(metrics.total_ms);
    }
    Ok(InteractionPresentMetrics {
        copy_ms: metrics.copy_ms,
        update_ms: metrics.update_ms,
        total_ms: started_at.elapsed().as_millis(),
        surface_recreated,
    })
}

fn render_selection_overlay(
    state: &mut InteractionWindowSharedState,
    session: &InteractionWindowSession,
) {
    let width = usize::try_from(session.physical_width).unwrap_or(0);
    let height = usize::try_from(session.physical_height).unwrap_or(0);
    state.pixel_buffer.copy_from_slice(&state.base_mask_buffer);

    if let Some(selection) = state.selection_physical {
        let left = selection.x.max(0.0).floor() as i32;
        let top = selection.y.max(0.0).floor() as i32;
        let right = (selection.x + selection.width)
            .min(f64::from(session.physical_width))
            .ceil() as i32;
        let bottom = (selection.y + selection.height)
            .min(f64::from(session.physical_height))
            .ceil() as i32;

        for row in top.max(0) as usize..bottom.max(0) as usize {
            if row >= height {
                break;
            }
            let row_start = row * width * 4;
            for column in left.max(0) as usize..right.max(0) as usize {
                if column >= width {
                    break;
                }
                let offset = row_start + column * 4;
                state.pixel_buffer[offset] = 0;
                state.pixel_buffer[offset + 1] = 0;
                state.pixel_buffer[offset + 2] = 0;
                state.pixel_buffer[offset + 3] = SELECTION_HOLE_ALPHA;
            }
        }

        let border = logical_to_physical(BORDER_THICKNESS_LOGICAL, session.scale_factor).max(1);
        fill_rect(
            &mut state.pixel_buffer,
            width,
            height,
            left,
            top,
            (right - left).max(1),
            border,
            BORDER_COLOR,
        );
        fill_rect(
            &mut state.pixel_buffer,
            width,
            height,
            left,
            (bottom - border).max(top),
            (right - left).max(1),
            border,
            BORDER_COLOR,
        );
        fill_rect(
            &mut state.pixel_buffer,
            width,
            height,
            left,
            top,
            border,
            (bottom - top).max(1),
            BORDER_COLOR,
        );
        fill_rect(
            &mut state.pixel_buffer,
            width,
            height,
            (right - border).max(left),
            top,
            border,
            (bottom - top).max(1),
            BORDER_COLOR,
        );

        for handle_rect in handle_rects(selection, session.scale_factor) {
            fill_rect(
                &mut state.pixel_buffer,
                width,
                height,
                handle_rect.x.floor() as i32,
                handle_rect.y.floor() as i32,
                handle_rect.width.ceil() as i32,
                handle_rect.height.ceil() as i32,
                HANDLE_COLOR,
            );
        }
    }

    if let Some(rect_draft) = state.rect_annotation_draft_physical {
        let fill_color = [
            state.rect_annotation_color[0],
            state.rect_annotation_color[1],
            state.rect_annotation_color[2],
            RECT_ANNOTATION_FILL_ALPHA,
        ];
        if matches!(state.interaction_mode, NativeInteractionMode::EllipseAnnotation) {
            fill_ellipse(
                &mut state.pixel_buffer,
                width,
                height,
                rect_draft,
                fill_color,
            );
            stroke_ellipse(
                &mut state.pixel_buffer,
                width,
                height,
                rect_draft,
                state.rect_annotation_stroke_width_physical.max(1),
                state.rect_annotation_color,
            );
        } else {
            fill_rect(
                &mut state.pixel_buffer,
                width,
                height,
                rect_draft.x.floor() as i32,
                rect_draft.y.floor() as i32,
                rect_draft.width.ceil() as i32,
                rect_draft.height.ceil() as i32,
                fill_color,
            );
            let border = state.rect_annotation_stroke_width_physical.max(1);
            let left = rect_draft.x.floor() as i32;
            let top = rect_draft.y.floor() as i32;
            let right = (rect_draft.x + rect_draft.width).ceil() as i32;
            let bottom = (rect_draft.y + rect_draft.height).ceil() as i32;
            fill_rect(
                &mut state.pixel_buffer,
                width,
                height,
                left,
                top,
                (right - left).max(1),
                border,
                state.rect_annotation_color,
            );
            fill_rect(
                &mut state.pixel_buffer,
                width,
                height,
                left,
                (bottom - border).max(top),
                (right - left).max(1),
                border,
                state.rect_annotation_color,
            );
            fill_rect(
                &mut state.pixel_buffer,
                width,
                height,
                left,
                top,
                border,
                (bottom - top).max(1),
                state.rect_annotation_color,
            );
            fill_rect(
                &mut state.pixel_buffer,
                width,
                height,
                (right - border).max(left),
                top,
                border,
                (bottom - top).max(1),
                state.rect_annotation_color,
            );
        }
    }

    for exclusion in &state.exclusion_rects_physical {
        clear_rect_transparent(
            &mut state.pixel_buffer,
            width,
            height,
            exclusion.x.floor() as i32,
            exclusion.y.floor() as i32,
            exclusion.width.ceil() as i32,
            exclusion.height.ceil() as i32,
        );
    }
}

fn update_layered_window_surface(
    hwnd: HWND,
    window_x: i32,
    window_y: i32,
    window_width: u32,
    window_height: u32,
    buffer: &[u8],
    surface: &mut LayeredWindowSurface,
) -> AppResult<InteractionPresentMetrics> {
    let expected_len = usize::try_from(window_width)
        .ok()
        .and_then(|value| value.checked_mul(usize::try_from(window_height).ok()?))
        .and_then(|value| value.checked_mul(4))
        .ok_or_else(|| {
            AppError::new(
                "NATIVE_INTERACTION_BUFFER_INVALID",
                "Native Interaction 缓冲区尺寸溢出",
            )
        })?;
    if buffer.len() != expected_len {
        return Err(AppError::new(
            "NATIVE_INTERACTION_BUFFER_INVALID",
            "Native Interaction 缓冲区尺寸不匹配",
        )
        .with_detail("expected", expected_len.to_string())
        .with_detail("actual", buffer.len().to_string()));
    }

    let copy_started_at = Instant::now();
    unsafe {
        copy_nonoverlapping(buffer.as_ptr(), surface.bitmap_bits, buffer.len());
    }
    let copy_ms = copy_started_at.elapsed().as_millis();

    let update_started_at = Instant::now();
    unsafe {
        let dst_point = POINT {
            x: window_x,
            y: window_y,
        };
        let size = SIZE {
            cx: window_width as i32,
            cy: window_height as i32,
        };
        let src_point = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };

        let updated = UpdateLayeredWindow(
            hwnd,
            surface.screen_dc,
            &dst_point,
            &size,
            surface.mem_dc,
            &src_point,
            0,
            &blend,
            ULW_ALPHA,
        );
        if updated == 0 {
            return Err(last_error(
                "NATIVE_INTERACTION_UPDATE_LAYERED_WINDOW_FAILED",
                "更新 Native Interaction Layered Window 失败",
            ));
        }
    }

    Ok(InteractionPresentMetrics {
        copy_ms,
        update_ms: update_started_at.elapsed().as_millis(),
        total_ms: copy_started_at.elapsed().as_millis(),
        surface_recreated: false,
    })
}

fn create_native_interaction_window() -> AppResult<HWND> {
    let module = unsafe { GetModuleHandleW(null()) };
    if module == 0 {
        return Err(last_error(
            "NATIVE_INTERACTION_MODULE_HANDLE_FAILED",
            "读取 Native Interaction 模块句柄失败",
        ));
    }
    let cursor = unsafe { LoadCursorW(0, IDC_ARROW) };
    if cursor == 0 {
        return Err(last_error(
            "NATIVE_INTERACTION_CURSOR_LOAD_FAILED",
            "加载 Native Interaction 光标失败",
        ));
    }

    let class = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(native_interaction_window_proc),
        hInstance: module,
        hCursor: cursor,
        lpszClassName: WINDOW_CLASS_NAME.as_ptr(),
        ..unsafe { zeroed() }
    };
    if unsafe { RegisterClassExW(&class) } == 0 {
        return Err(last_error(
            "NATIVE_INTERACTION_REGISTER_CLASS_FAILED",
            "注册 Native Interaction 窗口类失败",
        ));
    }

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            WINDOW_CLASS_NAME.as_ptr(),
            WINDOW_TITLE.as_ptr(),
            WS_POPUP,
            0,
            0,
            INITIAL_WINDOW_WIDTH,
            INITIAL_WINDOW_HEIGHT,
            0,
            0,
            module,
            null(),
        )
    };
    if hwnd == 0 {
        return Err(last_error(
            "NATIVE_INTERACTION_WINDOW_CREATE_FAILED",
            "创建 Native Interaction 窗口失败",
        ));
    }
    Ok(hwnd)
}

fn resolve_physical_window_geometry(
    session: &NativeInteractionSessionSpec,
) -> AppResult<(i32, i32, u32, u32)> {
    if !(session.scale_factor.is_finite() && session.scale_factor > 0.0) {
        return Err(AppError::validation("Native Interaction 缩放因子无效"));
    }
    let scale = f64::from(session.scale_factor);
    Ok((
        (f64::from(session.display_x) * scale).round() as i32,
        (f64::from(session.display_y) * scale).round() as i32,
        (f64::from(session.display_width.max(1)) * scale)
            .round()
            .max(1.0) as u32,
        (f64::from(session.display_height.max(1)) * scale)
            .round()
            .max(1.0) as u32,
    ))
}

fn resize_window(
    hwnd: HWND,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    visible: bool,
) -> AppResult<()> {
    let flags = if visible {
        SWP_NOACTIVATE
    } else {
        SWP_HIDEWINDOW | SWP_NOACTIVATE
    };
    let result =
        unsafe { SetWindowPos(hwnd, HWND_TOPMOST, x, y, width as i32, height as i32, flags) };
    if result == 0 {
        return Err(last_error(
            "NATIVE_INTERACTION_WINDOW_RESIZE_FAILED",
            "调整 Native Interaction 窗口尺寸失败",
        ));
    }
    Ok(())
}

fn ensure_pixel_buffer(
    state: &mut InteractionWindowSharedState,
    width: u32,
    height: u32,
) -> AppResult<()> {
    let expected = usize::try_from(width)
        .ok()
        .and_then(|value| value.checked_mul(usize::try_from(height).ok()?))
        .and_then(|value| value.checked_mul(4))
        .ok_or_else(|| {
            AppError::new(
                "NATIVE_INTERACTION_BUFFER_INVALID",
                "Native Interaction 缓冲区尺寸溢出",
            )
        })?;
    if state.pixel_buffer.len() != expected {
        state.pixel_buffer = vec![0; expected];
    }
    if state.base_mask_buffer.len() != expected {
        state.base_mask_buffer = build_base_mask_buffer(expected);
    }
    Ok(())
}

fn build_base_mask_buffer(expected: usize) -> Vec<u8> {
    let mut buffer = vec![0; expected];
    for pixel in buffer.chunks_exact_mut(4) {
        pixel[0] = 0;
        pixel[1] = 0;
        pixel[2] = 0;
        pixel[3] = MASK_ALPHA;
    }
    buffer
}

fn ensure_layered_surface(
    state: &mut InteractionWindowSharedState,
    width: u32,
    height: u32,
) -> AppResult<bool> {
    let needs_recreate = match state.layered_surface.as_ref() {
        Some(surface) => !surface.matches_size(width, height),
        None => true,
    };
    if !needs_recreate {
        return Ok(false);
    }

    state.layered_surface = Some(LayeredWindowSurface::create(width, height)?);
    Ok(true)
}

fn snapshot_from_shared(
    state: &InteractionWindowSharedState,
) -> NativeInteractionSelectionSnapshot {
    let selection = state.selection_physical.and_then(|rect| {
        state
            .session
            .as_ref()
            .map(|session| physical_to_logical(rect, session))
    });
    let rect_draft = state.rect_annotation_draft_physical.and_then(|rect| {
        state
            .session
            .as_ref()
            .map(|session| physical_to_logical(rect, session))
    });
    NativeInteractionSelectionSnapshot {
        selection,
        hovered_hit_region: state.hovered_hit_region,
        drag_mode: state.drag_mode,
        selection_revision: state.selection_revision,
        interaction_mode: state.interaction_mode,
        rect_draft,
    }
}

fn build_state_updated_event_from_shared(
    state: &InteractionWindowSharedState,
) -> Option<crate::services::NativeInteractionStateUpdatedEvent> {
    let snapshot = snapshot_from_shared(state);
    Some(crate::services::NativeInteractionStateUpdatedEvent {
        session_id: state.session.as_ref().map(|session| session.session_id.clone()),
        backend_kind: Some(crate::services::NativeInteractionBackendKind::WindowsLayeredSelectionMvp),
        lifecycle_state: "visible".to_string(),
        has_active_session: state.session.is_some(),
        selection: snapshot.selection,
        hovered_hit_region: snapshot.hovered_hit_region.as_str().to_string(),
        drag_mode: snapshot.drag_mode.map(|mode| mode.as_str().to_string()),
        selection_revision: snapshot.selection_revision,
        interaction_mode: snapshot.interaction_mode.as_str().to_string(),
        rect_draft: snapshot.rect_draft,
    })
}

fn emit_state_updated_from_shared(
    state: &mut InteractionWindowSharedState,
    force: bool,
) -> Option<NativeInteractionBackendEvent> {
    let event = build_state_updated_event_from_shared(state)?;
    if !force {
        if let Some(last) = state.last_state_event_ms {
            if last.elapsed().as_millis() < NATIVE_INTERACTION_EVENT_THROTTLE_MS {
                return None;
            }
        }
    }
    state.last_state_event_ms = Some(Instant::now());
    Some(NativeInteractionBackendEvent::StateUpdated(event))
}

fn emit_backend_event(event_sink: Option<NativeInteractionEventSink>, event: NativeInteractionBackendEvent) {
    if let Some(sink) = event_sink {
        sink(event);
    }
}

fn physical_to_logical(
    rect: PhysicalRect,
    session: &InteractionWindowSession,
) -> NativeInteractionSelectionRect {
    let scale = f64::from(session.scale_factor).max(0.0001);
    NativeInteractionSelectionRect {
        x: rect.x / scale,
        y: rect.y / scale,
        width: rect.width / scale,
        height: rect.height / scale,
    }
}

fn logical_selection_to_physical(
    rect: NativeInteractionSelectionRect,
    session: &InteractionWindowSession,
) -> Option<PhysicalRect> {
    if !(rect.width.is_finite()
        && rect.height.is_finite()
        && rect.x.is_finite()
        && rect.y.is_finite()
        && rect.width > 0.0
        && rect.height > 0.0)
    {
        return None;
    }
    let scale = f64::from(session.scale_factor).max(0.0001);
    let x = (rect.x * scale)
        .floor()
        .clamp(0.0, f64::from(session.physical_width));
    let y = (rect.y * scale)
        .floor()
        .clamp(0.0, f64::from(session.physical_height));
    let right = ((rect.x + rect.width) * scale)
        .ceil()
        .clamp(0.0, f64::from(session.physical_width));
    let bottom = ((rect.y + rect.height) * scale)
        .ceil()
        .clamp(0.0, f64::from(session.physical_height));
    let width = (right - x).max(0.0);
    let height = (bottom - y).max(0.0);
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    Some(PhysicalRect { x, y, width, height })
}

fn logical_to_physical_rect(
    rect: NativeInteractionExclusionRect,
    session: &InteractionWindowSession,
) -> Option<PhysicalRect> {
    if !(rect.width.is_finite()
        && rect.height.is_finite()
        && rect.x.is_finite()
        && rect.y.is_finite()
        && rect.width > 0.0
        && rect.height > 0.0)
    {
        return None;
    }

    let scale = f64::from(session.scale_factor).max(0.0001);
    let x = (rect.x * scale)
        .floor()
        .clamp(0.0, f64::from(session.physical_width));
    let y = (rect.y * scale)
        .floor()
        .clamp(0.0, f64::from(session.physical_height));
    let right = ((rect.x + rect.width) * scale)
        .ceil()
        .clamp(0.0, f64::from(session.physical_width));
    let bottom = ((rect.y + rect.height) * scale)
        .ceil()
        .clamp(0.0, f64::from(session.physical_height));
    let width = (right - x).max(0.0);
    let height = (bottom - y).max(0.0);
    if width <= 0.0 || height <= 0.0 {
        return None;
    }

    Some(PhysicalRect {
        x,
        y,
        width,
        height,
    })
}

fn hit_test_selection(
    state: &InteractionWindowSharedState,
    x: i32,
    y: i32,
) -> NativeInteractionHitRegion {
    let Some(session) = state.session.as_ref() else {
        return NativeInteractionHitRegion::None;
    };
    let Some(selection) = state.selection_physical else {
        return NativeInteractionHitRegion::None;
    };
    for (handle, rect) in handle_hit_boxes(selection, session.scale_factor) {
        if point_in_box(
            x,
            y,
            rect.x.floor() as i32,
            rect.y.floor() as i32,
            rect.width.ceil() as i32,
            rect.height.ceil() as i32,
        ) {
            return NativeInteractionHitRegion::Handle(handle);
        }
    }
    if point_in_box(
        x,
        y,
        selection.x.floor() as i32,
        selection.y.floor() as i32,
        selection.width.ceil() as i32,
        selection.height.ceil() as i32,
    ) {
        return NativeInteractionHitRegion::SelectionBody;
    }
    NativeInteractionHitRegion::None
}

fn clamp_rect_to_bounds(rect: PhysicalRect, bounds: PhysicalRect) -> PhysicalRect {
    let left = rect.x.max(bounds.x);
    let top = rect.y.max(bounds.y);
    let right = (rect.x + rect.width).min(bounds.x + bounds.width);
    let bottom = (rect.y + rect.height).min(bounds.y + bounds.height);
    PhysicalRect {
        x: left.min(right),
        y: top.min(bottom),
        width: (right - left).max(1.0),
        height: (bottom - top).max(1.0),
    }
}

fn move_selection_rect(
    origin: PhysicalRect,
    delta_x: i32,
    delta_y: i32,
    session: &InteractionWindowSession,
) -> PhysicalRect {
    let max_x = (f64::from(session.physical_width) - origin.width).max(0.0);
    let max_y = (f64::from(session.physical_height) - origin.height).max(0.0);
    PhysicalRect {
        x: (origin.x + f64::from(delta_x)).clamp(0.0, max_x),
        y: (origin.y + f64::from(delta_y)).clamp(0.0, max_y),
        width: origin.width,
        height: origin.height,
    }
}

fn resize_selection_rect(
    origin: PhysicalRect,
    handle: NativeInteractionSelectionHandle,
    x: i32,
    y: i32,
    session: &InteractionWindowSession,
) -> PhysicalRect {
    let mut left = origin.x;
    let mut top = origin.y;
    let mut right = origin.x + origin.width;
    let mut bottom = origin.y + origin.height;
    let x = f64::from(x);
    let y = f64::from(y);
    let min_size =
        logical_to_physical(MIN_SELECTION_SIZE_LOGICAL, session.scale_factor).max(1) as f64;

    match handle {
        NativeInteractionSelectionHandle::Nw => {
            left = x;
            top = y;
        }
        NativeInteractionSelectionHandle::N => top = y,
        NativeInteractionSelectionHandle::Ne => {
            right = x;
            top = y;
        }
        NativeInteractionSelectionHandle::E => right = x,
        NativeInteractionSelectionHandle::Se => {
            right = x;
            bottom = y;
        }
        NativeInteractionSelectionHandle::S => bottom = y,
        NativeInteractionSelectionHandle::Sw => {
            left = x;
            bottom = y;
        }
        NativeInteractionSelectionHandle::W => left = x,
    }

    let max_width = f64::from(session.physical_width);
    let max_height = f64::from(session.physical_height);
    left = left.clamp(0.0, max_width);
    right = right.clamp(0.0, max_width);
    top = top.clamp(0.0, max_height);
    bottom = bottom.clamp(0.0, max_height);

    if right - left < min_size {
        match handle {
            NativeInteractionSelectionHandle::Nw
            | NativeInteractionSelectionHandle::W
            | NativeInteractionSelectionHandle::Sw => left = (right - min_size).max(0.0),
            _ => right = (left + min_size).min(max_width),
        }
    }
    if bottom - top < min_size {
        match handle {
            NativeInteractionSelectionHandle::Nw
            | NativeInteractionSelectionHandle::N
            | NativeInteractionSelectionHandle::Ne => top = (bottom - min_size).max(0.0),
            _ => bottom = (top + min_size).min(max_height),
        }
    }

    PhysicalRect {
        x: left,
        y: top,
        width: (right - left).max(min_size),
        height: (bottom - top).max(min_size),
    }
}

fn build_physical_rect(x0: i32, y0: i32, x1: i32, y1: i32) -> PhysicalRect {
    let left = i32::min(x0, x1);
    let top = i32::min(y0, y1);
    let right = i32::max(x0, x1);
    let bottom = i32::max(y0, y1);
    PhysicalRect {
        x: f64::from(left),
        y: f64::from(top),
        width: f64::from((right - left).max(1)),
        height: f64::from((bottom - top).max(1)),
    }
}

fn handle_hit_boxes(
    selection: PhysicalRect,
    scale_factor: f32,
) -> [(NativeInteractionSelectionHandle, PhysicalRect); 8] {
    let size = logical_to_physical(HANDLE_SIZE_LOGICAL, scale_factor).max(6) as f64;
    let half = size / 2.0;
    let left = selection.x;
    let top = selection.y;
    let right = selection.x + selection.width;
    let bottom = selection.y + selection.height;
    let center_x = left + selection.width / 2.0;
    let center_y = top + selection.height / 2.0;
    [
        (
            NativeInteractionSelectionHandle::Nw,
            PhysicalRect {
                x: left - half,
                y: top - half,
                width: size,
                height: size,
            },
        ),
        (
            NativeInteractionSelectionHandle::N,
            PhysicalRect {
                x: center_x - half,
                y: top - half,
                width: size,
                height: size,
            },
        ),
        (
            NativeInteractionSelectionHandle::Ne,
            PhysicalRect {
                x: right - half,
                y: top - half,
                width: size,
                height: size,
            },
        ),
        (
            NativeInteractionSelectionHandle::E,
            PhysicalRect {
                x: right - half,
                y: center_y - half,
                width: size,
                height: size,
            },
        ),
        (
            NativeInteractionSelectionHandle::Se,
            PhysicalRect {
                x: right - half,
                y: bottom - half,
                width: size,
                height: size,
            },
        ),
        (
            NativeInteractionSelectionHandle::S,
            PhysicalRect {
                x: center_x - half,
                y: bottom - half,
                width: size,
                height: size,
            },
        ),
        (
            NativeInteractionSelectionHandle::Sw,
            PhysicalRect {
                x: left - half,
                y: bottom - half,
                width: size,
                height: size,
            },
        ),
        (
            NativeInteractionSelectionHandle::W,
            PhysicalRect {
                x: left - half,
                y: center_y - half,
                width: size,
                height: size,
            },
        ),
    ]
}

fn handle_rects(selection: PhysicalRect, scale_factor: f32) -> [PhysicalRect; 8] {
    let handles = handle_hit_boxes(selection, scale_factor);
    [
        handles[0].1,
        handles[1].1,
        handles[2].1,
        handles[3].1,
        handles[4].1,
        handles[5].1,
        handles[6].1,
        handles[7].1,
    ]
}

fn clamp_point_to_bounds(x: i32, y: i32, session: &InteractionWindowSession) -> (i32, i32) {
    (
        x.clamp(0, session.physical_width.saturating_sub(1) as i32),
        y.clamp(0, session.physical_height.saturating_sub(1) as i32),
    )
}

fn parse_color_rgba(color_hex: Option<&str>) -> Option<[u8; 4]> {
    let value = color_hex?.trim().trim_start_matches('#');
    let [r, g, b] = match value.len() {
        6 => [
            u8::from_str_radix(&value[0..2], 16).ok()?,
            u8::from_str_radix(&value[2..4], 16).ok()?,
            u8::from_str_radix(&value[4..6], 16).ok()?,
        ],
        3 => {
            let expand = |slice: &str| u8::from_str_radix(&format!("{slice}{slice}"), 16).ok();
            [
                expand(&value[0..1])?,
                expand(&value[1..2])?,
                expand(&value[2..3])?,
            ]
        }
        _ => return None,
    };
    Some([b, g, r, 0xFF])
}

fn cursor_kind_for_state(state: &InteractionWindowSharedState) -> NativeInteractionCursorKind {
    if let Some(drag_mode) = state.drag_mode {
        return match drag_mode {
            NativeInteractionDragMode::Creating => NativeInteractionCursorKind::Crosshair,
            NativeInteractionDragMode::Moving => NativeInteractionCursorKind::Move,
            NativeInteractionDragMode::Resizing(handle) => cursor_kind_for_handle(handle),
            NativeInteractionDragMode::RectCreating
            | NativeInteractionDragMode::EllipseCreating => NativeInteractionCursorKind::Crosshair,
        };
    }

    match state.interaction_mode {
        NativeInteractionMode::Selection => match state.hovered_hit_region {
            NativeInteractionHitRegion::None => NativeInteractionCursorKind::Crosshair,
            NativeInteractionHitRegion::SelectionBody => NativeInteractionCursorKind::Move,
            NativeInteractionHitRegion::Handle(handle) => cursor_kind_for_handle(handle),
        },
        NativeInteractionMode::RectAnnotation | NativeInteractionMode::EllipseAnnotation => {
            NativeInteractionCursorKind::Crosshair
        }
    }
}

fn cursor_kind_for_handle(handle: NativeInteractionSelectionHandle) -> NativeInteractionCursorKind {
    match handle {
        NativeInteractionSelectionHandle::N | NativeInteractionSelectionHandle::S => {
            NativeInteractionCursorKind::ResizeNs
        }
        NativeInteractionSelectionHandle::E | NativeInteractionSelectionHandle::W => {
            NativeInteractionCursorKind::ResizeWe
        }
        NativeInteractionSelectionHandle::Nw | NativeInteractionSelectionHandle::Se => {
            NativeInteractionCursorKind::ResizeNwse
        }
        NativeInteractionSelectionHandle::Ne | NativeInteractionSelectionHandle::Sw => {
            NativeInteractionCursorKind::ResizeNesw
        }
    }
}

fn apply_cursor_for_shared_state(state: &mut InteractionWindowSharedState) {
    let next_kind = cursor_kind_for_state(state);
    if state.last_cursor_kind == next_kind {
        return;
    }
    unsafe {
        let cursor_id = match next_kind {
            NativeInteractionCursorKind::Crosshair => IDC_CROSS,
            NativeInteractionCursorKind::Move => IDC_SIZEALL,
            NativeInteractionCursorKind::ResizeNs => IDC_SIZENS,
            NativeInteractionCursorKind::ResizeWe => IDC_SIZEWE,
            NativeInteractionCursorKind::ResizeNwse => IDC_SIZENWSE,
            NativeInteractionCursorKind::ResizeNesw => IDC_SIZENESW,
        };
        let cursor = LoadCursorW(0, cursor_id);
        if cursor != 0 {
            SetCursor(cursor);
        }
    }
    state.last_cursor_kind = next_kind;
}

fn logical_to_physical(value: f64, scale_factor: f32) -> i32 {
    (value * f64::from(scale_factor).max(1.0)).round().max(1.0) as i32
}

fn mouse_point_from_lparam(l_param: LPARAM) -> (i32, i32) {
    let raw = l_param as u32;
    (
        (raw & 0xFFFF) as i16 as i32,
        ((raw >> 16) & 0xFFFF) as i16 as i32,
    )
}

fn point_in_box(x: i32, y: i32, left: i32, top: i32, width: i32, height: i32) -> bool {
    x >= left
        && x < left.saturating_add(width.max(0))
        && y >= top
        && y < top.saturating_add(height.max(0))
}

fn fill_rect(
    buffer: &mut [u8],
    width: usize,
    height: usize,
    left: i32,
    top: i32,
    rect_width: i32,
    rect_height: i32,
    color: [u8; 4],
) {
    let left = left.max(0) as usize;
    let top = top.max(0) as usize;
    let right = (left + usize::try_from(rect_width.max(0)).unwrap_or(0)).min(width);
    let bottom = (top + usize::try_from(rect_height.max(0)).unwrap_or(0)).min(height);
    for row in top..bottom {
        let row_start = row * width * 4;
        for column in left..right {
            let offset = row_start + column * 4;
            buffer[offset..offset + 4].copy_from_slice(&color);
        }
    }
}

fn clear_rect_transparent(
    buffer: &mut [u8],
    width: usize,
    height: usize,
    left: i32,
    top: i32,
    rect_width: i32,
    rect_height: i32,
) {
    let left = left.max(0) as usize;
    let top = top.max(0) as usize;
    let right = (left + usize::try_from(rect_width.max(0)).unwrap_or(0)).min(width);
    let bottom = (top + usize::try_from(rect_height.max(0)).unwrap_or(0)).min(height);
    for row in top..bottom {
        let row_start = row * width * 4;
        for column in left..right {
            let offset = row_start + column * 4;
            buffer[offset] = 0;
            buffer[offset + 1] = 0;
            buffer[offset + 2] = 0;
            buffer[offset + 3] = 0;
        }
    }
}

fn fill_ellipse(
    buffer: &mut [u8],
    width: usize,
    height: usize,
    bounds: PhysicalRect,
    color: [u8; 4],
) {
    let left = bounds.x.floor().max(0.0) as i32;
    let top = bounds.y.floor().max(0.0) as i32;
    let right = (bounds.x + bounds.width).ceil().min(width as f64) as i32;
    let bottom = (bounds.y + bounds.height).ceil().min(height as f64) as i32;
    let radius_x = bounds.width.max(1.0) / 2.0;
    let radius_y = bounds.height.max(1.0) / 2.0;
    let center_x = bounds.x + radius_x;
    let center_y = bounds.y + radius_y;

    for row in top.max(0) as usize..bottom.max(0) as usize {
        let y = row as f64 + 0.5;
        let normalized_y = ((y - center_y) / radius_y).powi(2);
        if normalized_y > 1.0 {
            continue;
        }
        let row_start = row * width * 4;
        for column in left.max(0) as usize..right.max(0) as usize {
            let x = column as f64 + 0.5;
            let normalized_x = ((x - center_x) / radius_x).powi(2);
            if normalized_x + normalized_y <= 1.0 {
                let offset = row_start + column * 4;
                buffer[offset..offset + 4].copy_from_slice(&color);
            }
        }
    }
}

fn stroke_ellipse(
    buffer: &mut [u8],
    width: usize,
    height: usize,
    bounds: PhysicalRect,
    stroke_width: i32,
    color: [u8; 4],
) {
    let outer_rx = bounds.width.max(1.0) / 2.0;
    let outer_ry = bounds.height.max(1.0) / 2.0;
    let inner_rx = (outer_rx - f64::from(stroke_width.max(1))).max(0.0);
    let inner_ry = (outer_ry - f64::from(stroke_width.max(1))).max(0.0);
    let center_x = bounds.x + outer_rx;
    let center_y = bounds.y + outer_ry;
    let left = bounds.x.floor().max(0.0) as i32;
    let top = bounds.y.floor().max(0.0) as i32;
    let right = (bounds.x + bounds.width).ceil().min(width as f64) as i32;
    let bottom = (bounds.y + bounds.height).ceil().min(height as f64) as i32;

    for row in top.max(0) as usize..bottom.max(0) as usize {
        let y = row as f64 + 0.5;
        let outer_y = ((y - center_y) / outer_ry).powi(2);
        let inner_y = if inner_ry > 0.0 {
            ((y - center_y) / inner_ry).powi(2)
        } else {
            f64::INFINITY
        };
        let row_start = row * width * 4;
        for column in left.max(0) as usize..right.max(0) as usize {
            let x = column as f64 + 0.5;
            let outer = ((x - center_x) / outer_rx).powi(2) + outer_y;
            if outer > 1.0 {
                continue;
            }
            let inner = if inner_rx > 0.0 && inner_ry > 0.0 {
                ((x - center_x) / inner_rx).powi(2) + inner_y
            } else {
                f64::INFINITY
            };
            if inner <= 1.0 {
                continue;
            }
            let offset = row_start + column * 4;
            buffer[offset..offset + 4].copy_from_slice(&color);
        }
    }
}

fn shared_state_from_hwnd(hwnd: HWND) -> Option<&'static Arc<Mutex<InteractionWindowSharedState>>> {
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) }
        as *const Arc<Mutex<InteractionWindowSharedState>>;
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { &*ptr })
    }
}

fn format_selection(selection: Option<NativeInteractionSelectionRect>) -> String {
    selection
        .map(|rect| {
            format!(
                "{:.1},{:.1},{:.1},{:.1}",
                rect.x, rect.y, rect.width, rect.height
            )
        })
        .unwrap_or_else(|| "none".to_string())
}

fn last_error(code: &str, message: &str) -> AppError {
    AppError::new(code, message).with_detail("win32Error", unsafe { GetLastError() }.to_string())
}
