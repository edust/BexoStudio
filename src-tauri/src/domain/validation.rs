use std::{collections::HashSet, path::Path};

use crate::error::{AppError, AppResult};

pub const MAX_REORDER_ITEMS: usize = 1_000;

pub fn require_non_empty(field: &str, value: &str, max_len: usize) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation(format!("{field} is required")));
    }
    if trimmed.len() > max_len {
        return Err(AppError::validation(format!(
            "{field} exceeds {max_len} characters"
        )));
    }
    Ok(trimmed.to_string())
}

pub fn parse_color_or_none(value: Option<String>) -> AppResult<Option<String>> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let valid = trimmed.len() == 7
        && trimmed.starts_with('#')
        && trimmed.chars().skip(1).all(|char| char.is_ascii_hexdigit());
    if !valid {
        return Err(AppError::validation("color must be a #RRGGBB hex value"));
    }
    Ok(Some(trimmed.to_uppercase()))
}

pub fn validate_optional_uuid(field: &str, value: Option<String>) -> AppResult<Option<String>> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    uuid::Uuid::parse_str(trimmed)
        .map_err(|_| AppError::validation(format!("{field} must be a valid UUID")))?;
    Ok(Some(trimmed.to_string()))
}

pub fn validate_ordered_uuid_list(
    field: &str,
    values: Vec<String>,
    max_items: usize,
) -> AppResult<Vec<String>> {
    if values.is_empty() {
        return Err(
            AppError::validation(format!("{field} must contain at least one id"))
                .with_detail("field", field),
        );
    }
    if values.len() > max_items {
        return Err(AppError::validation(format!(
            "{field} cannot contain more than {max_items} ids"
        ))
        .with_detail("field", field)
        .with_detail("count", values.len().to_string())
        .with_detail("limit", max_items.to_string()));
    }

    let mut unique = HashSet::with_capacity(values.len());
    let mut normalized = Vec::with_capacity(values.len());
    for value in values {
        let id = validate_optional_uuid(field, Some(value))?
            .ok_or_else(|| AppError::validation(format!("{field} contains an empty id")))?;
        let id = uuid::Uuid::parse_str(&id)
            .map_err(|_| AppError::validation(format!("{field} must contain valid UUIDs")))?
            .to_string();
        if !unique.insert(id.clone()) {
            return Err(
                AppError::validation(format!("{field} contains a duplicate id"))
                    .with_detail("field", field)
                    .with_detail("id", id),
            );
        }
        normalized.push(id);
    }
    Ok(normalized)
}

pub fn ensure_absolute_directory(path: &str, error_code: &str) -> AppResult<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation("path is required"));
    }

    let directory = Path::new(trimmed);
    if !directory.is_absolute() {
        return Err(
            AppError::new(error_code, "path must be an absolute directory path")
                .with_detail("path", trimmed.to_string()),
        );
    }

    let metadata = std::fs::metadata(directory).map_err(|_| {
        AppError::new(error_code, "path does not exist").with_detail("path", trimmed.to_string())
    })?;
    if !metadata.is_dir() {
        return Err(AppError::new(error_code, "path is not a directory")
            .with_detail("path", trimmed.to_string()));
    }

    Ok(trimmed.to_string())
}

pub fn parse_json_string_list(input: &str) -> AppResult<Vec<String>> {
    serde_json::from_str::<Vec<String>>(input)
        .map_err(|_| AppError::new("DB_READ_FAILED", "invalid JSON string list"))
}

pub fn parse_restore_mode(value: &str) -> AppResult<String> {
    let trimmed = value.trim();
    let is_supported = matches!(
        trimmed,
        "full" | "terminals_only" | "ide_only" | "codex_only"
    );
    if !is_supported {
        return Err(AppError::new(
            "RESTORE_MODE_UNSUPPORTED",
            "restore mode must be one of full, terminals_only, ide_only, codex_only",
        )
        .with_detail("mode", trimmed.to_string()));
    }

    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::validate_ordered_uuid_list;

    #[test]
    fn ordered_uuid_list_rejects_duplicates_and_limits() {
        let id = uuid::Uuid::new_v4().to_string();
        let duplicate = validate_ordered_uuid_list("ids", vec![id.clone(), id], 10)
            .expect_err("duplicate ids must fail");
        assert_eq!(duplicate.code, "VALIDATION_ERROR");

        let overflow = validate_ordered_uuid_list(
            "ids",
            vec![
                uuid::Uuid::new_v4().to_string(),
                uuid::Uuid::new_v4().to_string(),
            ],
            1,
        )
        .expect_err("oversized reorder input must fail");
        assert_eq!(overflow.code, "VALIDATION_ERROR");
    }
}
