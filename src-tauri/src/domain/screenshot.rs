use serde::{Deserialize, Serialize};

pub const SCREENSHOT_OVERLAY_WINDOW_LABEL: &str = "screenshot_overlay";
pub const SCREENSHOT_SESSION_UPDATED_EVENT_NAME: &str = "screenshot://session-updated";

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
    pub image_data_url: String,
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
