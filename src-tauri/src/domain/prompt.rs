use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

use super::validate_optional_uuid;

pub const MAX_PROMPTS: usize = 1_000;
pub const MAX_PROMPT_TITLE_CHARS: usize = 120;
pub const MAX_PROMPT_CONTENT_CHARS: usize = 100_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PromptRecord {
    pub id: String,
    pub title: String,
    pub content: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertPromptInput {
    pub id: Option<String>,
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReorderPromptsInput {
    pub prompt_ids: Vec<String>,
}

pub fn validate_prompt_id(value: Option<String>) -> AppResult<Option<String>> {
    let Some(id) = validate_optional_uuid("id", value)? else {
        return Ok(None);
    };
    let normalized = uuid::Uuid::parse_str(&id)
        .map_err(|_| AppError::validation("id must be a valid UUID"))?
        .to_string();
    Ok(Some(normalized))
}

pub fn validate_prompt_title(value: &str) -> AppResult<String> {
    let title = value.trim();
    if title.is_empty() {
        return Err(AppError::validation("title is required").with_detail("field", "title"));
    }

    let character_count = title.chars().count();
    if character_count > MAX_PROMPT_TITLE_CHARS {
        return Err(AppError::validation(format!(
            "title cannot exceed {MAX_PROMPT_TITLE_CHARS} characters"
        ))
        .with_detail("field", "title")
        .with_detail("count", character_count.to_string())
        .with_detail("limit", MAX_PROMPT_TITLE_CHARS.to_string()));
    }
    if title.chars().any(char::is_control) {
        return Err(
            AppError::validation("title cannot contain control characters")
                .with_detail("field", "title"),
        );
    }

    Ok(title.to_string())
}

pub fn validate_prompt_content(value: String) -> AppResult<String> {
    if value.trim().is_empty() {
        return Err(AppError::validation("content is required").with_detail("field", "content"));
    }

    let character_count = value.chars().count();
    if character_count > MAX_PROMPT_CONTENT_CHARS {
        return Err(AppError::validation(format!(
            "content cannot exceed {MAX_PROMPT_CONTENT_CHARS} characters"
        ))
        .with_detail("field", "content")
        .with_detail("count", character_count.to_string())
        .with_detail("limit", MAX_PROMPT_CONTENT_CHARS.to_string()));
    }
    if value.contains('\0') {
        return Err(
            AppError::validation("content cannot contain NUL characters")
                .with_detail("field", "content"),
        );
    }

    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_validation_counts_unicode_characters_and_preserves_content() {
        let title = "常".repeat(MAX_PROMPT_TITLE_CHARS);
        assert_eq!(validate_prompt_title(&title).expect("valid title"), title);
        assert!(validate_prompt_title(&"常".repeat(MAX_PROMPT_TITLE_CHARS + 1)).is_err());
        assert!(validate_prompt_title("无效\n标题").is_err());

        let content = "\n  保留首尾空白  \n".to_string();
        assert_eq!(
            validate_prompt_content(content.clone()).expect("valid content"),
            content
        );
        assert!(validate_prompt_content("正文\0内容".to_string()).is_err());
    }
}
