use serde::{Deserialize, Serialize};

use crate::{
    domain::ProjectRecord,
    error::{AppError, AppResult},
};

use super::validate_optional_uuid;

pub const MAX_WORKSPACE_DESCRIPTION_CHARS: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRecord {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub sort_order: i64,
    pub is_default: bool,
    pub is_archived: bool,
    pub created_at: String,
    pub updated_at: String,
    pub projects: Vec<ProjectRecord>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertWorkspaceInput {
    pub id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub sort_order: Option<i64>,
    pub is_default: Option<bool>,
    pub is_archived: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWorkspaceDescriptionInput {
    pub workspace_id: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReorderWorkspacesInput {
    pub workspace_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteResult {
    pub id: String,
}

pub fn normalize_workspace_description(value: Option<String>) -> AppResult<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };

    if value.chars().any(|character| {
        character == '\0' || (character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    }) {
        return Err(AppError::validation(
            "description cannot contain unsupported control characters",
        )
        .with_detail("field", "description"));
    }

    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let character_count = trimmed.chars().count();
    if character_count > MAX_WORKSPACE_DESCRIPTION_CHARS {
        return Err(AppError::validation(format!(
            "description cannot exceed {MAX_WORKSPACE_DESCRIPTION_CHARS} characters"
        ))
        .with_detail("field", "description")
        .with_detail("count", character_count.to_string())
        .with_detail("limit", MAX_WORKSPACE_DESCRIPTION_CHARS.to_string()));
    }

    Ok(Some(trimmed))
}

pub fn validate_workspace_id(value: String) -> AppResult<String> {
    validate_optional_uuid("workspaceId", Some(value))?
        .ok_or_else(|| AppError::validation("workspaceId is required"))
}

#[cfg(test)]
mod tests {
    use super::{normalize_workspace_description, MAX_WORKSPACE_DESCRIPTION_CHARS};

    #[test]
    fn workspace_description_normalizes_empty_input_and_counts_unicode() {
        assert_eq!(
            normalize_workspace_description(Some("  用途说明  ".into())).unwrap(),
            Some("用途说明".into())
        );
        assert_eq!(
            normalize_workspace_description(Some("   ".into())).unwrap(),
            None
        );
        assert!(normalize_workspace_description(Some(
            "记".repeat(MAX_WORKSPACE_DESCRIPTION_CHARS + 1)
        ))
        .is_err());
    }

    #[test]
    fn workspace_description_allows_layout_whitespace_but_rejects_control_characters() {
        assert!(normalize_workspace_description(Some("第一行\n第二行\t".into())).is_ok());
        assert!(normalize_workspace_description(Some("不可见\u{0007}".into())).is_err());
        assert!(normalize_workspace_description(Some("\u{000B}".into())).is_err());
        assert!(normalize_workspace_description(Some("不可见\0".into())).is_err());
    }
}
