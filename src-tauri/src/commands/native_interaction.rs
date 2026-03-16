use std::time::Instant;

use serde::Deserialize;
use tauri::State;

use crate::{
    error::{AppError, CommandResponse},
    services::{
        NativeInteractionExclusionRect, NativeInteractionMode, NativeInteractionRuntimeUpdateInput,
        NativeInteractionSelectionRect, NativeInteractionService, NativeInteractionStateView,
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
    pub annotation_color: Option<String>,
    pub annotation_stroke_width: Option<f64>,
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
    match native_interaction_service.update_runtime(NativeInteractionRuntimeUpdateInput {
        session_id: input.session_id,
        visible: input.visible,
        exclusion_rects: input.exclusion_rects,
        mode: input.mode,
        selection: input.selection,
        annotation_color: input.annotation_color,
        annotation_stroke_width: input.annotation_stroke_width,
    }) {
        Ok(data) => {
            log::debug!(
                target: "bexo::command::native_interaction",
                "update_native_interaction_runtime completed lifecycle_state={} has_active_session={} selection_revision={} total_ms={}",
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
                "update_native_interaction_runtime failed total_ms={} reason={}",
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
