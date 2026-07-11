use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::{
    domain::{
        validate_ordered_uuid_list, validate_prompt_content, validate_prompt_id,
        validate_prompt_title, DeleteResult, PromptRecord, ReorderPromptsInput, UpsertPromptInput,
        MAX_PROMPTS,
    },
    error::{AppError, AppResult},
};

const PROMPT_SELECT: &str =
    "SELECT id, title, content, sort_order, created_at, updated_at FROM prompts";

fn map_prompt_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PromptRecord> {
    Ok(PromptRecord {
        id: row.get("id")?,
        title: row.get("title")?,
        content: row.get("content")?,
        sort_order: row.get("sort_order")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn list_prompts(connection: &Connection) -> AppResult<Vec<PromptRecord>> {
    let mut statement = connection
        .prepare(&format!(
            "{PROMPT_SELECT} ORDER BY sort_order ASC, created_at ASC, id ASC"
        ))
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to prepare prompt query")
                .with_detail("reason", error.to_string())
        })?;
    let rows = statement
        .query_map([], map_prompt_from_row)
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to query prompts")
                .with_detail("reason", error.to_string())
        })?;

    let mut prompts = Vec::new();
    for row in rows {
        prompts.push(row.map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to map prompt row")
                .with_detail("reason", error.to_string())
        })?);
    }
    Ok(prompts)
}

pub fn upsert_prompt(
    connection: &mut Connection,
    input: UpsertPromptInput,
) -> AppResult<PromptRecord> {
    let id = validate_prompt_id(input.id)?.unwrap_or_else(|| Uuid::new_v4().to_string());
    let title = validate_prompt_title(&input.title)?;
    let content = validate_prompt_content(input.content)?;
    let exists = connection
        .query_row(
            "SELECT 1 FROM prompts WHERE id = ?1 LIMIT 1",
            [id.as_str()],
            |_| Ok(true),
        )
        .optional()
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to check prompt existence")
                .with_detail("promptId", id.clone())
                .with_detail("reason", error.to_string())
        })?
        .unwrap_or(false);
    let timestamp = Utc::now().to_rfc3339();

    if exists {
        connection
            .execute(
                "UPDATE prompts SET title = ?1, content = ?2, updated_at = ?3 WHERE id = ?4",
                params![title, content, timestamp, id],
            )
            .map_err(|error| {
                AppError::new("DB_WRITE_FAILED", "failed to update prompt")
                    .with_detail("promptId", id.clone())
                    .with_detail("reason", error.to_string())
            })?;
    } else {
        let prompt_count = connection
            .query_row("SELECT COUNT(*) FROM prompts", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|error| {
                AppError::new("DB_READ_FAILED", "failed to count prompts")
                    .with_detail("reason", error.to_string())
            })?;
        if prompt_count >= MAX_PROMPTS as i64 {
            return Err(AppError::new(
                "PROMPT_LIMIT_REACHED",
                format!("cannot save more than {MAX_PROMPTS} prompts"),
            )
            .with_detail("limit", MAX_PROMPTS.to_string()));
        }

        let next_sort_order = connection
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM prompts",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| {
                AppError::new("DB_READ_FAILED", "failed to resolve prompt sort order")
                    .with_detail("reason", error.to_string())
            })?;
        connection
            .execute(
                "INSERT INTO prompts (id, title, content, sort_order, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                params![id, title, content, next_sort_order, timestamp],
            )
            .map_err(|error| {
                AppError::new("DB_WRITE_FAILED", "failed to create prompt")
                    .with_detail("promptId", id.clone())
                    .with_detail("reason", error.to_string())
            })?;
    }

    get_prompt_by_id(connection, &id)
}

pub fn delete_prompt(connection: &mut Connection, id: String) -> AppResult<DeleteResult> {
    let id = validate_prompt_id(Some(id))?
        .ok_or_else(|| AppError::validation("id is required").with_detail("field", "id"))?;
    let sort_order = connection
        .query_row(
            "SELECT sort_order FROM prompts WHERE id = ?1",
            [id.as_str()],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to load prompt before deletion")
                .with_detail("promptId", id.clone())
                .with_detail("reason", error.to_string())
        })?;

    let Some(sort_order) = sort_order else {
        return Ok(DeleteResult { id });
    };

    connection
        .execute("DELETE FROM prompts WHERE id = ?1", [id.as_str()])
        .map_err(|error| {
            AppError::new("DB_WRITE_FAILED", "failed to delete prompt")
                .with_detail("promptId", id.clone())
                .with_detail("reason", error.to_string())
        })?;
    connection
        .execute(
            "UPDATE prompts SET sort_order = sort_order - 1 WHERE sort_order > ?1",
            [sort_order],
        )
        .map_err(|error| {
            AppError::new(
                "DB_WRITE_FAILED",
                "failed to compact prompt order after deletion",
            )
            .with_detail("promptId", id.clone())
            .with_detail("reason", error.to_string())
        })?;

    Ok(DeleteResult { id })
}

pub fn reorder_prompts(
    connection: &mut Connection,
    input: ReorderPromptsInput,
) -> AppResult<Vec<PromptRecord>> {
    let prompt_ids = validate_ordered_uuid_list("promptIds", input.prompt_ids, MAX_PROMPTS)?;
    let transaction = connection.savepoint().map_err(|error| {
        AppError::new(
            "DB_WRITE_FAILED",
            "failed to open prompt reorder transaction",
        )
        .with_detail("reason", error.to_string())
    })?;
    let prompt_count = transaction
        .query_row("SELECT COUNT(*) FROM prompts", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to count prompts before reorder")
                .with_detail("reason", error.to_string())
        })?;
    if prompt_count != prompt_ids.len() as i64 {
        return Err(AppError::new(
            "PROMPT_REORDER_INCOMPLETE",
            "promptIds must contain every saved prompt exactly once",
        )
        .with_detail("expectedCount", prompt_count.to_string())
        .with_detail("receivedCount", prompt_ids.len().to_string()));
    }

    for (index, prompt_id) in prompt_ids.iter().enumerate() {
        let affected = transaction
            .execute(
                "UPDATE prompts SET sort_order = ?1 WHERE id = ?2",
                params![index as i64, prompt_id],
            )
            .map_err(|error| {
                AppError::new("DB_WRITE_FAILED", "failed to reorder prompts")
                    .with_detail("promptId", prompt_id.clone())
                    .with_detail("reason", error.to_string())
            })?;
        if affected != 1 {
            return Err(AppError::new("PROMPT_NOT_FOUND", "prompt was not found")
                .with_detail("promptId", prompt_id.clone()));
        }
    }

    transaction.commit().map_err(|error| {
        AppError::new("DB_WRITE_FAILED", "failed to commit prompt reorder")
            .with_detail("reason", error.to_string())
    })?;
    list_prompts(connection)
}

pub fn get_prompt_by_id(connection: &Connection, id: &str) -> AppResult<PromptRecord> {
    connection
        .query_row(
            &format!("{PROMPT_SELECT} WHERE id = ?1"),
            [id],
            map_prompt_from_row,
        )
        .optional()
        .map_err(|error| {
            AppError::new("DB_READ_FAILED", "failed to load prompt")
                .with_detail("promptId", id.to_string())
                .with_detail("reason", error.to_string())
        })?
        .ok_or_else(|| {
            AppError::new("PROMPT_NOT_FOUND", "绑定的 Prompt 已不存在")
                .with_detail("promptId", id.to_string())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn initialized_connection() -> Connection {
        let connection = Connection::open_in_memory().expect("open sqlite memory database");
        connection
            .execute_batch(crate::persistence::schema::SCHEMA)
            .expect("initialize schema");
        connection
    }

    fn create_prompt(connection: &mut Connection, id: &str, title: &str, content: &str) {
        upsert_prompt(
            connection,
            UpsertPromptInput {
                id: Some(id.to_string()),
                title: title.to_string(),
                content: content.to_string(),
            },
        )
        .expect("create prompt");
    }

    #[test]
    fn prompt_crud_preserves_content_and_compacts_order() {
        let mut connection = initialized_connection();
        let first_id = Uuid::new_v4().to_string();
        let second_id = Uuid::new_v4().to_string();
        let content = "\n  保留正文空白  \n";
        create_prompt(&mut connection, &first_id, "第一条", content);
        create_prompt(&mut connection, &second_id, "第二条", "正文二");

        let prompts = list_prompts(&connection).expect("list prompts");
        assert_eq!(prompts[0].content, content);
        assert_eq!(prompts[1].sort_order, 1);

        delete_prompt(&mut connection, first_id.clone()).expect("delete prompt");
        delete_prompt(&mut connection, first_id).expect("repeat delete is idempotent");
        let remaining = list_prompts(&connection).expect("list remaining prompts");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, second_id);
        assert_eq!(remaining[0].sort_order, 0);
    }

    #[test]
    fn prompt_reorder_rolls_back_missing_and_incomplete_batches() {
        let mut connection = initialized_connection();
        let first_id = Uuid::new_v4().to_string();
        let second_id = Uuid::new_v4().to_string();
        create_prompt(&mut connection, &first_id, "第一条", "正文一");
        create_prompt(&mut connection, &second_id, "第二条", "正文二");

        let incomplete = reorder_prompts(
            &mut connection,
            ReorderPromptsInput {
                prompt_ids: vec![first_id.clone()],
            },
        )
        .expect_err("incomplete order must fail");
        assert_eq!(incomplete.code, "PROMPT_REORDER_INCOMPLETE");

        let missing = reorder_prompts(
            &mut connection,
            ReorderPromptsInput {
                prompt_ids: vec![second_id.clone(), Uuid::new_v4().to_string()],
            },
        )
        .expect_err("missing prompt must fail");
        assert_eq!(missing.code, "PROMPT_NOT_FOUND");
        let unchanged = list_prompts(&connection).expect("list unchanged prompts");
        assert_eq!(unchanged[0].id, first_id);
        assert_eq!(unchanged[0].sort_order, 0);
        assert_eq!(unchanged[1].id, second_id);
        assert_eq!(unchanged[1].sort_order, 1);

        let reordered = reorder_prompts(
            &mut connection,
            ReorderPromptsInput {
                prompt_ids: vec![second_id.clone(), first_id.clone()],
            },
        )
        .expect("reorder prompts");
        assert_eq!(reordered[0].id, second_id);
        assert_eq!(reordered[1].id, first_id);
    }

    #[test]
    fn get_prompt_by_id_reports_deleted_binding_as_not_found() {
        let connection = initialized_connection();
        let missing_id = Uuid::new_v4().to_string();
        let error = get_prompt_by_id(&connection, missing_id.as_str())
            .expect_err("missing prompt must have a domain error");
        assert_eq!(error.code, "PROMPT_NOT_FOUND");
        assert_eq!(
            error
                .details
                .as_ref()
                .and_then(|details| details.get("promptId")),
            Some(&missing_id)
        );
    }
}
