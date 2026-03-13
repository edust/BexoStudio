use std::{
    borrow::Cow,
    fs,
    io::Cursor,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use arboard::{Clipboard, ImageData};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use chrono::{Local, Utc};
use screenshots::{
    display_info::DisplayInfo,
    image::{
        imageops::{self, FilterType},
        DynamicImage, ImageOutputFormat, Rgba, RgbaImage,
    },
    Screen,
};
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Position, Runtime, Size, WebviewUrl,
    WebviewWindowBuilder,
};

use crate::{
    domain::{
        CancelScreenshotSessionResult, CopyScreenshotSelectionResult,
        SaveScreenshotSelectionResult, ScreenshotRenderedImageInput, ScreenshotSelectionInput,
        ScreenshotSessionUpdatedEvent, ScreenshotSessionView, StartScreenshotSessionResult,
        SCREENSHOT_OVERLAY_WINDOW_LABEL, SCREENSHOT_SESSION_UPDATED_EVENT_NAME,
    },
    error::{AppError, AppResult},
};

const SCREENSHOT_OVERLAY_URL: &str = "index.html?overlay=screenshot";

#[derive(Debug, Clone)]
pub struct ScreenshotService {
    state: Arc<Mutex<ScreenshotState>>,
}

#[derive(Debug, Default)]
struct ScreenshotState {
    active_session: Option<ActiveScreenshotSession>,
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
    rgba_bytes: Arc<Vec<u8>>,
    image_data_url: Arc<String>,
}

#[derive(Debug, Clone, Copy)]
struct PixelSelection {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Copy)]
struct VirtualDesktopInfo {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[derive(Debug)]
struct CapturedScreenFrame {
    display_info: DisplayInfo,
    image: RgbaImage,
}

impl ScreenshotService {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(ScreenshotState::default())),
        }
    }

    pub fn start_session<R: Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> AppResult<StartScreenshotSessionResult> {
        hide_overlay_if_visible(app);

        let (captured, desktop_info) = capture_virtual_desktop()?;
        let capture_width = captured.width();
        let capture_height = captured.height();

        let png_bytes = encode_png(&captured)?;
        let image_data_url = format!(
            "data:image/png;base64,{}",
            BASE64_STANDARD.encode(png_bytes.as_slice())
        );

        let session = ActiveScreenshotSession {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: Utc::now().to_rfc3339(),
            display_x: desktop_info.x,
            display_y: desktop_info.y,
            display_width: desktop_info.width,
            display_height: desktop_info.height,
            scale_factor: 1.0,
            capture_width,
            capture_height,
            rgba_bytes: Arc::new(captured.into_raw()),
            image_data_url: Arc::new(image_data_url),
        };

        self.replace_active_session(session.clone())?;
        ensure_overlay_window(app, &session)?;
        emit_session_updated(app, &session);

        Ok(StartScreenshotSessionResult {
            session_id: session.id,
            window_label: SCREENSHOT_OVERLAY_WINDOW_LABEL.to_string(),
        })
    }

    pub fn get_active_session(&self) -> AppResult<ScreenshotSessionView> {
        let session = self.require_active_session(None)?;
        Ok(session_to_view(&session))
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
            let pixel_selection = normalize_selection(&session, selection)?;
            crop_selection(&session, pixel_selection)?
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
            let pixel_selection = normalize_selection(&session, selection)?;
            let cropped = crop_selection(&session, pixel_selection)?;
            let width = cropped.width();
            let height = cropped.height();
            let encoded = encode_png(&cropped)?;
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

        guard.active_session = None;
        Ok(true)
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
        guard.active_session = Some(session);
        Ok(())
    }
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

fn capture_virtual_desktop() -> AppResult<(RgbaImage, VirtualDesktopInfo)> {
    let screens = Screen::all().map_err(|error| {
        AppError::new("SCREENSHOT_SCREEN_ENUM_FAILED", "读取显示器信息失败")
            .with_detail("reason", error.to_string())
    })?;

    if screens.is_empty() {
        return Err(AppError::new(
            "SCREENSHOT_SCREEN_NOT_FOUND",
            "未找到可用显示器",
        ));
    }

    let mut frames: Vec<CapturedScreenFrame> = Vec::with_capacity(screens.len());
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;

    for screen in screens {
        let display_info = screen.display_info;
        let image = screen.capture().map_err(|error| {
            AppError::new("SCREENSHOT_CAPTURE_FAILED", "屏幕截图失败")
                .with_detail("reason", error.to_string())
                .with_detail("displayId", display_info.id.to_string())
        })?;

        let right = display_info.x + display_info.width as i32;
        let bottom = display_info.y + display_info.height as i32;
        min_x = min_x.min(display_info.x);
        min_y = min_y.min(display_info.y);
        max_x = max_x.max(right);
        max_y = max_y.max(bottom);

        frames.push(CapturedScreenFrame {
            display_info,
            image,
        });
    }

    if max_x <= min_x || max_y <= min_y {
        return Err(AppError::new(
            "SCREENSHOT_SCREEN_LAYOUT_INVALID",
            "显示器布局异常，无法创建截图会话",
        ));
    }

    let virtual_width = (max_x - min_x) as u32;
    let virtual_height = (max_y - min_y) as u32;
    let mut virtual_image =
        RgbaImage::from_pixel(virtual_width, virtual_height, Rgba([0, 0, 0, 255]));

    for frame in frames {
        let logical_width = frame.display_info.width.max(1);
        let logical_height = frame.display_info.height.max(1);
        let screen_image =
            if frame.image.width() == logical_width && frame.image.height() == logical_height {
                frame.image
            } else {
                imageops::resize(
                    &frame.image,
                    logical_width,
                    logical_height,
                    FilterType::Triangle,
                )
            };

        let offset_x = i64::from(frame.display_info.x - min_x);
        let offset_y = i64::from(frame.display_info.y - min_y);
        imageops::overlay(&mut virtual_image, &screen_image, offset_x, offset_y);
    }

    Ok((
        virtual_image,
        VirtualDesktopInfo {
            x: min_x,
            y: min_y,
            width: virtual_width,
            height: virtual_height,
        },
    ))
}

fn ensure_overlay_window<R: Runtime>(
    app: &AppHandle<R>,
    session: &ActiveScreenshotSession,
) -> AppResult<()> {
    if let Some(window) = app.get_webview_window(SCREENSHOT_OVERLAY_WINDOW_LABEL) {
        move_and_focus_overlay_window(&window, session)?;
        return Ok(());
    }

    let window = WebviewWindowBuilder::new(
        app,
        SCREENSHOT_OVERLAY_WINDOW_LABEL,
        WebviewUrl::App(SCREENSHOT_OVERLAY_URL.into()),
    )
    .title("Bexo Studio Screenshot")
    .inner_size(
        f64::from(session.display_width),
        f64::from(session.display_height),
    )
    .position(f64::from(session.display_x), f64::from(session.display_y))
    .decorations(false)
    .resizable(false)
    .transparent(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(true)
    .focused(true)
    .maximizable(false)
    .minimizable(false)
    .shadow(false)
    .build()
    .map_err(|error| {
        AppError::new("SCREENSHOT_OVERLAY_CREATE_FAILED", "创建截图窗口失败")
            .with_detail("reason", error.to_string())
    })?;

    move_and_focus_overlay_window(&window, session)
}

fn move_and_focus_overlay_window<R: Runtime>(
    window: &tauri::WebviewWindow<R>,
    session: &ActiveScreenshotSession,
) -> AppResult<()> {
    window
        .set_position(Position::Logical(LogicalPosition::new(
            f64::from(session.display_x),
            f64::from(session.display_y),
        )))
        .map_err(|error| {
            AppError::new("SCREENSHOT_OVERLAY_POSITION_FAILED", "定位截图窗口失败")
                .with_detail("reason", error.to_string())
        })?;
    window
        .set_size(Size::Logical(LogicalSize::new(
            f64::from(session.display_width),
            f64::from(session.display_height),
        )))
        .map_err(|error| {
            AppError::new("SCREENSHOT_OVERLAY_RESIZE_FAILED", "调整截图窗口尺寸失败")
                .with_detail("reason", error.to_string())
        })?;
    window.show().map_err(|error| {
        AppError::new("SCREENSHOT_OVERLAY_SHOW_FAILED", "显示截图窗口失败")
            .with_detail("reason", error.to_string())
    })?;

    if let Err(error) = window.set_focus() {
        log::warn!(
            target: "bexo::service::screenshot",
            "focus screenshot overlay window failed: {}",
            error
        );
    }

    Ok(())
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
        image_data_url: session.image_data_url.as_ref().clone(),
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

fn normalize_selection(
    session: &ActiveScreenshotSession,
    selection: ScreenshotSelectionInput,
) -> AppResult<PixelSelection> {
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

    let scale_x = if session.display_width == 0 {
        1.0
    } else {
        f64::from(session.capture_width) / f64::from(session.display_width)
    };
    let scale_y = if session.display_height == 0 {
        1.0
    } else {
        f64::from(session.capture_height) / f64::from(session.display_height)
    };
    let mut left = (selection.x * scale_x).floor();
    let mut top = (selection.y * scale_y).floor();
    let mut right = (selection_right * scale_x).ceil();
    let mut bottom = (selection_bottom * scale_y).ceil();

    let capture_width = f64::from(session.capture_width);
    let capture_height = f64::from(session.capture_height);

    left = left.clamp(0.0, capture_width);
    top = top.clamp(0.0, capture_height);
    right = right.clamp(0.0, capture_width);
    bottom = bottom.clamp(0.0, capture_height);

    if right <= left || bottom <= top {
        return Err(AppError::new(
            "SCREENSHOT_SELECTION_EMPTY",
            "截图选区尺寸必须大于 0",
        ));
    }

    let x = left as u32;
    let y = top as u32;
    let width = (right - left) as u32;
    let height = (bottom - top) as u32;

    if width == 0 || height == 0 {
        return Err(AppError::new(
            "SCREENSHOT_SELECTION_EMPTY",
            "截图选区尺寸必须大于 0",
        ));
    }

    Ok(PixelSelection {
        x,
        y,
        width,
        height,
    })
}

fn crop_selection(
    session: &ActiveScreenshotSession,
    selection: PixelSelection,
) -> AppResult<RgbaImage> {
    let source = RgbaImage::from_raw(
        session.capture_width,
        session.capture_height,
        session.rgba_bytes.to_vec(),
    )
    .ok_or_else(|| AppError::new("SCREENSHOT_IMAGE_DECODE_FAILED", "读取截图会话图像失败"))?;

    Ok(imageops::crop_imm(
        &source,
        selection.x,
        selection.y,
        selection.width,
        selection.height,
    )
    .to_image())
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
