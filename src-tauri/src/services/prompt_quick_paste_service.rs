use std::sync::Arc;

use chrono::Utc;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_notification::NotificationExt;

use crate::{
    adapters::{PromptPasteAdapter, SystemPromptPasteAdapter},
    domain::{
        HotkeyAction, PromptQuickPasteResultEvent, PromptQuickPasteResultStatus,
        PROMPT_QUICK_PASTE_RESULT_EVENT_NAME,
    },
    error::{AppError, AppResult},
};

use super::{PreferencesService, PromptService};

#[derive(Clone)]
pub struct PromptQuickPasteService {
    adapter: Arc<dyn PromptPasteAdapter>,
}

#[derive(Debug)]
struct ResolvedPromptPaste {
    slot: u8,
    prompt_id: String,
    prompt_title: String,
}

impl PromptQuickPasteService {
    pub fn new() -> Self {
        Self {
            adapter: Arc::new(SystemPromptPasteAdapter::new()),
        }
    }

    pub async fn execute<R: Runtime>(
        &self,
        app: AppHandle<R>,
        action: HotkeyAction,
        shortcut: String,
    ) {
        let slot = action.prompt_quick_paste_slot().unwrap_or_default();
        let result = self
            .resolve_and_paste(&app, action, shortcut.as_str())
            .await;

        let event = match result {
            Ok(resolved) => {
                log::info!(
                    target: "bexo::service::prompt_quick_paste",
                    "prompt quick paste sent slot={} prompt_id={} shortcut={}",
                    resolved.slot,
                    resolved.prompt_id,
                    shortcut
                );
                PromptQuickPasteResultEvent {
                    slot: resolved.slot,
                    shortcut,
                    status: PromptQuickPasteResultStatus::Sent,
                    prompt_id: Some(resolved.prompt_id),
                    prompt_title: Some(resolved.prompt_title.clone()),
                    message: format!("已发送粘贴：{}", resolved.prompt_title),
                    error: None,
                    occurred_at: Utc::now().to_rfc3339(),
                }
            }
            Err(error) => {
                log::error!(
                    target: "bexo::service::prompt_quick_paste",
                    "prompt quick paste failed slot={} action={} shortcut={} code={} reason={}",
                    slot,
                    action.key(),
                    shortcut,
                    error.code,
                    error
                );
                PromptQuickPasteResultEvent {
                    slot,
                    shortcut,
                    status: PromptQuickPasteResultStatus::Failed,
                    prompt_id: error
                        .details
                        .as_ref()
                        .and_then(|details| details.get("promptId"))
                        .cloned(),
                    prompt_title: None,
                    message: error.message.clone(),
                    error: Some(error),
                    occurred_at: Utc::now().to_rfc3339(),
                }
            }
        };

        send_failure_notification(&app, &event);
        if let Err(error) = app.emit(PROMPT_QUICK_PASTE_RESULT_EVENT_NAME, event) {
            log::error!(
                target: "bexo::service::prompt_quick_paste",
                "emit prompt quick paste result failed slot={} reason={}",
                slot,
                error
            );
        }
    }

    async fn resolve_and_paste<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        action: HotkeyAction,
        shortcut: &str,
    ) -> AppResult<ResolvedPromptPaste> {
        let slot_number = action.prompt_quick_paste_slot().ok_or_else(|| {
            AppError::new(
                "PROMPT_QUICK_PASTE_ACTION_INVALID",
                "无法识别 Prompt 快速粘贴动作",
            )
        })?;
        let preferences = app.state::<PreferencesService>().get_preferences()?;
        let slot = preferences
            .hotkey
            .prompt_quick_paste_slots
            .iter()
            .find(|slot| slot.slot == slot_number)
            .ok_or_else(|| {
                AppError::new(
                    "PROMPT_QUICK_PASTE_SLOT_MISSING",
                    "Prompt 快速粘贴槽位不存在，请在热键设置中重新保存",
                )
                .with_detail("slot", slot_number.to_string())
            })?;
        if !slot.enabled {
            return Err(AppError::new(
                "PROMPT_QUICK_PASTE_SLOT_DISABLED",
                "该 Prompt 快速粘贴槽位已停用",
            )
            .with_detail("slot", slot_number.to_string()));
        }
        if !slot.shortcut.eq_ignore_ascii_case(shortcut) {
            return Err(AppError::new(
                "PROMPT_QUICK_PASTE_BINDING_STALE",
                "快速粘贴配置正在更新，请重新按一次热键",
            )
            .with_detail("slot", slot_number.to_string())
            .with_detail("shortcut", shortcut.to_string()));
        }
        let prompt_id = slot.prompt_id.clone().ok_or_else(|| {
            AppError::new(
                "PROMPT_QUICK_PASTE_PROMPT_REQUIRED",
                "该槽位尚未选择 Prompt",
            )
            .with_detail("slot", slot_number.to_string())
        })?;
        let prompt = app
            .state::<PromptService>()
            .get_prompt(prompt_id)
            .await
            .map_err(|error| error.with_detail("slot", slot_number.to_string()))?;
        if prompt.content.trim().is_empty() {
            return Err(AppError::new(
                "PROMPT_QUICK_PASTE_CONTENT_EMPTY",
                "绑定的 Prompt 正文为空，未发送粘贴",
            )
            .with_detail("slot", slot_number.to_string())
            .with_detail("promptId", prompt.id));
        }

        let content = prompt.content.clone();
        let adapter = Arc::clone(&self.adapter);
        tauri::async_runtime::spawn_blocking(move || adapter.paste_text(content))
            .await
            .map_err(|error| {
                AppError::new("PROMPT_QUICK_PASTE_TASK_FAILED", "快速粘贴后台任务异常终止")
                    .with_detail("reason", error.to_string())
            })??;

        Ok(ResolvedPromptPaste {
            slot: slot_number,
            prompt_id: prompt.id,
            prompt_title: prompt.title,
        })
    }
}

fn send_failure_notification<R: Runtime>(app: &AppHandle<R>, event: &PromptQuickPasteResultEvent) {
    if event.status != PromptQuickPasteResultStatus::Failed {
        return;
    }

    if let Err(error) = app
        .notification()
        .builder()
        .title("Prompt 快速粘贴失败")
        .body(event.message.clone())
        .show()
    {
        log::warn!(
            target: "bexo::service::prompt_quick_paste",
            "show prompt quick paste notification failed slot={} status={:?} reason={}",
            event.slot,
            event.status,
            error
        );
    }
}

impl Default for PromptQuickPasteService {
    fn default() -> Self {
        Self::new()
    }
}
