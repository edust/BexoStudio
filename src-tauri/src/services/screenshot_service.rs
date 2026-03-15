use std::{
    borrow::Cow,
    fs,
    io::Cursor,
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

use arboard::{Clipboard, ImageData};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use chrono::{Local, Utc};
use screenshots::{
    display_info::DisplayInfo,
    image::{
        codecs::png::{
            CompressionType as PngCompressionType, FilterType as PngFilterType, PngEncoder,
        },
        imageops::{self, FilterType},
        ColorType, DynamicImage, ImageEncoder, ImageOutputFormat, Rgba, RgbaImage,
    },
    Screen,
};
use tauri::{
    window::Color, AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Position, Runtime,
    Size, WebviewUrl, WebviewWindowBuilder,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleBitmap, CreateCompatibleDC, CreateDCW, DeleteDC, DeleteObject, GetDIBits,
    GetMonitorInfoW, SelectObject, SetStretchBltMode, StretchBlt, BITMAPINFO, BITMAPINFOHEADER,
    COLORONCOLOR, DIB_RGB_COLORS, HBITMAP, HDC, HMONITOR, MONITORINFOEXW, SRCCOPY,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, GWL_STYLE, HWND_TOPMOST,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE, WS_CAPTION,
    WS_EX_CLIENTEDGE, WS_EX_DLGMODALFRAME, WS_EX_STATICEDGE, WS_EX_TOOLWINDOW, WS_EX_WINDOWEDGE,
    WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU, WS_THICKFRAME,
};

use crate::{
    domain::{
        CancelScreenshotSessionResult, CopyScreenshotSelectionResult,
        SaveScreenshotSelectionResult, ScreenshotImageStatus, ScreenshotMonitorView,
        ScreenshotPreviewTransport, ScreenshotRenderedImageInput, ScreenshotSelectionInput,
        ScreenshotSelectionRenderMode, ScreenshotSelectionRenderTile,
        ScreenshotSelectionRenderView, ScreenshotSessionUpdatedEvent, ScreenshotSessionView,
        StartScreenshotSessionResult, SCREENSHOT_OVERLAY_WINDOW_LABEL,
        SCREENSHOT_SESSION_UPDATED_EVENT_NAME,
    },
    error::{AppError, AppResult},
    services::{desktop_duplication_capture, wgc_capture},
};

const SCREENSHOT_OVERLAY_URL: &str = "index.html?overlay=screenshot";
const DPI_SCALE_EPSILON: f64 = 0.05;
const PREVIEW_RESIZE_FILTER: FilterType = FilterType::Nearest;
const PREVIEW_TEMP_DIR_NAME: &str = "bexo-screenshot-preview";
const OVERLAY_STABILIZE_MAX_ATTEMPTS: usize = 8;
const OVERLAY_STABILIZE_INTERVAL_MS: u64 = 12;
const BMP_FILE_HEADER_SIZE: usize = 14;
const BMP_INFO_HEADER_SIZE: usize = 40;
const BMP_PIXEL_OFFSET: usize = BMP_FILE_HEADER_SIZE + BMP_INFO_HEADER_SIZE;
const LIVE_CAPTURE_MIN_INTERVAL_MS: u64 = 80;
const LIVE_CAPTURE_MAX_FRAME_AGE_MS: u64 = 250;
const LIVE_CAPTURE_LOG_EVERY_N_FRAMES: u64 = 30;
const LIVE_CAPTURE_WAIT_FOR_READY_FRAME_MS: u64 = 180;
const LIVE_CAPTURE_WAIT_POLL_INTERVAL_MS: u64 = 10;
const OVERLAY_EVENT_SUPPRESS_MS: u64 = 250;
const OVERLAY_TRANSPARENT_BG: Color = Color(0, 0, 0, 0);

#[derive(Clone)]
pub struct ScreenshotService {
    state: Arc<Mutex<ScreenshotState>>,
    live_capture: Arc<Mutex<LiveCaptureState>>,
}

#[derive(Debug, Default)]
struct ScreenshotState {
    active_session: Option<ActiveScreenshotSession>,
    overlay_prewarmed: bool,
    overlay_event_suppressed_until: Option<Instant>,
    overlay_focus_drift_compensation: Option<OverlayFocusDriftCompensation>,
}

#[derive(Default)]
struct LiveCaptureState {
    latest_snapshot: Option<Arc<LiveCaptureSnapshot>>,
    worker: Option<LiveCaptureWorker>,
}

struct LiveCaptureWorker {
    _handle: desktop_duplication_capture::DesktopDuplicationLiveCaptureHandle,
}

#[derive(Debug, Clone, Copy)]
enum LiveCaptureBackend {
    DesktopDuplication,
}

impl LiveCaptureBackend {
    fn as_str(self) -> &'static str {
        match self {
            Self::DesktopDuplication => "desktop_duplication",
        }
    }

    fn capture_strategy(self) -> &'static str {
        match self {
            Self::DesktopDuplication => "desktop_duplication_live_cache",
        }
    }
}

#[derive(Debug, Clone)]
struct LiveCaptureSnapshot {
    backend: LiveCaptureBackend,
    sequence: u64,
    captured_at: Instant,
    display_id: u32,
    monitor_handle: isize,
    display_x: i32,
    display_y: i32,
    display_width: u32,
    display_height: u32,
    capture_width: u32,
    capture_height: u32,
    scale_factor: f32,
    preview_pixel_width: u32,
    preview_pixel_height: u32,
    bgra_top_down: Arc<Vec<u8>>,
    preview_protocol_bytes: Option<Arc<Vec<u8>>>,
}

#[derive(Debug, Clone)]
struct ActiveScreenshotSession {
    id: String,
    created_at: String,
    display_x: i32,
    display_y: i32,
    display_width: u32,
    display_height: u32,
    scale_factor: f32,
    capture_width: u32,
    capture_height: u32,
    image_status: ScreenshotImageStatus,
    image_error: Option<String>,
    image_data_url: Arc<String>,
    preview_image_path: Option<Arc<String>>,
    preview_protocol_bytes: Option<Arc<Vec<u8>>>,
    preview_transport: ScreenshotPreviewTransport,
    preview_pixel_width: u32,
    preview_pixel_height: u32,
    monitors: Arc<Vec<CapturedMonitorFrame>>,
}

#[derive(Debug)]
struct PreparedPreviewImage {
    image_data_url: String,
    preview_image_path: Option<String>,
    encoded_bytes: usize,
    width: u32,
    height: u32,
    preview_mode: &'static str,
    encode_path: &'static str,
    encoded_format: &'static str,
}

#[derive(Debug, Clone)]
struct CapturedMonitorFrame {
    display_id: u32,
    display_x: i32,
    display_y: i32,
    relative_x: u32,
    relative_y: u32,
    display_width: u32,
    display_height: u32,
    capture_width: u32,
    capture_height: u32,
    scale_factor: f32,
    bgra_top_down: Option<Arc<Vec<u8>>>,
    rgba_image: Arc<OnceLock<RgbaImage>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MonitorDisplayCoordinateSpace {
    TauriLogical,
    Logical,
    PhysicalConverted,
}

impl MonitorDisplayCoordinateSpace {
    fn as_str(self) -> &'static str {
        match self {
            Self::TauriLogical => "tauri_logical",
            Self::Logical => "logical",
            Self::PhysicalConverted => "physical_converted",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct TauriMonitorSnapshot {
    physical_x: i32,
    physical_y: i32,
    physical_width: u32,
    physical_height: u32,
    logical_x: i32,
    logical_y: i32,
    logical_width: u32,
    logical_height: u32,
    scale_factor: f64,
}

#[derive(Debug, Clone, Copy)]
struct NormalizedMonitorDisplay {
    display_x: i32,
    display_y: i32,
    display_width: u32,
    display_height: u32,
    coordinate_space: MonitorDisplayCoordinateSpace,
    reported_scale_factor: f32,
    measured_scale_x: f64,
    measured_scale_y: f64,
}

#[derive(Debug, Clone, Copy)]
struct VirtualDesktopInfo {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone)]
struct CapturedVirtualDesktop {
    desktop: VirtualDesktopInfo,
    monitors: Vec<CapturedMonitorFrame>,
}

#[derive(Debug)]
struct FastPreviewCapture {
    captured: CapturedVirtualDesktop,
    preview_protocol_bytes: Vec<u8>,
    preview_pixel_width: u32,
    preview_pixel_height: u32,
    capture_strategy: &'static str,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, Copy)]
struct LiveCaptureInitContext {
    display_id: u32,
    monitor_handle: isize,
    display_x: i32,
    display_y: i32,
    display_width: u32,
    display_height: u32,
    scale_factor: f32,
    preview_pixel_width: u32,
    preview_pixel_height: u32,
}

#[derive(Debug, Clone, Copy)]
struct LogicalSelection {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Copy)]
struct MonitorSelectionIntersection {
    monitor_index: usize,
    logical_x: u32,
    logical_y: u32,
    logical_width: u32,
    logical_height: u32,
}

#[derive(Debug)]
struct SelectionRenderData {
    mode: ScreenshotSelectionRenderMode,
    scale_factor: f32,
    image: RgbaImage,
    tiles: Vec<ScreenshotSelectionRenderTile>,
}

#[derive(Debug, Clone, Copy)]
struct OverlayGeometryProbe {
    scale_factor: f64,
    current_x: i32,
    current_y: i32,
    current_width: u32,
    current_height: u32,
    delta_x: i32,
    delta_y: i32,
    delta_width: i32,
    delta_height: i32,
    outer_x: i32,
    outer_y: i32,
    outer_width: u32,
    outer_height: u32,
}

#[derive(Debug, Clone, Copy)]
struct OverlayFocusDriftCompensation {
    display_width: u32,
    display_height: u32,
    scale_factor_millis: u32,
    offset_x: i32,
    offset_y: i32,
}

#[derive(Debug, Clone, Copy)]
struct OverlayActivationOutcome {
    observed_focus_drift: Option<OverlayGeometryProbe>,
}

impl OverlayGeometryProbe {
    fn is_position_aligned(&self) -> bool {
        self.delta_x == 0 && self.delta_y == 0
    }

    fn is_size_aligned(&self) -> bool {
        self.delta_width.abs() <= 1 && self.delta_height.abs() <= 1
    }

    fn is_aligned(&self) -> bool {
        self.is_position_aligned() && self.is_size_aligned()
    }
}

impl OverlayFocusDriftCompensation {
    fn for_session(session: &ActiveScreenshotSession, offset_x: i32, offset_y: i32) -> Self {
        Self {
            display_width: session.display_width,
            display_height: session.display_height,
            scale_factor_millis: scale_factor_to_millis(session.scale_factor),
            offset_x,
            offset_y,
        }
    }

    fn matches_session(&self, session: &ActiveScreenshotSession) -> bool {
        self.display_width == session.display_width
            && self.display_height == session.display_height
            && self.scale_factor_millis == scale_factor_to_millis(session.scale_factor)
    }
}

fn scale_factor_to_millis(scale_factor: f32) -> u32 {
    (f64::from(scale_factor) * 1000.0).round().max(1.0) as u32
}

impl CapturedMonitorFrame {
    fn scale_x(&self) -> f64 {
        if self.display_width == 0 {
            1.0
        } else {
            f64::from(self.capture_width) / f64::from(self.display_width)
        }
    }

    fn scale_y(&self) -> f64 {
        if self.display_height == 0 {
            1.0
        } else {
            f64::from(self.capture_height) / f64::from(self.display_height)
        }
    }

    fn source_image(&self) -> AppResult<&RgbaImage> {
        if let Some(image) = self.rgba_image.get() {
            return Ok(image);
        }

        let Some(bgra_top_down) = self.bgra_top_down.as_ref() else {
            return Err(AppError::new(
                "SCREENSHOT_CAPTURE_FAILED",
                "截图原始像素不存在",
            ));
        };

        let rgba_image = rgba_image_from_top_down_bgra(
            self.capture_width,
            self.capture_height,
            bgra_top_down.as_slice(),
        )?;
        let _ = self.rgba_image.set(rgba_image);
        self.rgba_image
            .get()
            .ok_or_else(|| AppError::new("SCREENSHOT_CAPTURE_FAILED", "初始化截图图像缓存失败"))
    }

    fn to_view(&self) -> ScreenshotMonitorView {
        ScreenshotMonitorView {
            display_id: self.display_id,
            display_x: self.display_x,
            display_y: self.display_y,
            relative_x: self.relative_x,
            relative_y: self.relative_y,
            display_width: self.display_width,
            display_height: self.display_height,
            capture_width: self.capture_width,
            capture_height: self.capture_height,
            scale_factor: self.scale_factor,
        }
    }
}

fn rgba_image_cell(image: RgbaImage) -> Arc<OnceLock<RgbaImage>> {
    let cell = OnceLock::new();
    let _ = cell.set(image);
    Arc::new(cell)
}

fn decode_rendered_png_to_rgba(input: &ScreenshotRenderedImageInput) -> AppResult<RgbaImage> {
    let bytes = decode_rendered_png_bytes(input)?;
    decode_png_to_rgba(bytes.as_slice())
}

fn decode_png_to_rgba(bytes: &[u8]) -> AppResult<RgbaImage> {
    screenshots::image::load_from_memory(bytes)
        .map(|image| image.to_rgba8())
        .map_err(|error| {
            AppError::new("SCREENSHOT_RENDERED_IMAGE_INVALID", "标注结果图像无效")
                .with_detail("reason", error.to_string())
        })
}

fn decode_rendered_png_bytes(input: &ScreenshotRenderedImageInput) -> AppResult<Vec<u8>> {
    let raw = input.data_url.trim();
    if raw.is_empty() {
        return Err(AppError::validation("标注结果图像不能为空"));
    }

    let payload = if raw.starts_with("data:") {
        let (meta, data) = raw.split_once(',').ok_or_else(|| {
            AppError::new("SCREENSHOT_RENDERED_IMAGE_INVALID", "标注结果图像格式无效")
        })?;
        let normalized_meta = meta.to_ascii_lowercase();
        if !normalized_meta.contains("image/png") || !normalized_meta.contains(";base64") {
            return Err(AppError::new(
                "SCREENSHOT_RENDERED_IMAGE_INVALID",
                "标注结果图像必须是 PNG Base64",
            ));
        }
        data
    } else {
        raw
    };

    BASE64_STANDARD.decode(payload).map_err(|error| {
        AppError::new("SCREENSHOT_RENDERED_IMAGE_INVALID", "标注结果图像解码失败")
            .with_detail("reason", error.to_string())
    })
}

fn hide_overlay_if_visible<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window(SCREENSHOT_OVERLAY_WINDOW_LABEL) else {
        return;
    };

    match window.is_visible() {
        Ok(true) => {
            if let Err(error) = window.hide() {
                log::warn!(
                    target: "bexo::service::screenshot",
                    "hide overlay window before capture failed: {}",
                    error
                );
            } else {
                std::thread::sleep(Duration::from_millis(80));
            }
        }
        Ok(false) => {}
        Err(error) => {
            log::warn!(
                target: "bexo::service::screenshot",
                "query overlay window visibility failed: {}",
                error
            );
        }
    }
}

fn capture_virtual_desktop<R: Runtime>(app: &AppHandle<R>) -> AppResult<CapturedVirtualDesktop> {
    let enumerate_started_at = Instant::now();
    let screens = Screen::all().map_err(|error| {
        AppError::new("SCREENSHOT_SCREEN_ENUM_FAILED", "读取显示器信息失败")
            .with_detail("reason", error.to_string())
    })?;
    let enumerate_ms = enumerate_started_at.elapsed().as_millis();
    let tauri_monitors = collect_tauri_monitor_snapshots(app);

    if screens.is_empty() {
        return Err(AppError::new(
            "SCREENSHOT_SCREEN_NOT_FOUND",
            "未找到可用显示器",
        ));
    }

    let mut monitors: Vec<CapturedMonitorFrame> = Vec::with_capacity(screens.len());
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;

    for (monitor_index, screen) in screens.into_iter().enumerate() {
        let monitor_started_at = Instant::now();
        let display_info = screen.display_info;
        let raw_display_x = display_info.x;
        let raw_display_y = display_info.y;
        let raw_display_width = display_info.width.max(1);
        let raw_display_height = display_info.height.max(1);
        let capture_started_at = Instant::now();
        let image = screen.capture().map_err(|error| {
            AppError::new("SCREENSHOT_CAPTURE_FAILED", "屏幕截图失败")
                .with_detail("reason", error.to_string())
                .with_detail("displayId", display_info.id.to_string())
        })?;
        let capture_ms = capture_started_at.elapsed().as_millis();
        let capture_width = image.width().max(1);
        let capture_height = image.height().max(1);

        let tauri_monitor = match_tauri_monitor_snapshot(
            raw_display_x,
            raw_display_y,
            raw_display_width,
            raw_display_height,
            capture_width,
            capture_height,
            tauri_monitors.as_slice(),
        );
        let normalized_display = normalize_monitor_display(
            raw_display_x,
            raw_display_y,
            raw_display_width,
            raw_display_height,
            capture_width,
            capture_height,
            display_info.scale_factor,
            tauri_monitor,
        );

        log::info!(
            target: "bexo::service::screenshot",
            "monitor_display_normalized display_id={} raw_display={}x{}@{},{} capture={}x{} reported_scale_factor={} measured_scale_x={:.4} measured_scale_y={:.4} coordinate_space={} normalized_display={}x{}@{},{}",
            display_info.id,
            raw_display_width,
            raw_display_height,
            raw_display_x,
            raw_display_y,
            capture_width,
            capture_height,
            normalized_display.reported_scale_factor,
            normalized_display.measured_scale_x,
            normalized_display.measured_scale_y,
            normalized_display.coordinate_space.as_str(),
            normalized_display.display_width,
            normalized_display.display_height,
            normalized_display.display_x,
            normalized_display.display_y
        );
        log::info!(
            target: "bexo::service::screenshot",
            "monitor_capture_profile index={} display_id={} capture_ms={} total_monitor_ms={} capture_pixels={} rgba_bytes={}",
            monitor_index,
            display_info.id,
            capture_ms,
            monitor_started_at.elapsed().as_millis(),
            u64::from(capture_width) * u64::from(capture_height),
            u64::from(capture_width) * u64::from(capture_height) * 4
        );

        let right = normalized_display.display_x + normalized_display.display_width as i32;
        let bottom = normalized_display.display_y + normalized_display.display_height as i32;
        min_x = min_x.min(normalized_display.display_x);
        min_y = min_y.min(normalized_display.display_y);
        max_x = max_x.max(right);
        max_y = max_y.max(bottom);

        monitors.push(CapturedMonitorFrame {
            display_id: display_info.id,
            display_x: normalized_display.display_x,
            display_y: normalized_display.display_y,
            relative_x: 0,
            relative_y: 0,
            display_width: normalized_display.display_width,
            display_height: normalized_display.display_height,
            capture_width,
            capture_height,
            scale_factor: resolve_monitor_scale_factor(
                normalized_display.display_width,
                capture_width,
                normalized_display.display_height,
                capture_height,
                normalized_display.reported_scale_factor,
            ),
            bgra_top_down: None,
            rgba_image: rgba_image_cell(image),
        });
    }

    if max_x <= min_x || max_y <= min_y {
        return Err(AppError::new(
            "SCREENSHOT_SCREEN_LAYOUT_INVALID",
            "显示器布局异常，无法创建截图会话",
        ));
    }

    for monitor in &mut monitors {
        monitor.relative_x = (monitor.display_x - min_x) as u32;
        monitor.relative_y = (monitor.display_y - min_y) as u32;
    }

    let desktop = VirtualDesktopInfo {
        x: min_x,
        y: min_y,
        width: (max_x - min_x) as u32,
        height: (max_y - min_y) as u32,
    };
    log::info!(
        target: "bexo::service::screenshot",
        "capture_virtual_desktop_completed enumerate_ms={} monitors={} desktop={}x{}@{},{} total_ms={}",
        enumerate_ms,
        monitors.len(),
        desktop.width,
        desktop.height,
        desktop.x,
        desktop.y,
        enumerate_started_at.elapsed().as_millis()
    );

    Ok(CapturedVirtualDesktop { desktop, monitors })
}

#[cfg(target_os = "windows")]
fn capture_single_monitor_fast_preview<R: Runtime>(
    app: &AppHandle<R>,
) -> AppResult<Option<FastPreviewCapture>> {
    let started_at = Instant::now();
    let screens = Screen::all().map_err(|error| {
        AppError::new("SCREENSHOT_SCREEN_ENUM_FAILED", "读取显示器信息失败")
            .with_detail("reason", error.to_string())
    })?;
    if screens.len() != 1 {
        return Ok(None);
    }

    let tauri_monitors = collect_tauri_monitor_snapshots(app);
    if tauri_monitors.len() != 1 {
        return Ok(None);
    }

    let screen = screens
        .into_iter()
        .next()
        .ok_or_else(|| AppError::new("SCREENSHOT_SCREEN_NOT_FOUND", "未找到可用显示器"))?;
    let display_info = screen.display_info;
    let tauri_monitor = tauri_monitors[0];

    let raw_capture_started_at = Instant::now();
    let (raw_image, preview_protocol_bytes, capture_strategy) =
        match capture_monitor_raw_and_preview_wgc_windows(
            &display_info,
            tauri_monitor.logical_width,
            tauri_monitor.logical_height,
        ) {
            Ok((raw_image, preview_protocol_bytes)) => {
                log::info!(
                    target: "bexo::service::screenshot",
                    "wgc_capture_attempted display_id={} result=success logical_preview={}x{} elapsed_ms={}",
                    display_info.id,
                    tauri_monitor.logical_width,
                    tauri_monitor.logical_height,
                    raw_capture_started_at.elapsed().as_millis()
                );
                (raw_image, preview_protocol_bytes, "wgc_single_monitor")
            }
            Err(error) => {
                log::warn!(
                    target: "bexo::service::screenshot",
                    "wgc_capture_failed display_id={} reason={} fallback=gdi_single_monitor_dual_capture",
                    display_info.id,
                    error
                );
                let (raw_image, preview_protocol_bytes) = capture_monitor_raw_and_preview_windows(
                    &display_info,
                    tauri_monitor.physical_width,
                    tauri_monitor.physical_height,
                    tauri_monitor.logical_width,
                    tauri_monitor.logical_height,
                )?;
                (
                    raw_image,
                    preview_protocol_bytes,
                    "gdi_single_monitor_dual_capture",
                )
            }
        };
    let raw_capture_ms = raw_capture_started_at.elapsed().as_millis();

    let normalized_display = NormalizedMonitorDisplay {
        display_x: tauri_monitor.logical_x,
        display_y: tauri_monitor.logical_y,
        display_width: tauri_monitor.logical_width,
        display_height: tauri_monitor.logical_height,
        coordinate_space: MonitorDisplayCoordinateSpace::TauriLogical,
        reported_scale_factor: tauri_monitor.scale_factor as f32,
        measured_scale_x: f64::from(tauri_monitor.physical_width)
            / f64::from(tauri_monitor.logical_width.max(1)),
        measured_scale_y: f64::from(tauri_monitor.physical_height)
            / f64::from(tauri_monitor.logical_height.max(1)),
    };

    let monitor = CapturedMonitorFrame {
        display_id: display_info.id,
        display_x: normalized_display.display_x,
        display_y: normalized_display.display_y,
        relative_x: 0,
        relative_y: 0,
        display_width: normalized_display.display_width,
        display_height: normalized_display.display_height,
        capture_width: raw_image.width().max(1),
        capture_height: raw_image.height().max(1),
        scale_factor: resolve_monitor_scale_factor(
            normalized_display.display_width,
            raw_image.width().max(1),
            normalized_display.display_height,
            raw_image.height().max(1),
            normalized_display.reported_scale_factor,
        ),
        bgra_top_down: None,
        rgba_image: rgba_image_cell(raw_image),
    };
    let desktop = VirtualDesktopInfo {
        x: normalized_display.display_x,
        y: normalized_display.display_y,
        width: normalized_display.display_width,
        height: normalized_display.display_height,
    };

    log::info!(
        target: "bexo::service::screenshot",
        "fast_preview_capture_completed display_id={} raw_capture_ms={} total_ms={} raw_pixels={}x{} preview_pixels={}x{} preview_bytes={} coordinate_space={}",
        display_info.id,
        raw_capture_ms,
        started_at.elapsed().as_millis(),
        monitor.capture_width,
        monitor.capture_height,
        tauri_monitor.logical_width,
        tauri_monitor.logical_height,
        preview_protocol_bytes.len(),
        normalized_display.coordinate_space.as_str()
    );

    Ok(Some(FastPreviewCapture {
        captured: CapturedVirtualDesktop {
            desktop,
            monitors: vec![monitor],
        },
        preview_protocol_bytes,
        preview_pixel_width: tauri_monitor.logical_width.max(1),
        preview_pixel_height: tauri_monitor.logical_height.max(1),
        capture_strategy,
    }))
}

#[cfg(not(target_os = "windows"))]
fn capture_single_monitor_fast_preview<R: Runtime>(
    _app: &AppHandle<R>,
) -> AppResult<Option<FastPreviewCapture>> {
    Ok(None)
}

#[cfg(target_os = "windows")]
fn capture_monitor_raw_and_preview_wgc_windows(
    display_info: &DisplayInfo,
    preview_width: u32,
    preview_height: u32,
) -> AppResult<(RgbaImage, Vec<u8>)> {
    let capture_started_at = Instant::now();
    let capture = wgc_capture::capture_monitor_frame(display_info.raw_handle.0 as isize)?;
    let raw_image = rgba_image_from_top_down_bgra(
        capture.width,
        capture.height,
        capture.bgra_top_down.as_slice(),
    )?;
    let preview_protocol_bytes = build_preview_bmp_from_top_down_bgra_windows(
        display_info,
        capture.width,
        capture.height,
        capture.bgra_top_down.as_slice(),
        preview_width.max(1),
        preview_height.max(1),
    )?;

    log::info!(
        target: "bexo::service::screenshot",
        "wgc_capture_completed display_id={} device_create_ms={} item_create_ms={} session_create_ms={} frame_wait_ms={} map_ms={} total_ms={} build_total_ms={} raw_pixels={}x{} preview_pixels={}x{} preview_bytes={}",
        display_info.id,
        capture.device_create_ms,
        capture.item_create_ms,
        capture.session_create_ms,
        capture.frame_wait_ms,
        capture.map_ms,
        capture.total_ms,
        capture_started_at.elapsed().as_millis(),
        capture.width,
        capture.height,
        preview_width.max(1),
        preview_height.max(1),
        preview_protocol_bytes.len()
    );

    Ok((raw_image, preview_protocol_bytes))
}

#[cfg(target_os = "windows")]
fn build_preview_bmp_from_top_down_bgra_windows(
    display_info: &DisplayInfo,
    raw_width: u32,
    raw_height: u32,
    raw_bgra_top_down: &[u8],
    preview_width: u32,
    preview_height: u32,
) -> AppResult<Vec<u8>> {
    build_preview_bmp_from_monitor_handle_windows(
        display_info.id,
        display_info.raw_handle.0 as isize,
        raw_width,
        raw_height,
        raw_bgra_top_down,
        preview_width,
        preview_height,
    )
}

#[cfg(target_os = "windows")]
fn build_preview_bmp_from_monitor_handle_windows(
    display_id: u32,
    monitor_handle: isize,
    raw_width: u32,
    raw_height: u32,
    raw_bgra_top_down: &[u8],
    preview_width: u32,
    preview_height: u32,
) -> AppResult<Vec<u8>> {
    let source_width = i32::try_from(raw_width).map_err(|error| {
        AppError::new("SCREENSHOT_CAPTURE_FAILED", "原始截图宽度无效")
            .with_detail("reason", error.to_string())
    })?;
    let source_height = i32::try_from(raw_height).map_err(|error| {
        AppError::new("SCREENSHOT_CAPTURE_FAILED", "原始截图高度无效")
            .with_detail("reason", error.to_string())
    })?;
    let preview_width_i32 = i32::try_from(preview_width).map_err(|error| {
        AppError::new("SCREENSHOT_CAPTURE_FAILED", "预览截图宽度无效")
            .with_detail("reason", error.to_string())
    })?;
    let preview_height_i32 = i32::try_from(preview_height).map_err(|error| {
        AppError::new("SCREENSHOT_CAPTURE_FAILED", "预览截图高度无效")
            .with_detail("reason", error.to_string())
    })?;

    let expected_len = usize::try_from(raw_width)
        .ok()
        .and_then(|width| {
            usize::try_from(raw_height)
                .ok()
                .and_then(move |height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| AppError::new("SCREENSHOT_CAPTURE_FAILED", "原始截图缓冲区溢出"))?;
    if raw_bgra_top_down.len() != expected_len {
        return Err(
            AppError::new("SCREENSHOT_CAPTURE_FAILED", "原始截图缓冲区长度异常")
                .with_detail("expected", expected_len.to_string())
                .with_detail("actual", raw_bgra_top_down.len().to_string()),
        );
    }

    let mut monitor_info: MONITORINFOEXW = unsafe { std::mem::zeroed() };
    monitor_info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    let monitor_handle = monitor_handle as HMONITOR;
    let monitor_info_ptr = &mut monitor_info as *mut MONITORINFOEXW as *mut _;
    let monitor_info_ok = unsafe { GetMonitorInfoW(monitor_handle, monitor_info_ptr) };
    if monitor_info_ok == 0 {
        return Err(
            AppError::new("SCREENSHOT_CAPTURE_FAILED", "读取显示器信息失败")
                .with_detail("displayId", display_id.to_string()),
        );
    }

    let mut source_dc: HDC = 0;
    let mut preview_dc: HDC = 0;
    let mut preview_bitmap: HBITMAP = 0;

    let result = (|| -> AppResult<Vec<u8>> {
        source_dc = unsafe {
            CreateDCW(
                monitor_info.szDevice.as_ptr(),
                monitor_info.szDevice.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
            )
        };
        if source_dc == 0 {
            return Err(
                AppError::new("SCREENSHOT_CAPTURE_FAILED", "创建设备上下文失败")
                    .with_detail("displayId", display_id.to_string()),
            );
        }

        preview_dc = unsafe { CreateCompatibleDC(source_dc) };
        if preview_dc == 0 {
            return Err(
                AppError::new("SCREENSHOT_CAPTURE_FAILED", "创建预览截图上下文失败")
                    .with_detail("displayId", display_id.to_string()),
            );
        }

        preview_bitmap =
            unsafe { CreateCompatibleBitmap(source_dc, preview_width_i32, preview_height_i32) };
        if preview_bitmap == 0 {
            return Err(
                AppError::new("SCREENSHOT_CAPTURE_FAILED", "创建预览截图位图失败")
                    .with_detail("displayId", display_id.to_string()),
            );
        }

        let preview_select = unsafe { SelectObject(preview_dc, preview_bitmap as _) };
        if preview_select == 0 || preview_select == -1 {
            return Err(
                AppError::new("SCREENSHOT_CAPTURE_FAILED", "绑定预览截图位图失败")
                    .with_detail("displayId", display_id.to_string()),
            );
        }

        unsafe {
            SetStretchBltMode(preview_dc, COLORONCOLOR);
        }

        let bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: source_width,
                biHeight: -source_height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: 0,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [unsafe { std::mem::zeroed() }; 1],
        };

        let blt_result = unsafe {
            windows_sys::Win32::Graphics::Gdi::StretchDIBits(
                preview_dc,
                0,
                0,
                preview_width_i32,
                preview_height_i32,
                0,
                0,
                source_width,
                source_height,
                raw_bgra_top_down.as_ptr() as *const _,
                &bitmap_info,
                DIB_RGB_COLORS,
                SRCCOPY,
            )
        };
        if blt_result == 0 {
            return Err(
                AppError::new("SCREENSHOT_CAPTURE_FAILED", "缩放预览截图失败")
                    .with_detail("displayId", display_id.to_string()),
            );
        }

        let preview_bgra = capture_bitmap_bgra_bottom_up_windows(
            preview_dc,
            preview_bitmap,
            preview_width,
            preview_height,
        )?;
        encode_bmp_from_bottom_up_bgra(preview_width, preview_height, preview_bgra.as_slice())
    })();

    if preview_bitmap != 0 {
        unsafe {
            DeleteObject(preview_bitmap as _);
        }
    }
    if preview_dc != 0 {
        unsafe {
            DeleteDC(preview_dc);
        }
    }
    if source_dc != 0 {
        unsafe {
            DeleteDC(source_dc);
        }
    }

    result
}

fn build_captured_virtual_desktop_from_live_snapshot(
    snapshot: &LiveCaptureSnapshot,
) -> CapturedVirtualDesktop {
    CapturedVirtualDesktop {
        desktop: VirtualDesktopInfo {
            x: snapshot.display_x,
            y: snapshot.display_y,
            width: snapshot.display_width,
            height: snapshot.display_height,
        },
        monitors: vec![CapturedMonitorFrame {
            display_id: snapshot.display_id,
            display_x: snapshot.display_x,
            display_y: snapshot.display_y,
            relative_x: 0,
            relative_y: 0,
            display_width: snapshot.display_width,
            display_height: snapshot.display_height,
            capture_width: snapshot.capture_width,
            capture_height: snapshot.capture_height,
            scale_factor: snapshot.scale_factor,
            bgra_top_down: Some(snapshot.bgra_top_down.clone()),
            rgba_image: Arc::new(OnceLock::new()),
        }],
    }
}

#[cfg(target_os = "windows")]
fn capture_monitor_raw_and_preview_windows(
    display_info: &DisplayInfo,
    raw_width: u32,
    raw_height: u32,
    preview_width: u32,
    preview_height: u32,
) -> AppResult<(RgbaImage, Vec<u8>)> {
    let source_width = i32::try_from(raw_width).map_err(|error| {
        AppError::new("SCREENSHOT_CAPTURE_FAILED", "屏幕截图失败")
            .with_detail("reason", error.to_string())
    })?;
    let source_height = i32::try_from(raw_height).map_err(|error| {
        AppError::new("SCREENSHOT_CAPTURE_FAILED", "屏幕截图失败")
            .with_detail("reason", error.to_string())
    })?;
    let preview_width_i32 = i32::try_from(preview_width).map_err(|error| {
        AppError::new("SCREENSHOT_CAPTURE_FAILED", "屏幕截图失败")
            .with_detail("reason", error.to_string())
    })?;
    let preview_height_i32 = i32::try_from(preview_height).map_err(|error| {
        AppError::new("SCREENSHOT_CAPTURE_FAILED", "屏幕截图失败")
            .with_detail("reason", error.to_string())
    })?;

    let mut monitor_info: MONITORINFOEXW = unsafe { std::mem::zeroed() };
    monitor_info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    let monitor_handle = display_info.raw_handle.0 as HMONITOR;
    let monitor_info_ptr = &mut monitor_info as *mut MONITORINFOEXW as *mut _;
    let monitor_info_ok = unsafe { GetMonitorInfoW(monitor_handle, monitor_info_ptr) };
    if monitor_info_ok == 0 {
        return Err(
            AppError::new("SCREENSHOT_CAPTURE_FAILED", "读取显示器信息失败")
                .with_detail("displayId", display_info.id.to_string()),
        );
    }

    let mut source_dc: HDC = 0;
    let mut raw_dc: HDC = 0;
    let mut preview_dc: HDC = 0;
    let mut raw_bitmap: HBITMAP = 0;
    let mut preview_bitmap: HBITMAP = 0;

    let result = (|| -> AppResult<(RgbaImage, Vec<u8>)> {
        source_dc = unsafe {
            CreateDCW(
                monitor_info.szDevice.as_ptr(),
                monitor_info.szDevice.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
            )
        };
        if source_dc == 0 {
            return Err(
                AppError::new("SCREENSHOT_CAPTURE_FAILED", "创建设备上下文失败")
                    .with_detail("displayId", display_info.id.to_string()),
            );
        }

        raw_dc = unsafe { CreateCompatibleDC(source_dc) };
        if raw_dc == 0 {
            return Err(
                AppError::new("SCREENSHOT_CAPTURE_FAILED", "创建原始截图上下文失败")
                    .with_detail("displayId", display_info.id.to_string()),
            );
        }

        raw_bitmap = unsafe { CreateCompatibleBitmap(source_dc, source_width, source_height) };
        if raw_bitmap == 0 {
            return Err(
                AppError::new("SCREENSHOT_CAPTURE_FAILED", "创建原始截图位图失败")
                    .with_detail("displayId", display_info.id.to_string()),
            );
        }

        let raw_select = unsafe { SelectObject(raw_dc, raw_bitmap as _) };
        if raw_select == 0 || raw_select == -1 {
            return Err(
                AppError::new("SCREENSHOT_CAPTURE_FAILED", "绑定原始截图位图失败")
                    .with_detail("displayId", display_info.id.to_string()),
            );
        }

        let raw_blt_ok = unsafe {
            StretchBlt(
                raw_dc,
                0,
                0,
                source_width,
                source_height,
                source_dc,
                0,
                0,
                source_width,
                source_height,
                SRCCOPY,
            )
        };
        if raw_blt_ok == 0 {
            return Err(
                AppError::new("SCREENSHOT_CAPTURE_FAILED", "采集原始截图失败")
                    .with_detail("displayId", display_info.id.to_string()),
            );
        }

        preview_dc = unsafe { CreateCompatibleDC(source_dc) };
        if preview_dc == 0 {
            return Err(
                AppError::new("SCREENSHOT_CAPTURE_FAILED", "创建预览截图上下文失败")
                    .with_detail("displayId", display_info.id.to_string()),
            );
        }

        preview_bitmap =
            unsafe { CreateCompatibleBitmap(source_dc, preview_width_i32, preview_height_i32) };
        if preview_bitmap == 0 {
            return Err(
                AppError::new("SCREENSHOT_CAPTURE_FAILED", "创建预览截图位图失败")
                    .with_detail("displayId", display_info.id.to_string()),
            );
        }

        let preview_select = unsafe { SelectObject(preview_dc, preview_bitmap as _) };
        if preview_select == 0 || preview_select == -1 {
            return Err(
                AppError::new("SCREENSHOT_CAPTURE_FAILED", "绑定预览截图位图失败")
                    .with_detail("displayId", display_info.id.to_string()),
            );
        }

        unsafe {
            SetStretchBltMode(preview_dc, COLORONCOLOR);
        }
        let preview_blt_ok = unsafe {
            StretchBlt(
                preview_dc,
                0,
                0,
                preview_width_i32,
                preview_height_i32,
                raw_dc,
                0,
                0,
                source_width,
                source_height,
                SRCCOPY,
            )
        };
        if preview_blt_ok == 0 {
            return Err(
                AppError::new("SCREENSHOT_CAPTURE_FAILED", "生成预览截图失败")
                    .with_detail("displayId", display_info.id.to_string()),
            );
        }

        let raw_bgra =
            capture_bitmap_bgra_bottom_up_windows(raw_dc, raw_bitmap, raw_width, raw_height)?;
        let preview_bgra = capture_bitmap_bgra_bottom_up_windows(
            preview_dc,
            preview_bitmap,
            preview_width,
            preview_height,
        )?;
        let raw_image = rgba_image_from_bottom_up_bgra(raw_width, raw_height, raw_bgra.as_slice())?;
        let preview_protocol_bytes =
            encode_bmp_from_bottom_up_bgra(preview_width, preview_height, preview_bgra.as_slice())?;
        Ok((raw_image, preview_protocol_bytes))
    })();

    if preview_bitmap != 0 {
        unsafe {
            DeleteObject(preview_bitmap as _);
        }
    }
    if raw_bitmap != 0 {
        unsafe {
            DeleteObject(raw_bitmap as _);
        }
    }
    if preview_dc != 0 {
        unsafe {
            DeleteDC(preview_dc);
        }
    }
    if raw_dc != 0 {
        unsafe {
            DeleteDC(raw_dc);
        }
    }
    if source_dc != 0 {
        unsafe {
            DeleteDC(source_dc);
        }
    }

    result
}

#[cfg(target_os = "windows")]
fn capture_bitmap_bgra_bottom_up_windows(
    bitmap_dc: HDC,
    bitmap: HBITMAP,
    width: u32,
    height: u32,
) -> AppResult<Vec<u8>> {
    if width == 0 || height == 0 {
        return Err(AppError::new(
            "SCREENSHOT_CAPTURE_FAILED",
            "截图位图尺寸无效",
        ));
    }

    let total_bytes = usize::try_from(width)
        .ok()
        .and_then(|w| {
            usize::try_from(height)
                .ok()
                .and_then(move |h| w.checked_mul(h))
        })
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| AppError::new("SCREENSHOT_CAPTURE_FAILED", "截图位图缓冲区溢出"))?;
    let mut buffer = vec![0u8; total_bytes];

    let mut bitmap_info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: i32::try_from(width).map_err(|error| {
                AppError::new("SCREENSHOT_CAPTURE_FAILED", "截图位图尺寸无效")
                    .with_detail("reason", error.to_string())
            })?,
            biHeight: i32::try_from(height).map_err(|error| {
                AppError::new("SCREENSHOT_CAPTURE_FAILED", "截图位图尺寸无效")
                    .with_detail("reason", error.to_string())
            })?,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: 0,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        },
        bmiColors: [unsafe { std::mem::zeroed() }; 1],
    };

    let result = unsafe {
        GetDIBits(
            bitmap_dc,
            bitmap,
            0,
            height,
            buffer.as_mut_ptr() as *mut _,
            &mut bitmap_info,
            DIB_RGB_COLORS,
        )
    };
    if result == 0 {
        return Err(AppError::new(
            "SCREENSHOT_CAPTURE_FAILED",
            "读取截图位图数据失败",
        ));
    }

    Ok(buffer)
}

#[cfg(target_os = "windows")]
fn rgba_image_from_top_down_bgra(
    width: u32,
    height: u32,
    top_down_bgra: &[u8],
) -> AppResult<RgbaImage> {
    let width_usize = usize::try_from(width).map_err(|error| {
        AppError::new("SCREENSHOT_CAPTURE_FAILED", "截图位图尺寸无效")
            .with_detail("reason", error.to_string())
    })?;
    let height_usize = usize::try_from(height).map_err(|error| {
        AppError::new("SCREENSHOT_CAPTURE_FAILED", "截图位图尺寸无效")
            .with_detail("reason", error.to_string())
    })?;
    let row_bytes = width_usize
        .checked_mul(4)
        .ok_or_else(|| AppError::new("SCREENSHOT_CAPTURE_FAILED", "截图位图行数据溢出"))?;
    let expected_len = row_bytes
        .checked_mul(height_usize)
        .ok_or_else(|| AppError::new("SCREENSHOT_CAPTURE_FAILED", "截图位图缓冲区溢出"))?;
    if top_down_bgra.len() != expected_len {
        return Err(
            AppError::new("SCREENSHOT_CAPTURE_FAILED", "截图位图缓冲区长度异常")
                .with_detail("expected", expected_len.to_string())
                .with_detail("actual", top_down_bgra.len().to_string()),
        );
    }

    let mut rgba = vec![0u8; expected_len];
    for row in 0..height_usize {
        let src_row = row * row_bytes;
        let dst_row = row * row_bytes;
        for column in 0..width_usize {
            let src_index = src_row + (column * 4);
            let dst_index = dst_row + (column * 4);
            rgba[dst_index] = top_down_bgra[src_index + 2];
            rgba[dst_index + 1] = top_down_bgra[src_index + 1];
            rgba[dst_index + 2] = top_down_bgra[src_index];
            rgba[dst_index + 3] = top_down_bgra[src_index + 3];
        }
    }

    RgbaImage::from_raw(width, height, rgba)
        .ok_or_else(|| AppError::new("SCREENSHOT_CAPTURE_FAILED", "从 WGC 像素缓冲区构建图像失败"))
}

#[cfg(target_os = "windows")]
fn rgba_image_from_bottom_up_bgra(
    width: u32,
    height: u32,
    bottom_up_bgra: &[u8],
) -> AppResult<RgbaImage> {
    let width_usize = usize::try_from(width).map_err(|error| {
        AppError::new("SCREENSHOT_CAPTURE_FAILED", "截图位图尺寸无效")
            .with_detail("reason", error.to_string())
    })?;
    let height_usize = usize::try_from(height).map_err(|error| {
        AppError::new("SCREENSHOT_CAPTURE_FAILED", "截图位图尺寸无效")
            .with_detail("reason", error.to_string())
    })?;
    let row_bytes = width_usize
        .checked_mul(4)
        .ok_or_else(|| AppError::new("SCREENSHOT_CAPTURE_FAILED", "截图位图行数据溢出"))?;
    let expected_len = row_bytes
        .checked_mul(height_usize)
        .ok_or_else(|| AppError::new("SCREENSHOT_CAPTURE_FAILED", "截图位图缓冲区溢出"))?;
    if bottom_up_bgra.len() != expected_len {
        return Err(
            AppError::new("SCREENSHOT_CAPTURE_FAILED", "截图位图缓冲区长度异常")
                .with_detail("expected", expected_len.to_string())
                .with_detail("actual", bottom_up_bgra.len().to_string()),
        );
    }

    let mut rgba = vec![0u8; expected_len];
    for y in 0..height_usize {
        let src_y = height_usize - 1 - y;
        let src_start = src_y
            .checked_mul(row_bytes)
            .ok_or_else(|| AppError::new("SCREENSHOT_CAPTURE_FAILED", "截图位图行偏移溢出"))?;
        let dst_start = y
            .checked_mul(row_bytes)
            .ok_or_else(|| AppError::new("SCREENSHOT_CAPTURE_FAILED", "截图位图行偏移溢出"))?;
        let src_row = &bottom_up_bgra[src_start..src_start + row_bytes];
        let dst_row = &mut rgba[dst_start..dst_start + row_bytes];
        for (src_pixel, dst_pixel) in src_row.chunks_exact(4).zip(dst_row.chunks_exact_mut(4)) {
            dst_pixel[0] = src_pixel[2];
            dst_pixel[1] = src_pixel[1];
            dst_pixel[2] = src_pixel[0];
            dst_pixel[3] = src_pixel[3];
        }
    }

    RgbaImage::from_raw(width, height, rgba)
        .ok_or_else(|| AppError::new("SCREENSHOT_CAPTURE_FAILED", "构建截图位图失败"))
}

fn collect_tauri_monitor_snapshots<R: Runtime>(app: &AppHandle<R>) -> Vec<TauriMonitorSnapshot> {
    match app.available_monitors() {
        Ok(monitors) => monitors
            .into_iter()
            .map(|monitor| {
                let physical_x = monitor.position().x;
                let physical_y = monitor.position().y;
                let physical_width = monitor.size().width.max(1);
                let physical_height = monitor.size().height.max(1);
                let scale_factor = monitor.scale_factor().max(1.0);

                TauriMonitorSnapshot {
                    physical_x,
                    physical_y,
                    physical_width,
                    physical_height,
                    logical_x: physical_offset_to_logical_with_scale(physical_x, scale_factor),
                    logical_y: physical_offset_to_logical_with_scale(physical_y, scale_factor),
                    logical_width: physical_length_to_logical_with_scale(
                        physical_width,
                        scale_factor,
                    ),
                    logical_height: physical_length_to_logical_with_scale(
                        physical_height,
                        scale_factor,
                    ),
                    scale_factor,
                }
            })
            .collect(),
        Err(error) => {
            log::warn!(
                target: "bexo::service::screenshot",
                "list tauri monitors failed, fallback to screenshots display info: {}",
                error
            );
            Vec::new()
        }
    }
}

#[cfg(target_os = "windows")]
fn collect_live_capture_init_context<R: Runtime>(
    app: &AppHandle<R>,
) -> AppResult<Option<LiveCaptureInitContext>> {
    let screens = Screen::all().map_err(|error| {
        AppError::new("SCREENSHOT_SCREEN_ENUM_FAILED", "读取显示器信息失败")
            .with_detail("reason", error.to_string())
    })?;
    if screens.len() != 1 {
        return Ok(None);
    }

    let tauri_monitors = collect_tauri_monitor_snapshots(app);
    if tauri_monitors.len() != 1 {
        return Ok(None);
    }

    let screen = screens
        .into_iter()
        .next()
        .ok_or_else(|| AppError::new("SCREENSHOT_SCREEN_NOT_FOUND", "未找到可用显示器"))?;
    let display_info = screen.display_info;
    let tauri_monitor = tauri_monitors[0];
    let normalized_display = normalize_monitor_display(
        display_info.x,
        display_info.y,
        display_info.width.max(1),
        display_info.height.max(1),
        tauri_monitor.physical_width.max(1),
        tauri_monitor.physical_height.max(1),
        display_info.scale_factor,
        Some(tauri_monitor),
    );

    Ok(Some(LiveCaptureInitContext {
        display_id: display_info.id,
        monitor_handle: display_info.raw_handle.0 as isize,
        display_x: normalized_display.display_x,
        display_y: normalized_display.display_y,
        display_width: normalized_display.display_width,
        display_height: normalized_display.display_height,
        scale_factor: resolve_monitor_scale_factor(
            normalized_display.display_width,
            tauri_monitor.physical_width.max(1),
            normalized_display.display_height,
            tauri_monitor.physical_height.max(1),
            normalized_display.reported_scale_factor,
        ),
        preview_pixel_width: tauri_monitor.logical_width.max(1),
        preview_pixel_height: tauri_monitor.logical_height.max(1),
    }))
}

fn match_tauri_monitor_snapshot(
    raw_display_x: i32,
    raw_display_y: i32,
    raw_display_width: u32,
    raw_display_height: u32,
    capture_width: u32,
    capture_height: u32,
    tauri_monitors: &[TauriMonitorSnapshot],
) -> Option<TauriMonitorSnapshot> {
    let mut best: Option<(i32, TauriMonitorSnapshot)> = None;

    for monitor in tauri_monitors {
        let mut score = 0_i32;

        if approx_u32_with_delta(capture_width, monitor.physical_width, 6)
            && approx_u32_with_delta(capture_height, monitor.physical_height, 6)
        {
            score += 90;
        }

        if approx_i32_with_delta(raw_display_x, monitor.physical_x, 6)
            && approx_i32_with_delta(raw_display_y, monitor.physical_y, 6)
            && approx_u32_with_delta(raw_display_width, monitor.physical_width, 6)
            && approx_u32_with_delta(raw_display_height, monitor.physical_height, 6)
        {
            score += 120;
        }

        if approx_i32_with_delta(raw_display_x, monitor.logical_x, 6)
            && approx_i32_with_delta(raw_display_y, monitor.logical_y, 6)
            && approx_u32_with_delta(raw_display_width, monitor.logical_width, 6)
            && approx_u32_with_delta(raw_display_height, monitor.logical_height, 6)
        {
            score += 100;
        }

        if score <= 0 {
            continue;
        }

        match best {
            Some((best_score, _)) if score <= best_score => {}
            _ => best = Some((score, *monitor)),
        }
    }

    best.map(|(_, monitor)| monitor)
}

fn normalize_monitor_display(
    raw_display_x: i32,
    raw_display_y: i32,
    raw_display_width: u32,
    raw_display_height: u32,
    capture_width: u32,
    capture_height: u32,
    reported_scale_factor: f32,
    tauri_monitor: Option<TauriMonitorSnapshot>,
) -> NormalizedMonitorDisplay {
    let measured_scale_x = if raw_display_width == 0 {
        1.0
    } else {
        f64::from(capture_width) / f64::from(raw_display_width)
    };
    let measured_scale_y = if raw_display_height == 0 {
        1.0
    } else {
        f64::from(capture_height) / f64::from(raw_display_height)
    };

    if let Some(monitor) = tauri_monitor {
        return NormalizedMonitorDisplay {
            display_x: monitor.logical_x,
            display_y: monitor.logical_y,
            display_width: monitor.logical_width,
            display_height: monitor.logical_height,
            coordinate_space: MonitorDisplayCoordinateSpace::TauriLogical,
            reported_scale_factor: monitor.scale_factor as f32,
            measured_scale_x,
            measured_scale_y,
        };
    }

    let reported = if reported_scale_factor.is_finite() && reported_scale_factor > 0.0 {
        reported_scale_factor
    } else {
        1.0
    };
    let measured_avg = (measured_scale_x + measured_scale_y) / 2.0;
    let reported_f64 = f64::from(reported);
    let reported_scaled = reported_f64 > 1.0 + DPI_SCALE_EPSILON;

    let looks_logical = approx_eq(measured_scale_x, reported_f64)
        && approx_eq(measured_scale_y, reported_f64)
        && approx_eq(measured_scale_x, measured_scale_y);
    let looks_physical =
        reported_scaled && approx_eq(measured_scale_x, 1.0) && approx_eq(measured_scale_y, 1.0);

    let should_convert_physical = if looks_physical {
        true
    } else if looks_logical {
        false
    } else {
        reported_scaled && measured_avg <= 1.0 + DPI_SCALE_EPSILON
    };

    if should_convert_physical {
        return NormalizedMonitorDisplay {
            display_x: physical_offset_to_logical(raw_display_x, reported),
            display_y: physical_offset_to_logical(raw_display_y, reported),
            display_width: physical_length_to_logical(raw_display_width, reported),
            display_height: physical_length_to_logical(raw_display_height, reported),
            coordinate_space: MonitorDisplayCoordinateSpace::PhysicalConverted,
            reported_scale_factor: reported,
            measured_scale_x,
            measured_scale_y,
        };
    }

    NormalizedMonitorDisplay {
        display_x: raw_display_x,
        display_y: raw_display_y,
        display_width: raw_display_width.max(1),
        display_height: raw_display_height.max(1),
        coordinate_space: MonitorDisplayCoordinateSpace::Logical,
        reported_scale_factor: reported,
        measured_scale_x,
        measured_scale_y,
    }
}

fn physical_length_to_logical(value: u32, scale_factor: f32) -> u32 {
    if scale_factor <= 1.0 {
        return value.max(1);
    }

    ((f64::from(value) / f64::from(scale_factor)).round() as u32).max(1)
}

fn physical_offset_to_logical(value: i32, scale_factor: f32) -> i32 {
    if scale_factor <= 1.0 {
        return value;
    }

    (f64::from(value) / f64::from(scale_factor)).round() as i32
}

fn physical_length_to_logical_with_scale(value: u32, scale_factor: f64) -> u32 {
    if scale_factor <= 1.0 {
        return value.max(1);
    }

    ((f64::from(value) / scale_factor).round() as u32).max(1)
}

fn physical_offset_to_logical_with_scale(value: i32, scale_factor: f64) -> i32 {
    if scale_factor <= 1.0 {
        return value;
    }

    (f64::from(value) / scale_factor).round() as i32
}

fn approx_i32_with_delta(left: i32, right: i32, delta: i32) -> bool {
    (i64::from(left) - i64::from(right)).abs() <= i64::from(delta.max(0))
}

fn approx_u32_with_delta(left: u32, right: u32, delta: u32) -> bool {
    left.abs_diff(right) <= delta
}

fn prepare_preview_image_data_url<R: Runtime>(
    app: &AppHandle<R>,
    session: &ActiveScreenshotSession,
) -> AppResult<PreparedPreviewImage> {
    let total_started_at = Instant::now();
    let prepare_started_at = Instant::now();
    let (image, preview_mode) = build_preview_image(session)?;
    let prepare_ms = prepare_started_at.elapsed().as_millis();

    let encode_started_at = Instant::now();
    let (encoded_bytes, encode_path, encoded_format) = match encode_preview_bmp_fast(&image) {
        Ok(bytes) => (bytes, "bmp_fast", "bmp"),
        Err(error) => {
            log::warn!(
                target: "bexo::service::screenshot",
                "preview_bmp_encode_failed session_id={} reason={}",
                session.id,
                error
            );
            match encode_preview_png_fast(&image) {
                Ok(bytes) => (bytes, "png_fast", "png"),
                Err(error) => {
                    log::warn!(
                        target: "bexo::service::screenshot",
                        "preview_fast_encode_failed session_id={} reason={}",
                        session.id,
                        error
                    );
                    (encode_png(&image)?, "png_fallback_default", "png")
                }
            }
        }
    };
    let encode_ms = encode_started_at.elapsed().as_millis();
    let encoded_bytes_len = encoded_bytes.len();

    let write_started_at = Instant::now();
    let preview_image_path =
        write_preview_bytes_to_temp_file(app, session.id.as_str(), encoded_format, &encoded_bytes)?;
    let write_ms = write_started_at.elapsed().as_millis();

    log::info!(
        target: "bexo::service::screenshot",
        "preview_image_ready session_id={} prepare_ms={} encode_ms={} write_ms={} base64_ms=0 total_ms={} width={} height={} encoded_bytes={} image_data_url_bytes=0 preview_image_path={} preview_mode={} resize_filter={:?} encode_path={} encoded_format={}",
        session.id,
        prepare_ms,
        encode_ms,
        write_ms,
        total_started_at.elapsed().as_millis(),
        image.width(),
        image.height(),
        encoded_bytes_len,
        preview_image_path,
        preview_mode,
        PREVIEW_RESIZE_FILTER,
        encode_path,
        encoded_format
    );

    Ok(PreparedPreviewImage {
        image_data_url: String::new(),
        preview_image_path: Some(preview_image_path),
        encoded_bytes: encoded_bytes_len,
        width: image.width(),
        height: image.height(),
        preview_mode,
        encode_path,
        encoded_format,
    })
}

fn write_preview_bytes_to_temp_file<R: Runtime>(
    app: &AppHandle<R>,
    session_id: &str,
    extension: &str,
    bytes: &[u8],
) -> AppResult<String> {
    let temp_dir = app.path().temp_dir().map_err(|error| {
        AppError::new(
            "SCREENSHOT_PREVIEW_PATH_UNAVAILABLE",
            "无法解析截图预览临时目录",
        )
        .with_detail("reason", error.to_string())
    })?;

    let preview_dir = temp_dir.join(PREVIEW_TEMP_DIR_NAME);
    fs::create_dir_all(&preview_dir).map_err(|error| {
        AppError::new(
            "SCREENSHOT_PREVIEW_DIR_CREATE_FAILED",
            "创建截图预览临时目录失败",
        )
        .with_detail("path", preview_dir.display().to_string())
        .with_detail("reason", error.to_string())
    })?;

    let sanitized_session_id = session_id
        .chars()
        .map(|value| {
            if value.is_ascii_alphanumeric() {
                value
            } else {
                '-'
            }
        })
        .collect::<String>();
    let file_name = format!(
        "{}-{}.{}",
        sanitized_session_id,
        Utc::now().format("%Y%m%d%H%M%S%3f"),
        extension
    );
    let file_path = preview_dir.join(file_name);
    fs::write(&file_path, bytes).map_err(|error| {
        AppError::new(
            "SCREENSHOT_PREVIEW_WRITE_FAILED",
            "写入截图预览临时文件失败",
        )
        .with_detail("path", file_path.display().to_string())
        .with_detail("reason", error.to_string())
    })?;

    Ok(file_path.display().to_string())
}

fn cleanup_preview_file(path: Option<&str>) {
    let Some(raw_path) = path else {
        return;
    };

    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return;
    }

    match fs::remove_file(trimmed) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            log::warn!(
                target: "bexo::service::screenshot",
                "cleanup screenshot preview file failed path={} reason={}",
                trimmed,
                error
            );
        }
    }
}

fn build_preview_image(session: &ActiveScreenshotSession) -> AppResult<(RgbaImage, &'static str)> {
    if let Some(scale_factor) = resolve_uniform_preview_scale(session.monitors.as_slice()) {
        if approx_eq(f64::from(scale_factor), 1.0) {
            let image = build_native_preview_image(session, scale_factor)?;
            return Ok((image, "native_uniform_scale"));
        }
    }

    Ok((
        build_logical_preview_image(session)?,
        "logical_display_fast",
    ))
}

fn resolve_preview_transport(
    monitors: &[CapturedMonitorFrame],
    display_width: u32,
    display_height: u32,
) -> (ScreenshotPreviewTransport, u32, u32) {
    if monitors.len() == 1 {
        let monitor = &monitors[0];
        if monitor.relative_x == 0
            && monitor.relative_y == 0
            && monitor.display_width == display_width
            && monitor.display_height == display_height
        {
            return (
                ScreenshotPreviewTransport::RawRgbaFast,
                monitor.capture_width.max(1),
                monitor.capture_height.max(1),
            );
        }
    }

    (
        ScreenshotPreviewTransport::File,
        display_width.max(1),
        display_height.max(1),
    )
}

fn build_logical_preview_image(session: &ActiveScreenshotSession) -> AppResult<RgbaImage> {
    if session.monitors.len() == 1 {
        let monitor = &session.monitors[0];
        if monitor.relative_x == 0
            && monitor.relative_y == 0
            && monitor.display_width == session.display_width
            && monitor.display_height == session.display_height
        {
            let source = monitor.source_image()?;
            if source.width() == monitor.display_width && source.height() == monitor.display_height
            {
                return Ok(source.clone());
            }
            return Ok(imageops::resize(
                source,
                monitor.display_width,
                monitor.display_height,
                PREVIEW_RESIZE_FILTER,
            ));
        }
    }

    let mut virtual_image = RgbaImage::from_pixel(
        session.display_width.max(1),
        session.display_height.max(1),
        Rgba([0, 0, 0, 255]),
    );

    for monitor in session.monitors.iter() {
        let source = monitor.source_image()?;
        if source.width() == monitor.display_width && source.height() == monitor.display_height {
            imageops::overlay(
                &mut virtual_image,
                source,
                i64::from(monitor.relative_x),
                i64::from(monitor.relative_y),
            );
        } else {
            let preview_image = imageops::resize(
                source,
                monitor.display_width,
                monitor.display_height,
                PREVIEW_RESIZE_FILTER,
            );
            imageops::overlay(
                &mut virtual_image,
                &preview_image,
                i64::from(monitor.relative_x),
                i64::from(monitor.relative_y),
            );
        }
    }

    Ok(virtual_image)
}

fn build_native_preview_image(
    session: &ActiveScreenshotSession,
    scale_factor: f32,
) -> AppResult<RgbaImage> {
    let virtual_width = compute_scaled_length(session.display_width.max(1), scale_factor);
    let virtual_height = compute_scaled_length(session.display_height.max(1), scale_factor);

    if session.monitors.len() == 1 {
        let monitor = &session.monitors[0];
        let expected_width = compute_scaled_length(monitor.display_width.max(1), scale_factor);
        let expected_height = compute_scaled_length(monitor.display_height.max(1), scale_factor);
        if monitor.relative_x == 0
            && monitor.relative_y == 0
            && expected_width == virtual_width
            && expected_height == virtual_height
        {
            let source = monitor.source_image()?;
            if source.width() == expected_width && source.height() == expected_height {
                return Ok(source.clone());
            }
            return Ok(imageops::resize(
                source,
                expected_width,
                expected_height,
                PREVIEW_RESIZE_FILTER,
            ));
        }
    }

    let mut virtual_image =
        RgbaImage::from_pixel(virtual_width, virtual_height, Rgba([0, 0, 0, 255]));

    for monitor in session.monitors.iter() {
        let source = monitor.source_image()?;
        let expected_width = compute_scaled_length(monitor.display_width.max(1), scale_factor);
        let expected_height = compute_scaled_length(monitor.display_height.max(1), scale_factor);
        let preview_image =
            if source.width() == expected_width && source.height() == expected_height {
                source.clone()
            } else {
                imageops::resize(
                    source,
                    expected_width,
                    expected_height,
                    PREVIEW_RESIZE_FILTER,
                )
            };

        imageops::overlay(
            &mut virtual_image,
            &preview_image,
            i64::from(compute_scaled_offset(monitor.relative_x, scale_factor)),
            i64::from(compute_scaled_offset(monitor.relative_y, scale_factor)),
        );
    }

    Ok(virtual_image)
}

fn build_overlay_prewarm_session<R: Runtime>(app: &AppHandle<R>) -> ActiveScreenshotSession {
    let monitor = collect_tauri_monitor_snapshots(app)
        .into_iter()
        .next()
        .unwrap_or(TauriMonitorSnapshot {
            physical_x: 0,
            physical_y: 0,
            physical_width: 1,
            physical_height: 1,
            logical_x: 0,
            logical_y: 0,
            logical_width: 1,
            logical_height: 1,
            scale_factor: 1.0,
        });

    ActiveScreenshotSession {
        id: "overlay-prewarm".to_string(),
        created_at: Utc::now().to_rfc3339(),
        display_x: monitor.logical_x,
        display_y: monitor.logical_y,
        display_width: monitor.logical_width.max(1),
        display_height: monitor.logical_height.max(1),
        scale_factor: monitor.scale_factor as f32,
        capture_width: monitor.physical_width.max(1),
        capture_height: monitor.physical_height.max(1),
        image_status: ScreenshotImageStatus::Loading,
        image_error: None,
        image_data_url: Arc::new(String::new()),
        preview_image_path: None,
        preview_protocol_bytes: None,
        preview_transport: ScreenshotPreviewTransport::File,
        preview_pixel_width: monitor.logical_width.max(1),
        preview_pixel_height: monitor.logical_height.max(1),
        monitors: Arc::new(Vec::new()),
    }
}

fn set_overlay_window_dormant_state<R: Runtime>(window: &tauri::WebviewWindow<R>) -> AppResult<()> {
    window.set_focusable(false).map_err(|error| {
        AppError::new(
            "SCREENSHOT_OVERLAY_DORMANT_FOCUS_FAILED",
            "禁用截图窗口焦点失败",
        )
        .with_detail("reason", error.to_string())
    })?;
    window.set_ignore_cursor_events(true).map_err(|error| {
        AppError::new(
            "SCREENSHOT_OVERLAY_DORMANT_CURSOR_FAILED",
            "启用截图窗口点击穿透失败",
        )
        .with_detail("reason", error.to_string())
    })?;
    Ok(())
}

fn set_overlay_window_interactive_state<R: Runtime>(
    window: &tauri::WebviewWindow<R>,
) -> AppResult<()> {
    window.set_focusable(true).map_err(|error| {
        AppError::new(
            "SCREENSHOT_OVERLAY_ACTIVE_FOCUS_FAILED",
            "启用截图窗口焦点失败",
        )
        .with_detail("reason", error.to_string())
    })?;
    window.set_ignore_cursor_events(false).map_err(|error| {
        AppError::new(
            "SCREENSHOT_OVERLAY_ACTIVE_CURSOR_FAILED",
            "禁用截图窗口点击穿透失败",
        )
        .with_detail("reason", error.to_string())
    })?;
    Ok(())
}

fn prewarm_overlay_window_once<R: Runtime>(
    window: &tauri::WebviewWindow<R>,
    session: &ActiveScreenshotSession,
) -> AppResult<()> {
    if let Err(error) = window.set_background_color(Some(OVERLAY_TRANSPARENT_BG)) {
        log::warn!(
            target: "bexo::service::screenshot",
            "set screenshot overlay background color during prewarm failed: {}",
            error
        );
    }
    set_overlay_window_geometry(window, session)?;
    lock_overlay_native_window_style(window)?;
    set_overlay_window_dormant_state(window)?;
    if window.is_visible().unwrap_or(false) {
        window.hide().map_err(|error| {
            AppError::new("SCREENSHOT_OVERLAY_PREWARM_FAILED", "隐藏预热截图窗口失败")
                .with_detail("reason", error.to_string())
        })?;
    }
    Ok(())
}

fn restore_overlay_window_hot_state<R: Runtime>(
    window: &tauri::WebviewWindow<R>,
    session: &ActiveScreenshotSession,
) -> AppResult<()> {
    set_overlay_window_dormant_state(window)?;
    let _ = session;
    if window.is_visible().unwrap_or(false) {
        window.hide().map_err(|error| {
            AppError::new("SCREENSHOT_OVERLAY_RESTORE_FAILED", "隐藏截图窗口失败")
                .with_detail("reason", error.to_string())
        })?;
    }
    Ok(())
}

fn prepare_overlay_window<R: Runtime>(
    app: &AppHandle<R>,
    session: &ActiveScreenshotSession,
) -> AppResult<tauri::WebviewWindow<R>> {
    if let Some(window) = app.get_webview_window(SCREENSHOT_OVERLAY_WINDOW_LABEL) {
        if let Err(error) = window.set_background_color(Some(OVERLAY_TRANSPARENT_BG)) {
            log::warn!(
                target: "bexo::service::screenshot",
                "set screenshot overlay background color during preparation failed: {}",
                error
            );
        }
        if let Err(error) = window.set_title("") {
            log::warn!(
                target: "bexo::service::screenshot",
                "set screenshot overlay title during preparation failed: {}",
                error
            );
        }
        if let Err(error) = window.set_decorations(false) {
            log::warn!(
                target: "bexo::service::screenshot",
                "set screenshot overlay decorations during preparation failed: {}",
                error
            );
        }
        if let Err(error) = window.set_resizable(false) {
            log::warn!(
                target: "bexo::service::screenshot",
                "set screenshot overlay resizable during preparation failed: {}",
                error
            );
        }
        lock_overlay_native_window_style(&window)?;
        return Ok(window);
    }

    let window = WebviewWindowBuilder::new(
        app,
        SCREENSHOT_OVERLAY_WINDOW_LABEL,
        WebviewUrl::App(SCREENSHOT_OVERLAY_URL.into()),
    )
    .title("")
    .inner_size(
        f64::from(session.display_width),
        f64::from(session.display_height),
    )
    .position(f64::from(session.display_x), f64::from(session.display_y))
    .decorations(false)
    .resizable(false)
    .transparent(true)
    .background_color(OVERLAY_TRANSPARENT_BG)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .focused(false)
    .maximizable(false)
    .minimizable(false)
    .shadow(false)
    .build()
    .map_err(|error| {
        AppError::new("SCREENSHOT_OVERLAY_CREATE_FAILED", "创建截图窗口失败")
            .with_detail("reason", error.to_string())
    })?;

    lock_overlay_native_window_style(&window)?;
    set_overlay_window_geometry(&window, session)?;
    Ok(window)
}

fn move_and_focus_overlay_window<R: Runtime>(
    window: &tauri::WebviewWindow<R>,
    session: &ActiveScreenshotSession,
    overlay_prewarmed: bool,
    focus_drift_compensation: Option<OverlayFocusDriftCompensation>,
) -> AppResult<OverlayActivationOutcome> {
    let activation_started_at = Instant::now();
    let was_visible = window.is_visible().unwrap_or(false);
    let hidden_prewarmed = overlay_prewarmed && !was_visible;
    let geometry_aligned = probe_overlay_geometry(window, session)
        .map(|probe| probe.is_aligned())
        .unwrap_or(false);
    let needs_geometry = !geometry_aligned;
    let focused_before = window.is_focused().unwrap_or(false);
    let focus_requested = !focused_before;
    let effective_focus_compensation =
        focus_drift_compensation.filter(|value| focus_requested && value.matches_session(session));
    let compensation_x = effective_focus_compensation
        .map(|value| value.offset_x)
        .unwrap_or(0);
    let compensation_y = effective_focus_compensation
        .map(|value| value.offset_y)
        .unwrap_or(0);

    let style_started_at = Instant::now();
    if let Err(error) = window.set_decorations(false) {
        log::warn!(
            target: "bexo::service::screenshot",
            "set screenshot overlay decorations failed: {}",
            error
        );
    }
    if let Err(error) = window.set_title("") {
        log::warn!(
            target: "bexo::service::screenshot",
            "set screenshot overlay title failed: {}",
            error
        );
    }
    if let Err(error) = window.set_resizable(false) {
        log::warn!(
            target: "bexo::service::screenshot",
            "set screenshot overlay resizable failed: {}",
            error
        );
    }
    lock_overlay_native_window_style(window)?;
    let style_ms = style_started_at.elapsed().as_millis();

    let geometry_started_at = Instant::now();
    if needs_geometry {
        apply_overlay_window_geometry(
            window,
            session.display_x + compensation_x,
            session.display_y + compensation_y,
            session.display_width,
            session.display_height,
        )?;
    }
    let geometry_ms = geometry_started_at.elapsed().as_millis();

    let interactive_started_at = Instant::now();
    set_overlay_window_interactive_state(window)?;
    let interactive_ms = interactive_started_at.elapsed().as_millis();

    let show_started_at = Instant::now();
    if !was_visible {
        window.show().map_err(|error| {
            AppError::new("SCREENSHOT_OVERLAY_SHOW_FAILED", "显示截图窗口失败")
                .with_detail("reason", error.to_string())
        })?;
    }
    let show_ms = show_started_at.elapsed().as_millis();

    let stabilize_started_at = Instant::now();
    if needs_geometry
        && !hidden_prewarmed
        && !stabilize_overlay_window_after_show(window, session, overlay_prewarmed)
    {
        log::warn!(
            target: "bexo::service::screenshot",
            "overlay_window_stabilization_fallback_exhausted session_id={} prewarmed={}",
            session.id,
            overlay_prewarmed
        );
    }
    let stabilize_ms = stabilize_started_at.elapsed().as_millis();

    let focus_started_at = Instant::now();
    if focus_requested {
        if let Err(error) = window.set_focus() {
            log::warn!(
                target: "bexo::service::screenshot",
                "focus screenshot overlay window failed: {}",
                error
            );
        }
    }
    let focus_ms = focus_started_at.elapsed().as_millis();

    let realign_started_at = Instant::now();
    let observed_focus_drift = realign_overlay_window_if_needed(
        window,
        session,
        if focus_requested {
            "post_focus_activation"
        } else {
            "post_show_activation"
        },
    )?;
    let corrected_after_focus = observed_focus_drift.is_some();
    let realign_ms = realign_started_at.elapsed().as_millis();
    log::info!(
        target: "bexo::service::screenshot",
        "overlay_hot_state_activated session_id={} prewarmed={} was_visible={} geometry_reused={} corrected_after_focus={}",
        session.id,
        overlay_prewarmed,
        was_visible,
        !needs_geometry && !corrected_after_focus,
        corrected_after_focus
    );

    if let Ok(probe) = probe_overlay_geometry(window, session) {
        log::info!(
            target: "bexo::service::screenshot",
            "overlay_geometry_applied session_id={} current_logical={}x{}@{},{} target_logical={}x{}@{},{} delta=({}, {}, {}, {}) outer_physical={}x{}@{},{} scale_factor={:.4}",
            session.id,
            probe.current_width,
            probe.current_height,
            probe.current_x,
            probe.current_y,
            session.display_width,
            session.display_height,
            session.display_x,
            session.display_y,
            probe.delta_x,
            probe.delta_y,
            probe.delta_width,
            probe.delta_height,
            probe.outer_width,
            probe.outer_height,
            probe.outer_x,
            probe.outer_y,
            probe.scale_factor
        );
    }
    if corrected_after_focus {
        log::info!(
            target: "bexo::service::screenshot",
            "overlay_hot_state_realigned session_id={} trigger=post_focus_activation",
            session.id
        );
    }
    log::info!(
        target: "bexo::service::screenshot",
        "overlay_activation_profile session_id={} prewarmed={} was_visible={} hidden_prewarmed={} geometry_aligned={} needs_geometry={} focused_before={} focus_requested={} focus_compensation=({}, {}) style_ms={} geometry_ms={} interactive_ms={} show_ms={} stabilize_ms={} focus_ms={} realign_ms={} total_ms={}",
        session.id,
        overlay_prewarmed,
        was_visible,
        hidden_prewarmed,
        geometry_aligned,
        needs_geometry,
        focused_before,
        focus_requested,
        compensation_x,
        compensation_y,
        style_ms,
        geometry_ms,
        interactive_ms,
        show_ms,
        stabilize_ms,
        focus_ms,
        realign_ms,
        activation_started_at.elapsed().as_millis()
    );

    Ok(OverlayActivationOutcome {
        observed_focus_drift,
    })
}

fn realign_overlay_window_if_needed<R: Runtime>(
    window: &tauri::WebviewWindow<R>,
    session: &ActiveScreenshotSession,
    trigger: &str,
) -> AppResult<Option<OverlayGeometryProbe>> {
    let probe = probe_overlay_geometry(window, session)?;
    if probe.is_aligned() {
        return Ok(None);
    }

    log::warn!(
        target: "bexo::service::screenshot",
        "overlay_geometry_drift_detected trigger={} session_id={} current_logical={}x{}@{},{} target_logical={}x{}@{},{} delta=({}, {}, {}, {}) outer_physical={}x{}@{},{} scale_factor={:.4}",
        trigger,
        session.id,
        probe.current_width,
        probe.current_height,
        probe.current_x,
        probe.current_y,
        session.display_width,
        session.display_height,
        session.display_x,
        session.display_y,
        probe.delta_x,
        probe.delta_y,
        probe.delta_width,
        probe.delta_height,
        probe.outer_width,
        probe.outer_height,
        probe.outer_x,
        probe.outer_y,
        probe.scale_factor
    );
    set_overlay_window_geometry(window, session)?;
    Ok(Some(probe))
}

fn stabilize_overlay_window_after_show<R: Runtime>(
    window: &tauri::WebviewWindow<R>,
    session: &ActiveScreenshotSession,
    overlay_prewarmed: bool,
) -> bool {
    if overlay_prewarmed {
        if let Ok(probe) = probe_overlay_geometry(window, session) {
            if probe.is_aligned() {
                return true;
            }
        }

        if let Err(error) = set_overlay_window_geometry(window, session) {
            log::warn!(
                target: "bexo::service::screenshot",
                "set screenshot overlay geometry during prewarmed stabilization failed: {}",
                error
            );
            return false;
        }

        if let Ok(probe) = probe_overlay_geometry(window, session) {
            if probe.is_aligned() {
                return true;
            }
        }
    }

    for _ in 0..OVERLAY_STABILIZE_MAX_ATTEMPTS {
        if let Err(error) = lock_overlay_native_window_style(window) {
            log::warn!(
                target: "bexo::service::screenshot",
                "lock screenshot overlay style during stabilization failed: {}",
                error
            );
            return false;
        }
        if let Err(error) = set_overlay_window_geometry(window, session) {
            log::warn!(
                target: "bexo::service::screenshot",
                "set screenshot overlay geometry during stabilization failed: {}",
                error
            );
            return false;
        }

        let Ok(probe) = probe_overlay_geometry(window, session) else {
            return false;
        };
        if probe.is_aligned() {
            return true;
        }

        std::thread::sleep(Duration::from_millis(OVERLAY_STABILIZE_INTERVAL_MS));
    }

    false
}

fn set_overlay_window_geometry<R: Runtime>(
    window: &tauri::WebviewWindow<R>,
    session: &ActiveScreenshotSession,
) -> AppResult<()> {
    apply_overlay_window_geometry(
        window,
        session.display_x,
        session.display_y,
        session.display_width,
        session.display_height,
    )?;

    for _ in 0..3 {
        let Ok(probe) = probe_overlay_geometry(window, session) else {
            break;
        };
        if probe.is_aligned() {
            break;
        }

        // Align the window client area (inner rect) to target logical coordinates.
        let corrected_x = session.display_x - probe.delta_x;
        let corrected_y = session.display_y - probe.delta_y;
        apply_overlay_window_geometry(
            window,
            corrected_x,
            corrected_y,
            session.display_width,
            session.display_height,
        )?;
    }

    Ok(())
}

fn apply_overlay_window_geometry<R: Runtime>(
    window: &tauri::WebviewWindow<R>,
    logical_x: i32,
    logical_y: i32,
    logical_width: u32,
    logical_height: u32,
) -> AppResult<()> {
    window
        .set_position(Position::Logical(LogicalPosition::new(
            f64::from(logical_x),
            f64::from(logical_y),
        )))
        .map_err(|error| {
            AppError::new("SCREENSHOT_OVERLAY_POSITION_FAILED", "定位截图窗口失败")
                .with_detail("reason", error.to_string())
        })?;
    window
        .set_size(Size::Logical(LogicalSize::new(
            f64::from(logical_width),
            f64::from(logical_height),
        )))
        .map_err(|error| {
            AppError::new("SCREENSHOT_OVERLAY_RESIZE_FAILED", "调整截图窗口尺寸失败")
                .with_detail("reason", error.to_string())
        })?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn lock_overlay_native_window_style<R: Runtime>(window: &tauri::WebviewWindow<R>) -> AppResult<()> {
    let hwnd = window.hwnd().map_err(|error| {
        AppError::new(
            "SCREENSHOT_OVERLAY_STYLE_LOCK_FAILED",
            "读取截图窗口句柄失败",
        )
        .with_detail("reason", error.to_string())
    })?;

    let hwnd_raw = hwnd.0 as isize;
    if hwnd_raw == 0 {
        return Err(AppError::new(
            "SCREENSHOT_OVERLAY_STYLE_LOCK_FAILED",
            "截图窗口句柄无效",
        ));
    }

    let style = unsafe { GetWindowLongPtrW(hwnd_raw, GWL_STYLE) as u32 };
    let locked_style = (style
        & !(WS_CAPTION | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_SYSMENU))
        | WS_POPUP;
    if locked_style != style {
        unsafe {
            SetWindowLongPtrW(hwnd_raw, GWL_STYLE, locked_style as isize);
        }
    }

    let ex_style = unsafe { GetWindowLongPtrW(hwnd_raw, GWL_EXSTYLE) as u32 };
    let locked_ex_style = (ex_style
        & !(WS_EX_DLGMODALFRAME | WS_EX_CLIENTEDGE | WS_EX_STATICEDGE | WS_EX_WINDOWEDGE))
        | WS_EX_TOOLWINDOW;
    if locked_ex_style != ex_style {
        unsafe {
            SetWindowLongPtrW(hwnd_raw, GWL_EXSTYLE, locked_ex_style as isize);
        }
    }

    unsafe {
        SetWindowPos(
            hwnd_raw,
            HWND_TOPMOST as isize,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_FRAMECHANGED,
        );
    }

    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn lock_overlay_native_window_style<R: Runtime>(
    _window: &tauri::WebviewWindow<R>,
) -> AppResult<()> {
    Ok(())
}

fn probe_overlay_geometry<R: Runtime>(
    window: &tauri::WebviewWindow<R>,
    session: &ActiveScreenshotSession,
) -> AppResult<OverlayGeometryProbe> {
    let scale_factor = window.scale_factor().map_err(|error| {
        AppError::new("SCREENSHOT_OVERLAY_PROBE_FAILED", "读取截图窗口缩放失败")
            .with_detail("reason", error.to_string())
    })?;
    let safe_scale_factor = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    };

    let inner_position = window.inner_position().map_err(|error| {
        AppError::new(
            "SCREENSHOT_OVERLAY_PROBE_FAILED",
            "读取截图窗口内容位置失败",
        )
        .with_detail("reason", error.to_string())
    })?;
    let outer_position = window.outer_position().map_err(|error| {
        AppError::new("SCREENSHOT_OVERLAY_PROBE_FAILED", "读取截图窗口位置失败")
            .with_detail("reason", error.to_string())
    })?;
    let inner_size = window.inner_size().map_err(|error| {
        AppError::new("SCREENSHOT_OVERLAY_PROBE_FAILED", "读取截图窗口尺寸失败")
            .with_detail("reason", error.to_string())
    })?;
    let outer_size = window.outer_size().map_err(|error| {
        AppError::new(
            "SCREENSHOT_OVERLAY_PROBE_FAILED",
            "读取截图窗口外框尺寸失败",
        )
        .with_detail("reason", error.to_string())
    })?;

    let current_position_logical = inner_position.to_logical::<f64>(safe_scale_factor);
    let current_size_logical = inner_size.to_logical::<f64>(safe_scale_factor);

    let current_x = current_position_logical.x.round() as i32;
    let current_y = current_position_logical.y.round() as i32;
    let current_width = current_size_logical.width.round().max(1.0) as u32;
    let current_height = current_size_logical.height.round().max(1.0) as u32;

    Ok(OverlayGeometryProbe {
        scale_factor: safe_scale_factor,
        current_x,
        current_y,
        current_width,
        current_height,
        delta_x: current_x - session.display_x,
        delta_y: current_y - session.display_y,
        delta_width: current_width as i32 - session.display_width as i32,
        delta_height: current_height as i32 - session.display_height as i32,
        outer_x: outer_position.x,
        outer_y: outer_position.y,
        outer_width: outer_size.width,
        outer_height: outer_size.height,
    })
}

fn emit_session_updated<R: Runtime>(app: &AppHandle<R>, session: &ActiveScreenshotSession) {
    let payload = ScreenshotSessionUpdatedEvent {
        session_id: session.id.clone(),
        created_at: session.created_at.clone(),
    };

    if let Err(error) = app.emit(SCREENSHOT_SESSION_UPDATED_EVENT_NAME, payload) {
        log::warn!(
            target: "bexo::service::screenshot",
            "emit screenshot session updated event failed: {}",
            error
        );
    }
}

fn session_to_view(session: &ActiveScreenshotSession) -> ScreenshotSessionView {
    ScreenshotSessionView {
        session_id: session.id.clone(),
        created_at: session.created_at.clone(),
        display_x: session.display_x,
        display_y: session.display_y,
        display_width: session.display_width,
        display_height: session.display_height,
        scale_factor: session.scale_factor,
        capture_width: session.capture_width,
        capture_height: session.capture_height,
        image_status: session.image_status,
        image_error: session.image_error.clone(),
        image_data_url: session.image_data_url.as_ref().clone(),
        preview_image_path: session
            .preview_image_path
            .as_ref()
            .map(|value| value.as_ref().clone()),
        preview_transport: session.preview_transport,
        preview_pixel_width: session.preview_pixel_width,
        preview_pixel_height: session.preview_pixel_height,
        monitors: session
            .monitors
            .iter()
            .map(CapturedMonitorFrame::to_view)
            .collect(),
    }
}

fn encode_png(image: &RgbaImage) -> AppResult<Vec<u8>> {
    let mut buffer = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image.clone())
        .write_to(&mut buffer, ImageOutputFormat::Png)
        .map_err(|error| {
            AppError::new("SCREENSHOT_ENCODE_FAILED", "截图编码失败")
                .with_detail("reason", error.to_string())
        })?;
    Ok(buffer.into_inner())
}

fn encode_preview_png_fast(image: &RgbaImage) -> AppResult<Vec<u8>> {
    let mut buffer = Vec::new();
    let encoder = PngEncoder::new_with_quality(
        &mut buffer,
        PngCompressionType::Fast,
        PngFilterType::NoFilter,
    );
    encoder
        .write_image(
            image.as_raw().as_slice(),
            image.width(),
            image.height(),
            ColorType::Rgba8.into(),
        )
        .map_err(|error| {
            AppError::new("SCREENSHOT_ENCODE_FAILED", "截图编码失败")
                .with_detail("reason", error.to_string())
        })?;
    Ok(buffer)
}

fn encode_preview_bmp_fast(image: &RgbaImage) -> AppResult<Vec<u8>> {
    let width = usize::try_from(image.width()).map_err(|error| {
        AppError::new("SCREENSHOT_ENCODE_FAILED", "截图编码失败")
            .with_detail("reason", error.to_string())
    })?;
    let height = usize::try_from(image.height()).map_err(|error| {
        AppError::new("SCREENSHOT_ENCODE_FAILED", "截图编码失败")
            .with_detail("reason", error.to_string())
    })?;
    if width == 0 || height == 0 {
        return Err(AppError::new("SCREENSHOT_ENCODE_FAILED", "截图编码失败")
            .with_detail("reason", "empty image"));
    }

    let row_bytes = width.checked_mul(3).ok_or_else(|| {
        AppError::new("SCREENSHOT_ENCODE_FAILED", "截图编码失败")
            .with_detail("reason", "bmp row bytes overflow")
    })?;
    let row_stride = (row_bytes + 3) & !3;
    let pixel_bytes = row_stride.checked_mul(height).ok_or_else(|| {
        AppError::new("SCREENSHOT_ENCODE_FAILED", "截图编码失败")
            .with_detail("reason", "bmp pixel bytes overflow")
    })?;
    let file_size = BMP_PIXEL_OFFSET.checked_add(pixel_bytes).ok_or_else(|| {
        AppError::new("SCREENSHOT_ENCODE_FAILED", "截图编码失败")
            .with_detail("reason", "bmp file size overflow")
    })?;

    if file_size > u32::MAX as usize || width > i32::MAX as usize || height > i32::MAX as usize {
        return Err(AppError::new("SCREENSHOT_ENCODE_FAILED", "截图编码失败")
            .with_detail("reason", "bmp dimensions exceed supported range"));
    }

    let mut buffer = Vec::with_capacity(file_size);
    buffer.extend_from_slice(b"BM");
    buffer.extend_from_slice(&(file_size as u32).to_le_bytes());
    buffer.extend_from_slice(&0u16.to_le_bytes());
    buffer.extend_from_slice(&0u16.to_le_bytes());
    buffer.extend_from_slice(&(BMP_PIXEL_OFFSET as u32).to_le_bytes());
    buffer.extend_from_slice(&(BMP_INFO_HEADER_SIZE as u32).to_le_bytes());
    buffer.extend_from_slice(&(width as i32).to_le_bytes());
    buffer.extend_from_slice(&(height as i32).to_le_bytes());
    buffer.extend_from_slice(&1u16.to_le_bytes());
    buffer.extend_from_slice(&24u16.to_le_bytes());
    buffer.extend_from_slice(&0u32.to_le_bytes());
    buffer.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
    buffer.extend_from_slice(&2835i32.to_le_bytes());
    buffer.extend_from_slice(&2835i32.to_le_bytes());
    buffer.extend_from_slice(&0u32.to_le_bytes());
    buffer.extend_from_slice(&0u32.to_le_bytes());

    let raw = image.as_raw();
    let padding = row_stride - row_bytes;
    let zero_padding = [0u8; 3];

    for y in (0..height).rev() {
        let row_start = y
            .checked_mul(width)
            .and_then(|offset| offset.checked_mul(4))
            .ok_or_else(|| {
                AppError::new("SCREENSHOT_ENCODE_FAILED", "截图编码失败")
                    .with_detail("reason", "bmp row offset overflow")
            })?;
        let row_end = row_start.checked_add(width * 4).ok_or_else(|| {
            AppError::new("SCREENSHOT_ENCODE_FAILED", "截图编码失败")
                .with_detail("reason", "bmp row end overflow")
        })?;
        let row = &raw[row_start..row_end];
        for pixel in row.chunks_exact(4) {
            buffer.push(pixel[2]);
            buffer.push(pixel[1]);
            buffer.push(pixel[0]);
        }
        if padding > 0 {
            buffer.extend_from_slice(&zero_padding[..padding]);
        }
    }

    Ok(buffer)
}

fn encode_bmp_from_bottom_up_bgra(
    width: u32,
    height: u32,
    bottom_up_bgra: &[u8],
) -> AppResult<Vec<u8>> {
    let width_usize = usize::try_from(width).map_err(|error| {
        AppError::new("SCREENSHOT_ENCODE_FAILED", "截图编码失败")
            .with_detail("reason", error.to_string())
    })?;
    let height_usize = usize::try_from(height).map_err(|error| {
        AppError::new("SCREENSHOT_ENCODE_FAILED", "截图编码失败")
            .with_detail("reason", error.to_string())
    })?;
    if width_usize == 0 || height_usize == 0 {
        return Err(AppError::new("SCREENSHOT_ENCODE_FAILED", "截图编码失败")
            .with_detail("reason", "empty image"));
    }

    let expected_len = width_usize
        .checked_mul(height_usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| {
            AppError::new("SCREENSHOT_ENCODE_FAILED", "截图编码失败")
                .with_detail("reason", "bmp raw buffer overflow")
        })?;
    if bottom_up_bgra.len() != expected_len {
        return Err(AppError::new("SCREENSHOT_ENCODE_FAILED", "截图编码失败")
            .with_detail("reason", "bmp raw buffer length mismatch")
            .with_detail("expected", expected_len.to_string())
            .with_detail("actual", bottom_up_bgra.len().to_string()));
    }

    let row_bytes = width_usize.checked_mul(3).ok_or_else(|| {
        AppError::new("SCREENSHOT_ENCODE_FAILED", "截图编码失败")
            .with_detail("reason", "bmp row bytes overflow")
    })?;
    let row_stride = (row_bytes + 3) & !3;
    let pixel_bytes = row_stride.checked_mul(height_usize).ok_or_else(|| {
        AppError::new("SCREENSHOT_ENCODE_FAILED", "截图编码失败")
            .with_detail("reason", "bmp pixel bytes overflow")
    })?;
    let file_size = BMP_PIXEL_OFFSET.checked_add(pixel_bytes).ok_or_else(|| {
        AppError::new("SCREENSHOT_ENCODE_FAILED", "截图编码失败")
            .with_detail("reason", "bmp file size overflow")
    })?;

    if file_size > u32::MAX as usize
        || width_usize > i32::MAX as usize
        || height_usize > i32::MAX as usize
    {
        return Err(AppError::new("SCREENSHOT_ENCODE_FAILED", "截图编码失败")
            .with_detail("reason", "bmp dimensions exceed supported range"));
    }

    let mut buffer = Vec::with_capacity(file_size);
    buffer.extend_from_slice(b"BM");
    buffer.extend_from_slice(&(file_size as u32).to_le_bytes());
    buffer.extend_from_slice(&0u16.to_le_bytes());
    buffer.extend_from_slice(&0u16.to_le_bytes());
    buffer.extend_from_slice(&(BMP_PIXEL_OFFSET as u32).to_le_bytes());
    buffer.extend_from_slice(&(BMP_INFO_HEADER_SIZE as u32).to_le_bytes());
    buffer.extend_from_slice(&(width_usize as i32).to_le_bytes());
    buffer.extend_from_slice(&(height_usize as i32).to_le_bytes());
    buffer.extend_from_slice(&1u16.to_le_bytes());
    buffer.extend_from_slice(&24u16.to_le_bytes());
    buffer.extend_from_slice(&0u32.to_le_bytes());
    buffer.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
    buffer.extend_from_slice(&2835i32.to_le_bytes());
    buffer.extend_from_slice(&2835i32.to_le_bytes());
    buffer.extend_from_slice(&0u32.to_le_bytes());
    buffer.extend_from_slice(&0u32.to_le_bytes());

    let padding = row_stride - row_bytes;
    let zero_padding = [0u8; 3];
    for row in bottom_up_bgra.chunks_exact(width_usize * 4) {
        for pixel in row.chunks_exact(4) {
            buffer.push(pixel[0]);
            buffer.push(pixel[1]);
            buffer.push(pixel[2]);
        }
        if padding > 0 {
            buffer.extend_from_slice(&zero_padding[..padding]);
        }
    }

    Ok(buffer)
}

fn normalize_selection(
    session: &ActiveScreenshotSession,
    selection: ScreenshotSelectionInput,
) -> AppResult<LogicalSelection> {
    if !selection.x.is_finite()
        || !selection.y.is_finite()
        || !selection.width.is_finite()
        || !selection.height.is_finite()
    {
        return Err(AppError::new(
            "SCREENSHOT_SELECTION_INVALID",
            "截图选区数据无效",
        ));
    }

    if selection.width < 1.0 || selection.height < 1.0 {
        return Err(AppError::new(
            "SCREENSHOT_SELECTION_EMPTY",
            "截图选区尺寸必须大于 0",
        ));
    }

    let selection_right = selection.x + selection.width;
    let selection_bottom = selection.y + selection.height;
    let max_width = f64::from(session.display_width);
    let max_height = f64::from(session.display_height);

    if selection.x < 0.0
        || selection.y < 0.0
        || selection_right > max_width + 0.1
        || selection_bottom > max_height + 0.1
    {
        return Err(AppError::new(
            "SCREENSHOT_SELECTION_OUT_OF_RANGE",
            "截图选区超出屏幕范围",
        ));
    }

    let left = selection.x.floor().clamp(0.0, max_width);
    let top = selection.y.floor().clamp(0.0, max_height);
    let right = selection_right.ceil().clamp(0.0, max_width);
    let bottom = selection_bottom.ceil().clamp(0.0, max_height);

    if right <= left || bottom <= top {
        return Err(AppError::new(
            "SCREENSHOT_SELECTION_EMPTY",
            "截图选区尺寸必须大于 0",
        ));
    }

    let width = (right - left) as u32;
    let height = (bottom - top) as u32;
    if width == 0 || height == 0 {
        return Err(AppError::new(
            "SCREENSHOT_SELECTION_EMPTY",
            "截图选区尺寸必须大于 0",
        ));
    }

    Ok(LogicalSelection {
        x: left as u32,
        y: top as u32,
        width,
        height,
    })
}

fn render_selection_base_image(
    session: &ActiveScreenshotSession,
    selection: ScreenshotSelectionInput,
) -> AppResult<SelectionRenderData> {
    let render_started_at = Instant::now();
    let logical_selection = normalize_selection(session, selection)?;
    let intersections = collect_monitor_intersections(session, logical_selection);
    if intersections.is_empty() {
        return Err(AppError::new(
            "SCREENSHOT_SELECTION_OUT_OF_RANGE",
            "截图选区未命中任何显示器区域",
        ));
    }

    let uniform_scale = resolve_uniform_render_scale(session, intersections.as_slice());
    let (mode, scale_factor) = match uniform_scale {
        Some(scale) => (ScreenshotSelectionRenderMode::Native, scale),
        None => (ScreenshotSelectionRenderMode::LogicalFallback, 1.0),
    };

    let output_width = match mode {
        ScreenshotSelectionRenderMode::Native => {
            compute_scaled_length(logical_selection.width, scale_factor)
        }
        ScreenshotSelectionRenderMode::LogicalFallback => logical_selection.width,
    }
    .max(1);
    let output_height = match mode {
        ScreenshotSelectionRenderMode::Native => {
            compute_scaled_length(logical_selection.height, scale_factor)
        }
        ScreenshotSelectionRenderMode::LogicalFallback => logical_selection.height,
    }
    .max(1);

    let mut output = RgbaImage::from_pixel(output_width, output_height, Rgba([0, 0, 0, 255]));
    let mut tiles = Vec::with_capacity(intersections.len());

    for intersection in intersections {
        let monitor = &session.monitors[intersection.monitor_index];
        let cropped = crop_monitor_intersection(monitor, intersection)?;
        let (output_x, output_y, tile_width, tile_height) = match mode {
            ScreenshotSelectionRenderMode::Native => {
                let local_x = intersection.logical_x - logical_selection.x;
                let local_y = intersection.logical_y - logical_selection.y;
                let output_left = compute_scaled_offset(local_x, scale_factor);
                let output_top = compute_scaled_offset(local_y, scale_factor);
                let output_right = compute_scaled_end(
                    local_x.saturating_add(intersection.logical_width),
                    scale_factor,
                );
                let output_bottom = compute_scaled_end(
                    local_y.saturating_add(intersection.logical_height),
                    scale_factor,
                );
                (
                    output_left,
                    output_top,
                    output_right.saturating_sub(output_left).max(1),
                    output_bottom.saturating_sub(output_top).max(1),
                )
            }
            ScreenshotSelectionRenderMode::LogicalFallback => (
                intersection.logical_x - logical_selection.x,
                intersection.logical_y - logical_selection.y,
                intersection.logical_width.max(1),
                intersection.logical_height.max(1),
            ),
        };

        let tile_image = match mode {
            ScreenshotSelectionRenderMode::Native => cropped,
            ScreenshotSelectionRenderMode::LogicalFallback => {
                imageops::resize(&cropped, tile_width, tile_height, FilterType::Triangle)
            }
        };

        imageops::overlay(
            &mut output,
            &tile_image,
            i64::from(output_x),
            i64::from(output_y),
        );

        tiles.push(ScreenshotSelectionRenderTile {
            display_id: monitor.display_id,
            scale_factor: monitor.scale_factor,
            logical_x: f64::from(intersection.logical_x - logical_selection.x),
            logical_y: f64::from(intersection.logical_y - logical_selection.y),
            logical_width: f64::from(intersection.logical_width),
            logical_height: f64::from(intersection.logical_height),
            output_x,
            output_y,
            output_width: tile_width,
            output_height: tile_height,
        });
    }

    log::info!(
        target: "bexo::service::screenshot",
        "selection_render_composed session_id={} mode={:?} selection={}x{} intersections={} total_ms={} scale_factor={}",
        session.id,
        mode,
        logical_selection.width,
        logical_selection.height,
        tiles.len(),
        render_started_at.elapsed().as_millis(),
        scale_factor
    );

    Ok(SelectionRenderData {
        mode,
        scale_factor,
        image: output,
        tiles,
    })
}

fn collect_monitor_intersections(
    session: &ActiveScreenshotSession,
    selection: LogicalSelection,
) -> Vec<MonitorSelectionIntersection> {
    let selection_right = selection.x.saturating_add(selection.width);
    let selection_bottom = selection.y.saturating_add(selection.height);

    session
        .monitors
        .iter()
        .enumerate()
        .filter_map(|(index, monitor)| {
            let monitor_left = monitor.relative_x;
            let monitor_top = monitor.relative_y;
            let monitor_right = monitor_left.saturating_add(monitor.display_width);
            let monitor_bottom = monitor_top.saturating_add(monitor.display_height);

            let left = selection.x.max(monitor_left);
            let top = selection.y.max(monitor_top);
            let right = selection_right.min(monitor_right);
            let bottom = selection_bottom.min(monitor_bottom);

            if right <= left || bottom <= top {
                return None;
            }

            Some(MonitorSelectionIntersection {
                monitor_index: index,
                logical_x: left,
                logical_y: top,
                logical_width: right - left,
                logical_height: bottom - top,
            })
        })
        .collect()
}

fn resolve_uniform_render_scale(
    session: &ActiveScreenshotSession,
    intersections: &[MonitorSelectionIntersection],
) -> Option<f32> {
    let first = intersections.first()?;
    let first_monitor = &session.monitors[first.monitor_index];
    let first_scale_x = first_monitor.scale_x();
    let first_scale_y = first_monitor.scale_y();
    if !approx_eq(first_scale_x, first_scale_y) {
        return None;
    }

    for intersection in intersections.iter().skip(1) {
        let monitor = &session.monitors[intersection.monitor_index];
        let scale_x = monitor.scale_x();
        let scale_y = monitor.scale_y();
        if !approx_eq(scale_x, scale_y)
            || !approx_eq(scale_x, first_scale_x)
            || !approx_eq(scale_y, first_scale_y)
        {
            return None;
        }
    }

    Some(first_scale_x as f32)
}

fn crop_monitor_intersection(
    monitor: &CapturedMonitorFrame,
    intersection: MonitorSelectionIntersection,
) -> AppResult<RgbaImage> {
    let relative_x = intersection.logical_x.saturating_sub(monitor.relative_x);
    let relative_y = intersection.logical_y.saturating_sub(monitor.relative_y);

    let crop_left = (f64::from(relative_x) * monitor.scale_x()).floor() as u32;
    let crop_top = (f64::from(relative_y) * monitor.scale_y()).floor() as u32;
    let crop_right = (f64::from(relative_x.saturating_add(intersection.logical_width))
        * monitor.scale_x())
    .ceil() as u32;
    let crop_bottom = (f64::from(relative_y.saturating_add(intersection.logical_height))
        * monitor.scale_y())
    .ceil() as u32;

    let bounded_left = crop_left.min(monitor.capture_width.saturating_sub(1));
    let bounded_top = crop_top.min(monitor.capture_height.saturating_sub(1));
    let bounded_right = crop_right
        .max(bounded_left.saturating_add(1))
        .min(monitor.capture_width);
    let bounded_bottom = crop_bottom
        .max(bounded_top.saturating_add(1))
        .min(monitor.capture_height);

    let width = bounded_right.saturating_sub(bounded_left).max(1);
    let height = bounded_bottom.saturating_sub(bounded_top).max(1);
    let source = monitor.source_image()?;
    Ok(imageops::crop_imm(source, bounded_left, bounded_top, width, height).to_image())
}

fn resolve_monitor_scale_factor(
    display_width: u32,
    capture_width: u32,
    display_height: u32,
    capture_height: u32,
    fallback: f32,
) -> f32 {
    let scale_x = if display_width == 0 {
        1.0
    } else {
        capture_width as f32 / display_width as f32
    };
    let scale_y = if display_height == 0 {
        1.0
    } else {
        capture_height as f32 / display_height as f32
    };

    let measured = (scale_x + scale_y) / 2.0;
    if measured.is_finite() && measured > 0.0 {
        measured
    } else {
        fallback.max(1.0)
    }
}

fn derive_legacy_capture_metrics(
    monitors: &[CapturedMonitorFrame],
    display_width: u32,
    display_height: u32,
) -> (f32, u32, u32) {
    let Some(first) = monitors.first() else {
        return (1.0, display_width, display_height);
    };

    let first_scale = first.scale_factor as f64;
    let has_uniform_scale = monitors.iter().all(|monitor| {
        approx_eq(monitor.scale_x(), first_scale)
            && approx_eq(monitor.scale_y(), first_scale)
            && approx_eq(monitor.scale_x(), monitor.scale_y())
    });

    if !has_uniform_scale {
        return (1.0, display_width, display_height);
    }

    let scale = first.scale_factor.max(1.0);
    (
        scale,
        compute_scaled_length(display_width, scale),
        compute_scaled_length(display_height, scale),
    )
}

fn compute_scaled_length(value: u32, scale_factor: f32) -> u32 {
    ((f64::from(value) * f64::from(scale_factor)).ceil() as u32).max(1)
}

fn compute_scaled_offset(value: u32, scale_factor: f32) -> u32 {
    (f64::from(value) * f64::from(scale_factor)).floor() as u32
}

fn compute_scaled_end(value: u32, scale_factor: f32) -> u32 {
    (f64::from(value) * f64::from(scale_factor)).ceil() as u32
}

fn resolve_uniform_preview_scale(monitors: &[CapturedMonitorFrame]) -> Option<f32> {
    let first = monitors.first()?;
    let first_scale_x = first.scale_x();
    let first_scale_y = first.scale_y();
    if !approx_eq(first_scale_x, first_scale_y) {
        return None;
    }

    for monitor in monitors.iter().skip(1) {
        let scale_x = monitor.scale_x();
        let scale_y = monitor.scale_y();
        if !approx_eq(scale_x, scale_y) || !approx_eq(scale_x, first_scale_x) {
            return None;
        }
    }

    let scale = first_scale_x as f32;
    if scale.is_finite() && scale > 0.0 {
        Some(scale)
    } else {
        None
    }
}

fn approx_eq(left: f64, right: f64) -> bool {
    (left - right).abs() <= DPI_SCALE_EPSILON
}

fn resolve_output_path<R: Runtime>(
    app: &AppHandle<R>,
    file_path: Option<String>,
) -> AppResult<PathBuf> {
    let resolved = if let Some(raw) = file_path {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(AppError::validation("保存路径不能为空"));
        }

        let mut path = PathBuf::from(trimmed);
        if !path.is_absolute() {
            return Err(AppError::validation("保存路径必须是绝对路径")
                .with_detail("filePath", trimmed.to_string()));
        }

        if path.extension().is_none() {
            path.set_extension("png");
        }

        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .unwrap_or_default();
        if extension != "png" {
            return Err(AppError::validation("截图仅支持保存为 PNG 格式")
                .with_detail("filePath", path.display().to_string()));
        }

        path
    } else {
        let base_dir = app
            .path()
            .picture_dir()
            .or_else(|_| app.path().desktop_dir())
            .or_else(|_| app.path().temp_dir())
            .map_err(|error| {
                AppError::new(
                    "SCREENSHOT_SAVE_PATH_UNAVAILABLE",
                    "无法解析截图默认保存目录",
                )
                .with_detail("reason", error.to_string())
            })?;
        let file_name = format!(
            "bexo-screenshot-{}.png",
            Local::now().format("%Y%m%d-%H%M%S")
        );
        base_dir.join(file_name)
    };

    Ok(resolved)
}

impl ScreenshotService {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(ScreenshotState::default())),
            live_capture: Arc::new(Mutex::new(LiveCaptureState::default())),
        }
    }

    pub fn initialize_live_capture<R: Runtime>(&self, app: &AppHandle<R>) -> AppResult<()> {
        #[cfg(not(target_os = "windows"))]
        {
            let _ = app;
            return Ok(());
        }

        #[cfg(target_os = "windows")]
        {
            self.initialize_live_capture_windows(app)
        }
    }

    pub fn prewarm_overlay_window<R: Runtime>(&self, app: &AppHandle<R>) -> AppResult<()> {
        if self.is_overlay_prewarmed()? {
            return Ok(());
        }

        let session = build_overlay_prewarm_session(app);
        let started_at = Instant::now();
        self.suppress_overlay_window_events(Duration::from_millis(OVERLAY_EVENT_SUPPRESS_MS))?;
        let window = prepare_overlay_window(app, &session)?;
        prewarm_overlay_window_once(&window, &session)?;
        self.set_overlay_prewarmed(true)?;

        log::info!(
            target: "bexo::service::screenshot",
            "overlay_prewarm_ready total_ms={} geometry={}x{}@{},{}",
            started_at.elapsed().as_millis(),
            session.display_width,
            session.display_height,
            session.display_x,
            session.display_y
        );

        Ok(())
    }

    pub fn restore_overlay_hot_state<R: Runtime>(&self, app: &AppHandle<R>) -> AppResult<()> {
        let session = build_overlay_prewarm_session(app);
        let started_at = Instant::now();
        self.suppress_overlay_window_events(Duration::from_millis(OVERLAY_EVENT_SUPPRESS_MS))?;
        let window = prepare_overlay_window(app, &session)?;
        restore_overlay_window_hot_state(&window, &session)?;
        self.set_overlay_prewarmed(true)?;

        log::info!(
            target: "bexo::service::screenshot",
            "overlay_hidden_after_session total_ms={} geometry={}x{}@{},{}",
            started_at.elapsed().as_millis(),
            session.display_width,
            session.display_height,
            session.display_x,
            session.display_y
        );

        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn initialize_live_capture_windows<R: Runtime>(&self, app: &AppHandle<R>) -> AppResult<()> {
        {
            let guard = self.live_capture.lock().map_err(|_| {
                AppError::new(
                    "SCREENSHOT_LIVE_CAPTURE_LOCK_FAILED",
                    "读取持续截图状态失败",
                )
            })?;
            if guard.worker.is_some() {
                return Ok(());
            }
        }

        let Some(context) = collect_live_capture_init_context(app)? else {
            log::info!(
                target: "bexo::service::screenshot",
                "live_capture_skipped reason=single_monitor_context_unavailable"
            );
            return Ok(());
        };

        let service = self.clone();
        let display_id = context.display_id;
        let monitor_handle = context.monitor_handle;
        let display_x = context.display_x;
        let display_y = context.display_y;
        let display_width = context.display_width;
        let display_height = context.display_height;
        let scale_factor = context.scale_factor;
        let preview_pixel_width = context.preview_pixel_width;
        let preview_pixel_height = context.preview_pixel_height;

        let started_at = Instant::now();
        match desktop_duplication_capture::start_live_capture(
            monitor_handle,
            Duration::from_millis(LIVE_CAPTURE_MIN_INTERVAL_MS),
            {
                let service = service.clone();
                move |frame| match service.store_live_capture_frame_snapshot_windows(
                    LiveCaptureBackend::DesktopDuplication,
                    display_id,
                    monitor_handle,
                    display_x,
                    display_y,
                    display_width,
                    display_height,
                    scale_factor,
                    preview_pixel_width,
                    preview_pixel_height,
                    frame.sequence,
                    frame.captured_at,
                    frame.width.max(1),
                    frame.height.max(1),
                    frame.bgra_top_down,
                ) {
                    Ok(true) => {
                        if frame.sequence == 1
                            || frame.sequence % LIVE_CAPTURE_LOG_EVERY_N_FRAMES == 0
                        {
                            log::info!(
                                target: "bexo::service::screenshot",
                                "desktop_duplication_frame_ready display_id={} sequence={} pixels={}x{} preview_pixels={}x{} frame_wait_ms={} map_ms={} total_ms={}",
                                display_id,
                                frame.sequence,
                                frame.width,
                                frame.height,
                                preview_pixel_width,
                                preview_pixel_height,
                                frame.frame_wait_ms,
                                frame.map_ms,
                                frame.total_ms
                            );
                        }
                    }
                    Ok(false) => {
                        if frame.sequence == 1
                            || frame.sequence % LIVE_CAPTURE_LOG_EVERY_N_FRAMES == 0
                        {
                            log::info!(
                                target: "bexo::service::screenshot",
                                "desktop_duplication_frame_dropped display_id={} sequence={} reason=active_screenshot_session",
                                display_id,
                                frame.sequence
                            );
                        }
                    }
                    Err(error) => {
                        log::warn!(
                            target: "bexo::service::screenshot",
                            "desktop_duplication_frame_store_failed display_id={} sequence={} reason={}",
                            display_id,
                            frame.sequence,
                            error
                        );
                    }
                }
            },
        ) {
            Ok((handle, started)) => {
                let mut guard = self.live_capture.lock().map_err(|_| {
                    AppError::new(
                        "SCREENSHOT_LIVE_CAPTURE_LOCK_FAILED",
                        "更新持续截图状态失败",
                    )
                })?;
                guard.worker = Some(LiveCaptureWorker { _handle: handle });
                drop(guard);

                log::info!(
                    target: "bexo::service::screenshot",
                    "desktop_duplication_live_capture_started display_id={} pixels={}x{} preview_pixels={}x{} factory_create_ms={} adapter_match_ms={} device_create_ms={} duplication_create_ms={} total_ms={}",
                    context.display_id,
                    started.width,
                    started.height,
                    context.preview_pixel_width,
                    context.preview_pixel_height,
                    started.factory_create_ms,
                    started.adapter_match_ms,
                    started.device_create_ms,
                    started.duplication_create_ms,
                    started_at.elapsed().as_millis()
                );

                Ok(())
            }
            Err(error) => {
                log::warn!(
                    target: "bexo::service::screenshot",
                    "desktop_duplication_live_capture_failed display_id={} reason={} fallback=disabled_live_cache",
                    context.display_id,
                    error
                );
                Ok(())
            }
        }
    }

    pub fn start_session<R: Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> AppResult<StartScreenshotSessionResult> {
        let started_at = Instant::now();
        let hide_overlay_started_at = Instant::now();
        let had_active_session = self.get_active_session_optional()?.is_some();
        if had_active_session {
            hide_overlay_if_visible(app);
        }
        let hide_overlay_ms = hide_overlay_started_at.elapsed().as_millis();

        let capture_started_at = Instant::now();
        let live_capture_available = self.get_live_capture_snapshot()?.is_some();
        #[cfg(target_os = "windows")]
        let initial_live_cache_capture = match self.try_capture_from_live_cache() {
            Ok(value) => value,
            Err(error) => {
                log::warn!(
                    target: "bexo::service::screenshot",
                    "live_capture_snapshot_use_failed reason={} fallback=one_shot_capture",
                    error
                );
                None
            }
        };
        #[cfg(target_os = "windows")]
        let live_cache_capture = if initial_live_cache_capture.is_some() {
            initial_live_cache_capture
        } else {
            match self.wait_for_live_capture_snapshot(Duration::from_millis(
                LIVE_CAPTURE_WAIT_FOR_READY_FRAME_MS,
            )) {
                Ok(value) => value,
                Err(error) => {
                    log::warn!(
                        target: "bexo::service::screenshot",
                        "live_capture_snapshot_wait_failed reason={} fallback=one_shot_capture",
                        error
                    );
                    None
                }
            }
        };
        #[cfg(not(target_os = "windows"))]
        let live_cache_capture: Option<(FastPreviewCapture, u128, u64)> = None;
        let fast_preview_capture = if live_cache_capture.is_some() {
            None
        } else {
            capture_single_monitor_fast_preview(app)?
        };
        let (
            captured,
            preview_transport,
            preview_pixel_width,
            preview_pixel_height,
            preview_protocol_bytes,
            capture_strategy,
            frame_age_ms,
            live_capture_sequence,
        ) = if let Some(fast) = fast_preview_capture {
            (
                fast.captured,
                ScreenshotPreviewTransport::RawRgbaFast,
                fast.preview_pixel_width,
                fast.preview_pixel_height,
                Some(fast.preview_protocol_bytes),
                fast.capture_strategy,
                None,
                None,
            )
        } else if let Some((fast, frame_age_ms, live_capture_sequence)) = live_cache_capture {
            (
                fast.captured,
                ScreenshotPreviewTransport::RawRgbaFast,
                fast.preview_pixel_width,
                fast.preview_pixel_height,
                Some(fast.preview_protocol_bytes),
                fast.capture_strategy,
                Some(frame_age_ms),
                Some(live_capture_sequence),
            )
        } else {
            let captured = capture_virtual_desktop(app)?;
            let (preview_transport, preview_pixel_width, preview_pixel_height) =
                resolve_preview_transport(
                    captured.monitors.as_slice(),
                    captured.desktop.width,
                    captured.desktop.height,
                );
            (
                captured,
                preview_transport,
                preview_pixel_width,
                preview_pixel_height,
                None,
                "screenshots_capture",
                None,
                None,
            )
        };
        let capture_ms = capture_started_at.elapsed().as_millis();

        let (scale_factor, capture_width, capture_height) = derive_legacy_capture_metrics(
            captured.monitors.as_slice(),
            captured.desktop.width,
            captured.desktop.height,
        );

        let session_build_started_at = Instant::now();
        let session = ActiveScreenshotSession {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: Utc::now().to_rfc3339(),
            display_x: captured.desktop.x,
            display_y: captured.desktop.y,
            display_width: captured.desktop.width,
            display_height: captured.desktop.height,
            scale_factor,
            capture_width,
            capture_height,
            image_status: match preview_transport {
                ScreenshotPreviewTransport::RawRgbaFast => ScreenshotImageStatus::Ready,
                ScreenshotPreviewTransport::File => ScreenshotImageStatus::Loading,
            },
            image_error: None,
            image_data_url: Arc::new(String::new()),
            preview_image_path: None,
            preview_protocol_bytes: preview_protocol_bytes.map(Arc::new),
            preview_transport,
            preview_pixel_width,
            preview_pixel_height,
            monitors: Arc::new(captured.monitors),
        };
        let session_build_ms = session_build_started_at.elapsed().as_millis();
        let raw_rgba_bytes = session
            .monitors
            .iter()
            .map(|monitor| {
                u64::from(monitor.capture_width)
                    .saturating_mul(u64::from(monitor.capture_height))
                    .saturating_mul(4)
            })
            .sum::<u64>();

        let state_store_started_at = Instant::now();
        self.replace_active_session(session.clone())?;
        let state_store_ms = state_store_started_at.elapsed().as_millis();
        let overlay_prewarmed = self.is_overlay_prewarmed()?;
        let overlay_focus_drift_compensation =
            self.get_overlay_focus_drift_compensation(&session)?;

        let emit_loading_started_at = Instant::now();
        let overlay_window = prepare_overlay_window(app, &session)?;
        emit_session_updated(app, &session);
        let emit_loading_ms = emit_loading_started_at.elapsed().as_millis();

        let overlay_started_at = Instant::now();
        self.suppress_overlay_window_events(Duration::from_millis(OVERLAY_EVENT_SUPPRESS_MS))?;
        let overlay_activation = move_and_focus_overlay_window(
            &overlay_window,
            &session,
            overlay_prewarmed,
            overlay_focus_drift_compensation,
        )?;
        if let Some(probe) = overlay_activation.observed_focus_drift {
            self.update_overlay_focus_drift_compensation(&session, &probe)?;
        }
        let overlay_ready_ms = overlay_started_at.elapsed().as_millis();

        let spawn_preview_started_at = Instant::now();
        let preview_spawn_ms =
            if matches!(session.preview_transport, ScreenshotPreviewTransport::File) {
                self.spawn_preview_preparation(app, session.clone());
                spawn_preview_started_at.elapsed().as_millis()
            } else {
                0
            };

        log::info!(
            target: "bexo::service::screenshot",
            "start_session_completed session_id={} monitors={} hide_overlay_ms={} capture_ms={} capture_strategy={} live_capture_available={} frame_age_ms={} live_capture_sequence={} session_build_ms={} state_store_ms={} overlay_ready_ms={} overlay_prewarmed={} emit_loading_ms={} preview_spawn_ms={} total_ms={} display={}x{} display_origin=({}, {}) display_unit=logical_px legacy_capture={}x{} legacy_scale_factor={} raw_rgba_bytes={} preview_transport={:?} preview_pixels={}x{} overlay_show={}",
            session.id,
            session.monitors.len(),
            hide_overlay_ms,
            capture_ms,
            capture_strategy,
            live_capture_available,
            frame_age_ms
                .map(|value| value.to_string())
                .unwrap_or_else(|| "na".to_string()),
            live_capture_sequence
                .map(|value| value.to_string())
                .unwrap_or_else(|| "na".to_string()),
            session_build_ms,
            state_store_ms,
            overlay_ready_ms,
            overlay_prewarmed,
            emit_loading_ms,
            preview_spawn_ms,
            started_at.elapsed().as_millis(),
            session.display_width,
            session.display_height,
            session.display_x,
            session.display_y,
            session.capture_width,
            session.capture_height,
            session.scale_factor,
            raw_rgba_bytes,
            session.preview_transport,
            session.preview_pixel_width,
            session.preview_pixel_height,
            match session.preview_transport {
                ScreenshotPreviewTransport::RawRgbaFast => "immediate_overlay_raw_ready",
                ScreenshotPreviewTransport::File => "immediate_overlay_background_async",
            }
        );

        Ok(StartScreenshotSessionResult {
            session_id: session.id,
            window_label: SCREENSHOT_OVERLAY_WINDOW_LABEL.to_string(),
        })
    }

    pub fn get_active_session(&self) -> AppResult<ScreenshotSessionView> {
        let started_at = Instant::now();
        let session = self.require_active_session(None)?;
        let view_started_at = Instant::now();
        let view = session_to_view(&session);
        let view_build_ms = view_started_at.elapsed().as_millis();
        let image_data_url_bytes = view.image_data_url.len();
        let preview_image_path = view.preview_image_path.as_deref().unwrap_or("");
        let preview_transport = view.preview_transport;
        log::info!(
            target: "bexo::service::screenshot",
            "get_active_session_completed session_id={} image_status={:?} image_data_url_bytes={} preview_image_path={} preview_transport={:?} preview_pixels={}x{} monitors={} view_build_ms={} total_ms={}",
            view.session_id,
            view.image_status,
            image_data_url_bytes,
            preview_image_path,
            preview_transport,
            view.preview_pixel_width,
            view.preview_pixel_height,
            view.monitors.len(),
            view_build_ms,
            started_at.elapsed().as_millis()
        );
        Ok(view)
    }

    pub fn get_preview_rgba(&self, session_id: &str) -> AppResult<Vec<u8>> {
        let started_at = Instant::now();
        let session = self.require_active_session(Some(session_id))?;
        if !matches!(
            session.preview_transport,
            ScreenshotPreviewTransport::RawRgbaFast
        ) {
            return Err(AppError::new(
                "SCREENSHOT_PREVIEW_TRANSPORT_UNAVAILABLE",
                "当前截图会话不支持原始预览快路径",
            )
            .with_detail("sessionId", session_id.to_string()));
        }

        let monitor = session.monitors.first().ok_or_else(|| {
            AppError::new(
                "SCREENSHOT_PREVIEW_MONITOR_NOT_FOUND",
                "截图预览监视器不存在",
            )
            .with_detail("sessionId", session_id.to_string())
        })?;
        let bytes = monitor.source_image()?.as_raw().clone();
        log::info!(
            target: "bexo::service::screenshot",
            "get_preview_rgba_completed session_id={} bytes={} preview_pixels={}x{} total_ms={}",
            session.id,
            bytes.len(),
            session.preview_pixel_width,
            session.preview_pixel_height,
            started_at.elapsed().as_millis()
        );
        Ok(bytes)
    }

    pub fn get_preview_protocol_bmp(&self, session_id: &str) -> AppResult<Vec<u8>> {
        let started_at = Instant::now();
        let session = self.require_active_session(Some(session_id))?;
        if !matches!(
            session.preview_transport,
            ScreenshotPreviewTransport::RawRgbaFast
        ) {
            return Err(AppError::new(
                "SCREENSHOT_PREVIEW_PROTOCOL_UNAVAILABLE",
                "当前截图会话不支持协议直供预览",
            )
            .with_detail("sessionId", session_id.to_string()));
        }

        let monitor = session.monitors.first().ok_or_else(|| {
            AppError::new(
                "SCREENSHOT_PREVIEW_MONITOR_NOT_FOUND",
                "截图预览监视器不存在",
            )
            .with_detail("sessionId", session_id.to_string())
        })?;

        let encode_started_at = Instant::now();
        let (bytes, encode_path) = if let Some(cached) = session.preview_protocol_bytes.as_ref() {
            (cached.as_ref().clone(), "cached")
        } else {
            (
                encode_preview_bmp_fast(monitor.source_image()?)?,
                "bmp_fast",
            )
        };
        let encode_ms = encode_started_at.elapsed().as_millis();
        log::info!(
            target: "bexo::service::screenshot",
            "get_preview_protocol_bmp_completed session_id={} encode_ms={} bytes={} preview_pixels={}x{} total_ms={} encode_path={}",
            session.id,
            encode_ms,
            bytes.len(),
            session.preview_pixel_width,
            session.preview_pixel_height,
            started_at.elapsed().as_millis(),
            encode_path
        );

        Ok(bytes)
    }

    pub fn enforce_overlay_window_geometry<R: Runtime>(
        &self,
        window: &tauri::WebviewWindow<R>,
        trigger: &str,
    ) -> AppResult<bool> {
        if self.should_ignore_overlay_window_event()? {
            return Ok(false);
        }

        let Some(session) = self.get_active_session_optional()? else {
            return Ok(false);
        };

        let probe = probe_overlay_geometry(window, &session)?;
        if probe.is_aligned() {
            return Ok(false);
        }

        log::warn!(
            target: "bexo::service::screenshot",
            "overlay_geometry_drift_detected trigger={} session_id={} current_logical={}x{}@{},{} target_logical={}x{}@{},{} delta=({}, {}, {}, {}) outer_physical={}x{}@{},{} scale_factor={:.4}",
            trigger,
            session.id,
            probe.current_width,
            probe.current_height,
            probe.current_x,
            probe.current_y,
            session.display_width,
            session.display_height,
            session.display_x,
            session.display_y,
            probe.delta_x,
            probe.delta_y,
            probe.delta_width,
            probe.delta_height,
            probe.outer_width,
            probe.outer_height,
            probe.outer_x,
            probe.outer_y,
            probe.scale_factor
        );

        set_overlay_window_geometry(window, &session)?;
        Ok(true)
    }

    pub fn get_selection_render(
        &self,
        session_id: &str,
        selection: ScreenshotSelectionInput,
    ) -> AppResult<ScreenshotSelectionRenderView> {
        let session = self.require_active_session(Some(session_id))?;
        let compose_started_at = Instant::now();
        let render = render_selection_base_image(&session, selection)?;
        let compose_ms = compose_started_at.elapsed().as_millis();

        let encode_started_at = Instant::now();
        let png_bytes = encode_png(&render.image)?;
        let encode_ms = encode_started_at.elapsed().as_millis();

        let base64_started_at = Instant::now();
        let image_data_url = format!(
            "data:image/png;base64,{}",
            BASE64_STANDARD.encode(png_bytes.as_slice())
        );
        let base64_ms = base64_started_at.elapsed().as_millis();

        log::info!(
            target: "bexo::service::screenshot",
            "selection_render_ready session_id={} mode={:?} compose_ms={} encode_ms={} base64_ms={} width={} height={} scale_factor={}",
            session.id,
            render.mode,
            compose_ms,
            encode_ms,
            base64_ms,
            render.image.width(),
            render.image.height(),
            render.scale_factor
        );

        Ok(ScreenshotSelectionRenderView {
            session_id: session.id,
            width: render.image.width(),
            height: render.image.height(),
            scale_factor: render.scale_factor,
            render_mode: render.mode,
            image_data_url,
            tiles: render.tiles,
        })
    }

    pub fn copy_selection(
        &self,
        session_id: &str,
        selection: ScreenshotSelectionInput,
        rendered_image: Option<ScreenshotRenderedImageInput>,
    ) -> AppResult<CopyScreenshotSelectionResult> {
        let session = self.require_active_session(Some(session_id))?;
        let cropped = if let Some(rendered_image) = rendered_image {
            decode_rendered_png_to_rgba(&rendered_image)?
        } else {
            render_selection_base_image(&session, selection)?.image
        };
        let width = cropped.width();
        let height = cropped.height();

        let mut clipboard = Clipboard::new().map_err(|error| {
            AppError::new("SCREENSHOT_CLIPBOARD_UNAVAILABLE", "剪贴板不可用")
                .with_detail("reason", error.to_string())
        })?;
        clipboard
            .set_image(ImageData {
                width: width as usize,
                height: height as usize,
                bytes: Cow::Owned(cropped.into_raw()),
            })
            .map_err(|error| {
                AppError::new("SCREENSHOT_CLIPBOARD_FAILED", "复制截图到剪贴板失败")
                    .with_detail("reason", error.to_string())
            })?;

        let _ = self.clear_active_session(Some(session_id));

        Ok(CopyScreenshotSelectionResult {
            session_id: session.id,
            width,
            height,
        })
    }

    pub fn save_selection<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        session_id: &str,
        selection: ScreenshotSelectionInput,
        file_path: Option<String>,
        rendered_image: Option<ScreenshotRenderedImageInput>,
    ) -> AppResult<SaveScreenshotSelectionResult> {
        let session = self.require_active_session(Some(session_id))?;
        let (width, height, encoded) = if let Some(rendered_image) = rendered_image {
            let bytes = decode_rendered_png_bytes(&rendered_image)?;
            let image = decode_png_to_rgba(bytes.as_slice())?;
            (image.width(), image.height(), bytes)
        } else {
            let render = render_selection_base_image(&session, selection)?;
            let width = render.image.width();
            let height = render.image.height();
            let encoded = encode_png(&render.image)?;
            (width, height, encoded)
        };

        let output_path = resolve_output_path(app, file_path)?;
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                AppError::new("SCREENSHOT_SAVE_DIR_FAILED", "创建截图保存目录失败")
                    .with_detail("path", parent.display().to_string())
                    .with_detail("reason", error.to_string())
            })?;
        }

        fs::write(&output_path, encoded).map_err(|error| {
            AppError::new("SCREENSHOT_SAVE_FAILED", "保存截图失败")
                .with_detail("path", output_path.display().to_string())
                .with_detail("reason", error.to_string())
        })?;

        let _ = self.clear_active_session(Some(session_id));

        Ok(SaveScreenshotSelectionResult {
            session_id: session.id,
            file_path: output_path.display().to_string(),
            width,
            height,
        })
    }

    pub fn cancel_session(&self, session_id: &str) -> AppResult<CancelScreenshotSessionResult> {
        let cancelled = self.clear_active_session(Some(session_id))?;
        Ok(CancelScreenshotSessionResult {
            session_id: session_id.to_string(),
            cancelled,
        })
    }

    pub fn clear_active_session(&self, session_id: Option<&str>) -> AppResult<bool> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| AppError::new("SCREENSHOT_SESSION_LOCK_FAILED", "读取截图会话状态失败"))?;

        let Some(active) = guard.active_session.as_ref() else {
            return Ok(false);
        };

        if let Some(expected) = session_id {
            if !active.id.eq_ignore_ascii_case(expected) {
                return Ok(false);
            }
        }

        let stale_preview_path = active
            .preview_image_path
            .as_ref()
            .map(|value| value.as_ref().clone());
        guard.active_session = None;
        drop(guard);
        cleanup_preview_file(stale_preview_path.as_deref());
        Ok(true)
    }

    fn is_overlay_prewarmed(&self) -> AppResult<bool> {
        let guard = self.state.lock().map_err(|_| {
            AppError::new("SCREENSHOT_SESSION_LOCK_FAILED", "读取截图窗口预热状态失败")
        })?;
        Ok(guard.overlay_prewarmed)
    }

    fn set_overlay_prewarmed(&self, value: bool) -> AppResult<()> {
        let mut guard = self.state.lock().map_err(|_| {
            AppError::new("SCREENSHOT_SESSION_LOCK_FAILED", "更新截图窗口预热状态失败")
        })?;
        guard.overlay_prewarmed = value;
        Ok(())
    }

    fn get_overlay_focus_drift_compensation(
        &self,
        session: &ActiveScreenshotSession,
    ) -> AppResult<Option<OverlayFocusDriftCompensation>> {
        let guard = self.state.lock().map_err(|_| {
            AppError::new("SCREENSHOT_SESSION_LOCK_FAILED", "读取截图窗口漂移补偿失败")
        })?;
        Ok(guard
            .overlay_focus_drift_compensation
            .filter(|value| value.matches_session(session)))
    }

    fn update_overlay_focus_drift_compensation(
        &self,
        session: &ActiveScreenshotSession,
        probe: &OverlayGeometryProbe,
    ) -> AppResult<()> {
        if !probe.is_size_aligned() || probe.delta_x.abs() > 4 || probe.delta_y.abs() > 4 {
            return Ok(());
        }

        let compensation =
            OverlayFocusDriftCompensation::for_session(session, -probe.delta_x, -probe.delta_y);

        let mut guard = self.state.lock().map_err(|_| {
            AppError::new("SCREENSHOT_SESSION_LOCK_FAILED", "更新截图窗口漂移补偿失败")
        })?;
        let changed = guard
            .overlay_focus_drift_compensation
            .map(|existing| {
                existing.offset_x != compensation.offset_x
                    || existing.offset_y != compensation.offset_y
                    || !existing.matches_session(session)
            })
            .unwrap_or(true);
        guard.overlay_focus_drift_compensation = Some(compensation);
        drop(guard);

        if changed {
            log::info!(
                target: "bexo::service::screenshot",
                "overlay_focus_drift_compensation_updated session_id={} offset=({}, {}) scale_factor={} display={}x{}",
                session.id,
                compensation.offset_x,
                compensation.offset_y,
                session.scale_factor,
                session.display_width,
                session.display_height
            );
        }

        Ok(())
    }

    fn suppress_overlay_window_events(&self, duration: Duration) -> AppResult<()> {
        let mut guard = self.state.lock().map_err(|_| {
            AppError::new(
                "SCREENSHOT_SESSION_LOCK_FAILED",
                "更新截图窗口事件抑制状态失败",
            )
        })?;
        guard.overlay_event_suppressed_until = Some(Instant::now() + duration);
        Ok(())
    }

    pub fn should_ignore_overlay_window_event(&self) -> AppResult<bool> {
        let mut guard = self.state.lock().map_err(|_| {
            AppError::new(
                "SCREENSHOT_SESSION_LOCK_FAILED",
                "读取截图窗口事件抑制状态失败",
            )
        })?;

        let Some(until) = guard.overlay_event_suppressed_until else {
            return Ok(false);
        };

        if Instant::now() <= until {
            return Ok(true);
        }

        guard.overlay_event_suppressed_until = None;
        Ok(false)
    }

    fn store_live_capture_snapshot(&self, snapshot: LiveCaptureSnapshot) -> AppResult<bool> {
        {
            let state_guard = self.state.lock().map_err(|_| {
                AppError::new(
                    "SCREENSHOT_LIVE_CAPTURE_LOCK_FAILED",
                    "读取截图会话状态失败",
                )
            })?;
            if state_guard.active_session.is_some() {
                return Ok(false);
            }
        }

        let mut guard = self.live_capture.lock().map_err(|_| {
            AppError::new(
                "SCREENSHOT_LIVE_CAPTURE_LOCK_FAILED",
                "更新持续截图状态失败",
            )
        })?;
        guard.latest_snapshot = Some(Arc::new(snapshot));
        Ok(true)
    }

    #[cfg(target_os = "windows")]
    #[allow(clippy::too_many_arguments)]
    fn store_live_capture_frame_snapshot_windows(
        &self,
        backend: LiveCaptureBackend,
        display_id: u32,
        monitor_handle: isize,
        display_x: i32,
        display_y: i32,
        display_width: u32,
        display_height: u32,
        scale_factor: f32,
        preview_pixel_width: u32,
        preview_pixel_height: u32,
        sequence: u64,
        captured_at: Instant,
        capture_width: u32,
        capture_height: u32,
        bgra_top_down: Vec<u8>,
    ) -> AppResult<bool> {
        let preview_protocol_bytes = match build_preview_bmp_from_monitor_handle_windows(
            display_id,
            monitor_handle,
            capture_width.max(1),
            capture_height.max(1),
            bgra_top_down.as_slice(),
            preview_pixel_width,
            preview_pixel_height,
        ) {
            Ok(bytes) => Some(Arc::new(bytes)),
            Err(error) => {
                log::warn!(
                    target: "bexo::service::screenshot",
                    "live_capture_preview_prepare_failed backend={} display_id={} sequence={} reason={}",
                    backend.as_str(),
                    display_id,
                    sequence,
                    error
                );
                None
            }
        };

        let snapshot = LiveCaptureSnapshot {
            backend,
            sequence,
            captured_at,
            display_id,
            monitor_handle,
            display_x,
            display_y,
            display_width,
            display_height,
            capture_width: capture_width.max(1),
            capture_height: capture_height.max(1),
            scale_factor,
            preview_pixel_width,
            preview_pixel_height,
            bgra_top_down: Arc::new(bgra_top_down),
            preview_protocol_bytes,
        };

        self.store_live_capture_snapshot(snapshot)
    }

    fn get_live_capture_snapshot(&self) -> AppResult<Option<Arc<LiveCaptureSnapshot>>> {
        let guard = self.live_capture.lock().map_err(|_| {
            AppError::new(
                "SCREENSHOT_LIVE_CAPTURE_LOCK_FAILED",
                "读取持续截图状态失败",
            )
        })?;
        Ok(guard.latest_snapshot.clone())
    }

    #[cfg(target_os = "windows")]
    fn try_capture_from_live_cache(&self) -> AppResult<Option<(FastPreviewCapture, u128, u64)>> {
        let Some(snapshot) = self.get_live_capture_snapshot()? else {
            return Ok(None);
        };

        let frame_age_ms = snapshot.captured_at.elapsed().as_millis();
        if frame_age_ms > u128::from(LIVE_CAPTURE_MAX_FRAME_AGE_MS) {
            log::info!(
                target: "bexo::service::screenshot",
                "live_capture_snapshot_skipped display_id={} sequence={} frame_age_ms={} reason=stale_frame",
                snapshot.display_id,
                snapshot.sequence,
                frame_age_ms
            );
            return Ok(None);
        }

        let (preview_protocol_bytes, preview_source) =
            if let Some(cached) = snapshot.preview_protocol_bytes.as_ref() {
                (cached.as_ref().clone(), "cached")
            } else {
                (
                    build_preview_bmp_from_monitor_handle_windows(
                        snapshot.display_id,
                        snapshot.monitor_handle,
                        snapshot.capture_width,
                        snapshot.capture_height,
                        snapshot.bgra_top_down.as_slice(),
                        snapshot.preview_pixel_width,
                        snapshot.preview_pixel_height,
                    )?,
                    "hot_path_fallback",
                )
            };
        let captured = build_captured_virtual_desktop_from_live_snapshot(snapshot.as_ref());

        log::info!(
            target: "bexo::service::screenshot",
            "live_capture_snapshot_used backend={} display_id={} sequence={} frame_age_ms={} raw_pixels={}x{} preview_pixels={}x{} preview_bytes={} preview_source={}",
            snapshot.backend.as_str(),
            snapshot.display_id,
            snapshot.sequence,
            frame_age_ms,
            snapshot.capture_width,
            snapshot.capture_height,
            snapshot.preview_pixel_width,
            snapshot.preview_pixel_height,
            preview_protocol_bytes.len(),
            preview_source
        );

        Ok(Some((
            FastPreviewCapture {
                captured,
                preview_protocol_bytes,
                preview_pixel_width: snapshot.preview_pixel_width,
                preview_pixel_height: snapshot.preview_pixel_height,
                capture_strategy: snapshot.backend.capture_strategy(),
            },
            frame_age_ms,
            snapshot.sequence,
        )))
    }

    #[cfg(target_os = "windows")]
    fn wait_for_live_capture_snapshot(
        &self,
        timeout: Duration,
    ) -> AppResult<Option<(FastPreviewCapture, u128, u64)>> {
        let started_at = Instant::now();
        loop {
            if let Some(capture) = self.try_capture_from_live_cache()? {
                let waited_ms = started_at.elapsed().as_millis();
                if waited_ms > 0 {
                    log::info!(
                        target: "bexo::service::screenshot",
                        "live_capture_snapshot_wait_succeeded waited_ms={}",
                        waited_ms
                    );
                }
                return Ok(Some(capture));
            }

            if started_at.elapsed() >= timeout {
                log::info!(
                    target: "bexo::service::screenshot",
                    "live_capture_snapshot_wait_timed_out timeout_ms={}",
                    timeout.as_millis()
                );
                return Ok(None);
            }

            std::thread::sleep(Duration::from_millis(LIVE_CAPTURE_WAIT_POLL_INTERVAL_MS));
        }
    }

    fn require_active_session(
        &self,
        session_id: Option<&str>,
    ) -> AppResult<ActiveScreenshotSession> {
        let guard = self
            .state
            .lock()
            .map_err(|_| AppError::new("SCREENSHOT_SESSION_LOCK_FAILED", "读取截图会话状态失败"))?;

        let session = guard
            .active_session
            .as_ref()
            .ok_or_else(|| AppError::new("SCREENSHOT_SESSION_NOT_FOUND", "截图会话不存在"))?;

        if let Some(expected) = session_id {
            if !session.id.eq_ignore_ascii_case(expected) {
                return Err(
                    AppError::new("SCREENSHOT_SESSION_EXPIRED", "截图会话已过期")
                        .with_detail("sessionId", expected.to_string()),
                );
            }
        }

        Ok(session.clone())
    }

    fn replace_active_session(&self, session: ActiveScreenshotSession) -> AppResult<()> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| AppError::new("SCREENSHOT_SESSION_LOCK_FAILED", "更新截图会话状态失败"))?;
        let stale_preview_path = guard
            .active_session
            .as_ref()
            .and_then(|active| active.preview_image_path.as_ref())
            .map(|value| value.as_ref().clone());
        guard.active_session = Some(session);
        drop(guard);
        cleanup_preview_file(stale_preview_path.as_deref());
        Ok(())
    }

    fn get_active_session_optional(&self) -> AppResult<Option<ActiveScreenshotSession>> {
        let guard = self
            .state
            .lock()
            .map_err(|_| AppError::new("SCREENSHOT_SESSION_LOCK_FAILED", "读取截图会话状态失败"))?;
        Ok(guard.active_session.clone())
    }

    fn update_active_session_if_current<F>(
        &self,
        session_id: &str,
        updater: F,
    ) -> AppResult<Option<ActiveScreenshotSession>>
    where
        F: FnOnce(&mut ActiveScreenshotSession),
    {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| AppError::new("SCREENSHOT_SESSION_LOCK_FAILED", "更新截图会话状态失败"))?;

        let Some(active) = guard.active_session.as_mut() else {
            return Ok(None);
        };

        if !active.id.eq_ignore_ascii_case(session_id) {
            return Ok(None);
        }

        updater(active);
        Ok(Some(active.clone()))
    }

    fn spawn_preview_preparation<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        session: ActiveScreenshotSession,
    ) {
        let service = self.clone();
        let app_handle = app.clone();
        std::thread::spawn(move || {
            let preview_started_at = Instant::now();
            let result = prepare_preview_image_data_url(&app_handle, &session);
            if let Err(error) = &result {
                log::error!(
                    target: "bexo::service::screenshot",
                    "preview_preparation_failed session_id={} total_ms={} reason={}",
                    session.id,
                    preview_started_at.elapsed().as_millis(),
                    error
                );
            }
            let _ = service.finish_preview_preparation(&app_handle, session.id.as_str(), result);
        });
    }

    fn finish_preview_preparation<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        session_id: &str,
        result: AppResult<PreparedPreviewImage>,
    ) -> AppResult<()> {
        let finish_started_at = Instant::now();
        let previous_preview_path = self
            .get_active_session_optional()?
            .filter(|session| session.id.eq_ignore_ascii_case(session_id))
            .and_then(|session| {
                session
                    .preview_image_path
                    .map(|value| value.as_ref().clone())
            });
        let orphan_preview_path = match &result {
            Ok(prepared) => prepared.preview_image_path.clone(),
            Err(_) => None,
        };
        let update_state_started_at = Instant::now();
        let updated = self.update_active_session_if_current(session_id, |session| match result {
            Ok(prepared) => {
                session.image_status = ScreenshotImageStatus::Ready;
                session.image_error = None;
                session.image_data_url = Arc::new(prepared.image_data_url);
                session.preview_image_path = prepared.preview_image_path.map(Arc::new);
                session.preview_protocol_bytes = None;
                log::info!(
                    target: "bexo::service::screenshot",
                    "preview_payload_applied session_id={} width={} height={} encoded_bytes={} preview_mode={} encode_path={} encoded_format={}",
                    session.id,
                    prepared.width,
                    prepared.height,
                    prepared.encoded_bytes,
                    prepared.preview_mode,
                    prepared.encode_path,
                    prepared.encoded_format
                );
            }
            Err(error) => {
                session.preview_image_path = None;
                session.preview_protocol_bytes = None;
                session.image_status = ScreenshotImageStatus::Failed;
                session.image_error = Some(error.message);
                session.image_data_url = Arc::new(String::new());
            }
        })?;
        let update_state_ms = update_state_started_at.elapsed().as_millis();

        if let Some(session) = updated {
            let current_preview_path = session
                .preview_image_path
                .as_ref()
                .map(|value| value.as_ref().clone());
            if previous_preview_path != current_preview_path {
                cleanup_preview_file(previous_preview_path.as_deref());
            }
            let emit_started_at = Instant::now();
            emit_session_updated(app, &session);
            let emit_event_ms = emit_started_at.elapsed().as_millis();
            log::info!(
                target: "bexo::service::screenshot",
                "finish_preview_preparation_completed session_id={} image_status={:?} image_data_url_bytes={} preview_image_path={} update_state_ms={} emit_event_ms={} total_ms={}",
                session.id,
                session.image_status,
                session.image_data_url.len(),
                session
                    .preview_image_path
                    .as_ref()
                    .map(|value| value.as_str())
                    .unwrap_or(""),
                update_state_ms,
                emit_event_ms,
                finish_started_at.elapsed().as_millis()
            );
        } else {
            cleanup_preview_file(orphan_preview_path.as_deref());
            log::warn!(
                target: "bexo::service::screenshot",
                "finish_preview_preparation_dropped session_id={} orphan_preview_path={} update_state_ms={} total_ms={}",
                session_id,
                orphan_preview_path.as_deref().unwrap_or(""),
                update_state_ms,
                finish_started_at.elapsed().as_millis()
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_monitor_display, MonitorDisplayCoordinateSpace, TauriMonitorSnapshot};

    #[test]
    fn keep_logical_display_when_capture_matches_reported_scale() {
        let normalized = normalize_monitor_display(0, 0, 1280, 720, 1920, 1080, 1.5, None);
        assert_eq!(
            normalized.coordinate_space,
            MonitorDisplayCoordinateSpace::Logical
        );
        assert_eq!(normalized.display_width, 1280);
        assert_eq!(normalized.display_height, 720);
    }

    #[test]
    fn convert_physical_display_when_capture_scale_is_unity_but_reported_scaled() {
        let normalized = normalize_monitor_display(0, 0, 1920, 1080, 1920, 1080, 1.5, None);
        assert_eq!(
            normalized.coordinate_space,
            MonitorDisplayCoordinateSpace::PhysicalConverted
        );
        assert_eq!(normalized.display_width, 1280);
        assert_eq!(normalized.display_height, 720);
    }

    #[test]
    fn prefer_tauri_monitor_logical_metrics_when_available() {
        let tauri_monitor = TauriMonitorSnapshot {
            physical_x: 0,
            physical_y: 0,
            physical_width: 3840,
            physical_height: 2160,
            logical_x: 0,
            logical_y: 0,
            logical_width: 1920,
            logical_height: 1080,
            scale_factor: 2.0,
        };
        let normalized =
            normalize_monitor_display(0, 0, 3840, 2160, 3840, 2160, 1.0, Some(tauri_monitor));
        assert_eq!(
            normalized.coordinate_space,
            MonitorDisplayCoordinateSpace::TauriLogical
        );
        assert_eq!(normalized.display_width, 1920);
        assert_eq!(normalized.display_height, 1080);
        assert_eq!(normalized.reported_scale_factor, 2.0);
    }
}
