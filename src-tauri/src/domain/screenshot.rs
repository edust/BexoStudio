use serde::{Deserialize, Serialize};

pub const SCREENSHOT_OVERLAY_WINDOW_LABEL: &str = "screenshot_overlay";
pub const SCREENSHOT_SESSION_UPDATED_EVENT_NAME: &str = "screenshot://session-updated";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenshotImageStatus {
    Loading,
    Ready,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenshotPreviewTransport {
    File,
    RawRgbaFast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenshotSelectionRenderMode {
    Native,
    LogicalFallback,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotMonitorView {
    pub display_id: u32,
    pub display_x: i32,
    pub display_y: i32,
    pub relative_x: u32,
    pub relative_y: u32,
    pub display_width: u32,
    pub display_height: u32,
    pub capture_width: u32,
    pub capture_height: u32,
    pub scale_factor: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotSessionView {
    pub session_id: String,
    pub created_at: String,
    pub display_x: i32,
    pub display_y: i32,
    pub display_width: u32,
    pub display_height: u32,
    pub scale_factor: f32,
    pub capture_width: u32,
    pub capture_height: u32,
    pub image_status: ScreenshotImageStatus,
    pub image_error: Option<String>,
    pub image_data_url: String,
    pub preview_image_path: Option<String>,
    pub preview_transport: ScreenshotPreviewTransport,
    pub preview_pixel_width: u32,
    pub preview_pixel_height: u32,
    pub monitors: Vec<ScreenshotMonitorView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotSelectionInput {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotRenderedImageInput {
    pub data_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartScreenshotSessionResult {
    pub session_id: String,
    pub window_label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotSelectionRenderTile {
    pub display_id: u32,
    pub scale_factor: f32,
    pub logical_x: f64,
    pub logical_y: f64,
    pub logical_width: f64,
    pub logical_height: f64,
    pub output_x: u32,
    pub output_y: u32,
    pub output_width: u32,
    pub output_height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotSelectionRenderView {
    pub session_id: String,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    pub render_mode: ScreenshotSelectionRenderMode,
    pub image_data_url: String,
    pub tiles: Vec<ScreenshotSelectionRenderTile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyScreenshotSelectionResult {
    pub session_id: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveScreenshotSelectionResult {
    pub session_id: String,
    pub file_path: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelScreenshotSessionResult {
    pub session_id: String,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotSessionUpdatedEvent {
    pub session_id: String,
    pub created_at: String,
}
