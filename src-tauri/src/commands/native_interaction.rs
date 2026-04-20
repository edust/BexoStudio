use std::time::Instant;

use serde::Deserialize;
use tauri::State;

use crate::{
    error::{AppError, CommandResponse},
    services::{
        NativeInteractionEditableShape, NativeInteractionExclusionRect, NativeInteractionMode,
        NativeInteractionRuntimeUpdateInput, NativeInteractionSelectionRect,
        NativeInteractionService, NativeInteractionStateView,
    },
};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateNativeInteractionRuntimePayload {
    pub session_id: String,
    pub visible: bool,
    #[serde(default)]
    pub exclusion_rects: Vec<NativeInteractionExclusionRect>,
    #[serde(default = "default_native_interaction_mode")]
    pub mode: NativeInteractionMode,
    pub selection: Option<NativeInteractionSelectionRect>,
    pub active_shape: Option<NativeInteractionEditableShape>,
    #[serde(default)]
    pub shape_candidates: Vec<NativeInteractionEditableShape>,
    pub annotation_color: Option<String>,
    pub annotation_stroke_width: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateNativeInteractionExclusionRectsPayload {
    pub session_id: String,
    #[serde(default)]
    pub exclusion_rects: Vec<NativeInteractionExclusionRect>,
}

#[tauri::command]
pub async fn get_native_interaction_state(
    native_interaction_service: State<'_, NativeInteractionService>,
) -> Result<CommandResponse<NativeInteractionStateView>, AppError> {
    let started_at = Instant::now();
    match native_interaction_service.snapshot_state() {
        Ok(data) => {
            log::debug!(
                target: "bexo::command::native_interaction",
                "get_native_interaction_state completed lifecycle_state={} has_active_session={} selection_revision={} total_ms={}",
                data.lifecycle_state,
                data.has_active_session,
                data.selection_revision,
                started_at.elapsed().as_millis()
            );
            Ok(CommandResponse::success(data))
        }
        Err(error) => {
            log::error!(
                target: "bexo::command::native_interaction",
                "get_native_interaction_state failed total_ms={} reason={}",
                started_at.elapsed().as_millis(),
                error
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn update_native_interaction_runtime(
    native_interaction_service: State<'_, NativeInteractionService>,
    input: UpdateNativeInteractionRuntimePayload,
) -> Result<CommandResponse<NativeInteractionStateView>, AppError> {
    let started_at = Instant::now();
    let request_session_id = input.session_id.clone();
    let request_visible = input.visible;
    let request_mode = input.mode;
    let request_shape_candidates = input.shape_candidates.len();
    match native_interaction_service.update_runtime(NativeInteractionRuntimeUpdateInput {
        session_id: input.session_id,
        visible: input.visible,
        exclusion_rects: input.exclusion_rects,
        mode: input.mode,
        selection: input.selection,
        active_shape: input.active_shape,
        shape_candidates: input.shape_candidates,
        annotation_color: input.annotation_color,
        annotation_stroke_width: input.annotation_stroke_width,
    }) {
        Ok(data) => {
            log::debug!(
                target: "bexo::command::native_interaction",
                "update_native_interaction_runtime completed session_id={} lifecycle_state={} has_active_session={} selection_revision={} total_ms={}",
                request_session_id,
                data.lifecycle_state,
                data.has_active_session,
                data.selection_revision,
                started_at.elapsed().as_millis()
            );
            Ok(CommandResponse::success(data))
        }
        Err(error) => {
            log::error!(
                target: "bexo::command::native_interaction",
                "update_native_interaction_runtime failed session_id={} visible={} mode={} candidates={} total_ms={} reason={}",
                request_session_id,
                request_visible,
                request_mode.as_str(),
                request_shape_candidates,
                started_at.elapsed().as_millis(),
                error
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn update_native_interaction_exclusion_rects(
    native_interaction_service: State<'_, NativeInteractionService>,
    input: UpdateNativeInteractionExclusionRectsPayload,
) -> Result<CommandResponse<bool>, AppError> {
    let started_at = Instant::now();
    let request_session_id = input.session_id.clone();
    let request_rects = input.exclusion_rects.len();

    match native_interaction_service.update_exclusion_rects(input.session_id, input.exclusion_rects)
    {
        Ok(()) => {
            log::debug!(
                target: "bexo::command::native_interaction",
                "update_native_interaction_exclusion_rects completed session_id={} rects={} total_ms={}",
                request_session_id,
                request_rects,
                started_at.elapsed().as_millis()
            );
            Ok(CommandResponse::success(true))
        }
        Err(error) => {
            log::error!(
                target: "bexo::command::native_interaction",
                "update_native_interaction_exclusion_rects failed session_id={} rects={} total_ms={} reason={}",
                request_session_id,
                request_rects,
                started_at.elapsed().as_millis(),
                error
            );
            Ok(CommandResponse::failure(error))
        }
    }
}

fn default_native_interaction_mode() -> NativeInteractionMode {
    NativeInteractionMode::Selection
}
