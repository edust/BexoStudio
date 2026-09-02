use chrono::Utc;
use rusqlite::{params, Connection};

use crate::{
    domain::{
        classify_prompt_import_entries, prompt_import_action_allowed,
        validate_prompt_import_selections, PromptImportAction, PromptImportApplyResult,
        PromptImportSelection, PromptTransferPrompt, MAX_PROMPTS,
    },
    error::{AppError, AppResult},
};

use super::list_prompts;

pub fn apply_prompt_import(
    connection: &mut Connection,
    prompts: Vec<PromptTransferPrompt>,
    selections: Vec<PromptImportSelection>,
) -> AppResult<PromptImportApplyResult> {
    let selections = validate_prompt_import_selections(prompts.len(), selections)?;
    let existing_prompts = list_prompts(connection)?;
    let classifications = classify_prompt_import_entries(&prompts, &existing_prompts);

    let mut effective_actions = Vec::with_capacity(selections.len());
    for ((prompt_index, selection), classification) in
        selections.iter().enumerate().zip(classifications.iter())
    {
        let effective_action = if prompt_import_action_allowed(classification, selection.action) {
            selection.action
        } else if classification.status == crate::domain::PromptImportItemStatus::Existing
            && matches!(
                selection.action,
                PromptImportAction::Create | PromptImportAction::Update
            )
        {
            // The first request may have committed while its response was lost. Replaying the
            // same target state is a successful no-op, not a conflict or a second write.
            PromptImportAction::Skip
        } else {
            return Err(AppError::new(
                "PROMPT_TRANSFER_SELECTION_INVALID",
                "导入动作与当前 Prompt 状态不一致，请重新预检",
            )
            .with_detail("promptIndex", prompt_index.to_string())
            .with_detail("status", format!("{:?}", classification.status))
            .with_detail("action", format!("{:?}", selection.action)));
        };
        effective_actions.push(effective_action);
    }

    let create_count = effective_actions
        .iter()
        .filter(|action| **action == PromptImportAction::Create)
        .count();
    let resulting_count = existing_prompts.len().saturating_add(create_count);
    if resulting_count > MAX_PROMPTS {
        return Err(AppError::new(
            "PROMPT_TRANSFER_LIMIT_REACHED",
            format!("导入后不能超过 {MAX_PROMPTS} 条 Prompt"),
        )
        .with_detail("currentCount", existing_prompts.len().to_string())
        .with_detail("createCount", create_count.to_string())
        .with_detail("limit", MAX_PROMPTS.to_string()));
    }

    let mut next_sort_order = connection
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM prompts",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| {
            AppError::new("PROMPT_TRANSFER_DB_FAILED", "读取 Prompt 追加顺序失败")
                .with_detail("reason", error.to_string())
        })?;
    let timestamp = Utc::now().to_rfc3339();
    let mut result = PromptImportApplyResult::default();

    for (prompt_index, (prompt, action)) in prompts.into_iter().zip(effective_actions).enumerate() {
        match action {
            PromptImportAction::Skip => result.skipped_prompt_count += 1,
            PromptImportAction::Create => {
                connection
                    .execute(
                        "INSERT INTO prompts
                         (id, title, content, sort_order, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                        params![
                            prompt.id,
                            prompt.title,
                            prompt.content,
                            next_sort_order,
                            timestamp,
                        ],
                    )
                    .map_err(|error| {
                        AppError::new("PROMPT_TRANSFER_DB_FAILED", "创建导入 Prompt 失败")
                            .with_detail("promptIndex", prompt_index.to_string())
                            .with_detail("reason", error.to_string())
                    })?;
                next_sort_order = next_sort_order.saturating_add(1);
                result.created_prompt_count += 1;
            }
            PromptImportAction::Update => {
                let affected = connection
                    .execute(
                        "UPDATE prompts
                         SET title = ?2, content = ?3, updated_at = ?4
                         WHERE id = ?1",
                        params![prompt.id, prompt.title, prompt.content, timestamp],
                    )
                    .map_err(|error| {
                        AppError::new("PROMPT_TRANSFER_DB_FAILED", "更新导入 Prompt 失败")
                            .with_detail("promptIndex", prompt_index.to_string())
                            .with_detail("reason", error.to_string())
                    })?;
                if affected != 1 {
                    return Err(AppError::new(
                        "PROMPT_TRANSFER_SELECTION_INVALID",
                        "预检时匹配的 Prompt 已不存在，请重新预检",
                    )
                    .with_detail("promptIndex", prompt_index.to_string()));
                }
                result.updated_prompt_count += 1;
            }
        }
    }

    result.prompts = list_prompts(connection)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::UpsertPromptInput,
        persistence::{prompt_repo::upsert_prompt, schema},
    };

    fn initialized_connection() -> Connection {
        let connection = Connection::open_in_memory().expect("open sqlite memory database");
        connection
            .execute_batch(schema::SCHEMA)
            .expect("initialize schema");
        connection
    }

    fn imported_prompt(
        id: &str,
        title: &str,
        content: &str,
        sort_order: i64,
    ) -> PromptTransferPrompt {
        PromptTransferPrompt {
            id: id.to_string(),
            title: title.to_string(),
            content: content.to_string(),
            sort_order,
        }
    }

    #[test]
    fn repeated_create_request_is_an_idempotent_successful_skip() {
        let mut connection = initialized_connection();
        let id = uuid::Uuid::new_v4().to_string();
        let prompt = imported_prompt(&id, "导入", "正文", 0);
        let first = apply_prompt_import(
            &mut connection,
            vec![prompt.clone()],
            vec![PromptImportSelection {
                prompt_index: 0,
                action: PromptImportAction::Create,
            }],
        )
        .expect("first import");
        assert_eq!(first.created_prompt_count, 1);

        let second = apply_prompt_import(
            &mut connection,
            vec![prompt],
            vec![PromptImportSelection {
                prompt_index: 0,
                action: PromptImportAction::Create,
            }],
        )
        .expect("repeat the exact create request");
        assert_eq!(second.skipped_prompt_count, 1);
        assert_eq!(second.prompts.len(), 1);
        assert_eq!(second.prompts[0].id, id);
    }

    #[test]
    fn explicit_update_preserves_stable_id_and_local_order() {
        let mut connection = initialized_connection();
        let first_id = uuid::Uuid::new_v4().to_string();
        let second_id = uuid::Uuid::new_v4().to_string();
        upsert_prompt(
            &mut connection,
            UpsertPromptInput {
                id: Some(first_id.clone()),
                title: "第一条".to_string(),
                content: "旧正文".to_string(),
            },
        )
        .expect("create first");
        upsert_prompt(
            &mut connection,
            UpsertPromptInput {
                id: Some(second_id.clone()),
                title: "第二条".to_string(),
                content: "正文二".to_string(),
            },
        )
        .expect("create second");

        let result = apply_prompt_import(
            &mut connection,
            vec![imported_prompt(&first_id, "第一条新标题", "新正文", 0)],
            vec![PromptImportSelection {
                prompt_index: 0,
                action: PromptImportAction::Update,
            }],
        )
        .expect("update existing prompt");
        assert_eq!(result.updated_prompt_count, 1);
        assert_eq!(result.prompts[0].id, first_id);
        assert_eq!(result.prompts[0].sort_order, 0);
        assert_eq!(result.prompts[0].content, "新正文");
        assert_eq!(result.prompts[1].id, second_id);
        assert_eq!(result.prompts[1].sort_order, 1);

        let replay = apply_prompt_import(
            &mut connection,
            vec![imported_prompt(&first_id, "第一条新标题", "新正文", 0)],
            vec![PromptImportSelection {
                prompt_index: 0,
                action: PromptImportAction::Update,
            }],
        )
        .expect("repeat the exact update request");
        assert_eq!(replay.updated_prompt_count, 0);
        assert_eq!(replay.skipped_prompt_count, 1);
        assert_eq!(replay.prompts[0].id, first_id);
    }

    #[test]
    fn invalid_action_is_rejected_before_any_write() {
        let mut connection = initialized_connection();
        let ready_id = uuid::Uuid::new_v4().to_string();
        let second_id = uuid::Uuid::new_v4().to_string();
        let error = apply_prompt_import(
            &mut connection,
            vec![
                imported_prompt(&ready_id, "可新建", "正文", 0),
                imported_prompt(&second_id, "第二条", "正文二", 1),
            ],
            vec![
                PromptImportSelection {
                    prompt_index: 0,
                    action: PromptImportAction::Create,
                },
                PromptImportSelection {
                    prompt_index: 1,
                    action: PromptImportAction::Update,
                },
            ],
        )
        .expect_err("invalid update must fail");
        assert_eq!(error.code, "PROMPT_TRANSFER_SELECTION_INVALID");
        assert!(list_prompts(&connection).expect("list prompts").is_empty());
    }
}
