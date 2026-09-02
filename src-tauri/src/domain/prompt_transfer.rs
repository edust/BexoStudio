use std::collections::{HashMap, HashSet};

use chrono::DateTime;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

use super::{
    validate_prompt_content, validate_prompt_id, validate_prompt_title, PromptRecord, MAX_PROMPTS,
};

pub const PROMPT_TRANSFER_FORMAT: &str = "bexo-studio.prompts";
pub const PROMPT_TRANSFER_SCHEMA_VERSION: u32 = 1;
pub const PROMPT_TRANSFER_MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PromptTransferDocument {
    pub format: String,
    pub schema_version: u32,
    pub exported_at: String,
    pub prompts: Vec<PromptTransferPrompt>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PromptTransferPrompt {
    pub id: String,
    pub title: String,
    pub content: String,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportPromptListInput {
    pub destination_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPromptListResult {
    pub prompt_count: usize,
    pub file_size_bytes: u64,
    pub file_hash: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewPromptListImportInput {
    pub source_path: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PromptImportAction {
    Create,
    Skip,
    Update,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PromptImportSelection {
    pub prompt_index: usize,
    pub action: PromptImportAction,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyPromptListImportInput {
    pub source_path: String,
    pub expected_file_hash: String,
    pub selections: Vec<PromptImportSelection>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PromptImportItemStatus {
    Ready,
    Existing,
    Conflict,
    Duplicate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptImportClassification {
    pub status: PromptImportItemStatus,
    pub recommended_action: PromptImportAction,
    pub can_create: bool,
    pub can_update: bool,
    pub matched_prompt_id: Option<String>,
    pub matched_prompt_title: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptImportItemPreview {
    pub prompt_index: usize,
    pub id: String,
    pub title: String,
    pub content_preview: String,
    pub status: PromptImportItemStatus,
    pub recommended_action: PromptImportAction,
    pub can_create: bool,
    pub can_update: bool,
    pub matched_prompt_id: Option<String>,
    pub matched_prompt_title: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PromptImportPreviewSummary {
    pub total_prompt_count: usize,
    pub ready_prompt_count: usize,
    pub existing_prompt_count: usize,
    pub conflict_prompt_count: usize,
    pub duplicate_prompt_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptImportPreview {
    pub source_path: String,
    pub file_hash: String,
    pub schema_version: u32,
    pub summary: PromptImportPreviewSummary,
    pub items: Vec<PromptImportItemPreview>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PromptImportApplyResult {
    pub created_prompt_count: usize,
    pub updated_prompt_count: usize,
    pub skipped_prompt_count: usize,
    pub prompts: Vec<PromptRecord>,
}

pub fn validate_prompt_transfer_document(
    mut document: PromptTransferDocument,
) -> AppResult<PromptTransferDocument> {
    if document.format != PROMPT_TRANSFER_FORMAT {
        return Err(AppError::new(
            "PROMPT_TRANSFER_SCHEMA_UNSUPPORTED",
            "这不是 Bexo Studio Prompts 文件",
        )
        .with_detail("format", document.format));
    }
    if document.schema_version != PROMPT_TRANSFER_SCHEMA_VERSION {
        return Err(AppError::new(
            "PROMPT_TRANSFER_SCHEMA_UNSUPPORTED",
            "Prompts 文件版本不受当前 Bexo Studio 支持",
        )
        .with_detail("schemaVersion", document.schema_version.to_string())
        .with_detail(
            "supportedSchemaVersion",
            PROMPT_TRANSFER_SCHEMA_VERSION.to_string(),
        ));
    }

    DateTime::parse_from_rfc3339(document.exported_at.trim()).map_err(|error| {
        AppError::new(
            "PROMPT_TRANSFER_INVALID",
            "exportedAt 必须是合法的 RFC 3339 时间",
        )
        .with_detail("reason", error.to_string())
    })?;
    document.exported_at = document.exported_at.trim().to_string();

    if document.prompts.len() > MAX_PROMPTS {
        return Err(AppError::new(
            "PROMPT_TRANSFER_LIMIT_REACHED",
            format!("Prompts 文件不能超过 {MAX_PROMPTS} 条记录"),
        )
        .with_detail("count", document.prompts.len().to_string())
        .with_detail("limit", MAX_PROMPTS.to_string()));
    }

    let mut ids = HashSet::with_capacity(document.prompts.len());
    for (prompt_index, prompt) in document.prompts.iter_mut().enumerate() {
        let id = validate_prompt_id(Some(prompt.id.clone()))
            .map_err(|error| transfer_validation_error(error, prompt_index, "id"))?
            .ok_or_else(|| {
                AppError::new("PROMPT_TRANSFER_INVALID", "Prompt ID 不能为空")
                    .with_detail("promptIndex", prompt_index.to_string())
            })?;
        if !ids.insert(id.clone()) {
            return Err(
                AppError::new("PROMPT_TRANSFER_INVALID", "Prompts 文件包含重复 ID")
                    .with_detail("promptIndex", prompt_index.to_string())
                    .with_detail("promptId", id),
            );
        }
        prompt.id = id;
        prompt.title = validate_prompt_title(&prompt.title)
            .map_err(|error| transfer_validation_error(error, prompt_index, "title"))?;
        prompt.content = validate_prompt_content(std::mem::take(&mut prompt.content))
            .map_err(|error| transfer_validation_error(error, prompt_index, "content"))?;
        if prompt.sort_order != prompt_index as i64 {
            return Err(AppError::new(
                "PROMPT_TRANSFER_INVALID",
                "sortOrder 必须从 0 开始连续排列，并与文件顺序一致",
            )
            .with_detail("promptIndex", prompt_index.to_string())
            .with_detail("sortOrder", prompt.sort_order.to_string())
            .with_detail("expectedSortOrder", prompt_index.to_string()));
        }
    }

    Ok(document)
}

pub fn classify_prompt_import_entries(
    prompts: &[PromptTransferPrompt],
    existing_prompts: &[PromptRecord],
) -> Vec<PromptImportClassification> {
    let existing_by_id = existing_prompts
        .iter()
        .map(|prompt| (prompt.id.as_str(), prompt))
        .collect::<HashMap<_, _>>();
    let mut existing_by_content = HashMap::<(&str, &str), &PromptRecord>::new();
    for prompt in existing_prompts {
        existing_by_content
            .entry((prompt.title.as_str(), prompt.content.as_str()))
            .or_insert(prompt);
    }

    let mut incoming_by_content = HashMap::<(&str, &str), &PromptTransferPrompt>::new();
    let mut classifications = Vec::with_capacity(prompts.len());
    for prompt in prompts {
        let exact_key = (prompt.title.as_str(), prompt.content.as_str());
        let classification = if let Some(previous) = incoming_by_content.get(&exact_key) {
            PromptImportClassification {
                status: PromptImportItemStatus::Duplicate,
                recommended_action: PromptImportAction::Skip,
                can_create: false,
                can_update: false,
                matched_prompt_id: Some(previous.id.clone()),
                matched_prompt_title: Some(previous.title.clone()),
                message: "导入文件中已有标题和正文完全相同的 Prompt，将跳过此重复项。".to_string(),
            }
        } else if let Some(existing) = existing_by_id.get(prompt.id.as_str()) {
            if existing.title == prompt.title && existing.content == prompt.content {
                PromptImportClassification {
                    status: PromptImportItemStatus::Existing,
                    recommended_action: PromptImportAction::Skip,
                    can_create: false,
                    can_update: false,
                    matched_prompt_id: Some(existing.id.clone()),
                    matched_prompt_title: Some(existing.title.clone()),
                    message: "本地已存在相同 ID、标题和正文，将保持现有记录。".to_string(),
                }
            } else {
                PromptImportClassification {
                    status: PromptImportItemStatus::Conflict,
                    recommended_action: PromptImportAction::Skip,
                    can_create: false,
                    can_update: true,
                    matched_prompt_id: Some(existing.id.clone()),
                    matched_prompt_title: Some(existing.title.clone()),
                    message: "相同 ID 的本地 Prompt 内容不同；默认跳过，可显式更新现有记录。"
                        .to_string(),
                }
            }
        } else if let Some(existing) = existing_by_content.get(&exact_key) {
            PromptImportClassification {
                status: PromptImportItemStatus::Duplicate,
                recommended_action: PromptImportAction::Skip,
                can_create: false,
                can_update: false,
                matched_prompt_id: Some(existing.id.clone()),
                matched_prompt_title: Some(existing.title.clone()),
                message: "本地已有不同 ID 但标题和正文完全相同的 Prompt，将跳过重复项。"
                    .to_string(),
            }
        } else {
            PromptImportClassification {
                status: PromptImportItemStatus::Ready,
                recommended_action: PromptImportAction::Create,
                can_create: true,
                can_update: false,
                matched_prompt_id: None,
                matched_prompt_title: None,
                message: "本地没有相同记录，将追加到 Prompt 列表末尾。".to_string(),
            }
        };
        incoming_by_content.entry(exact_key).or_insert(prompt);
        classifications.push(classification);
    }
    classifications
}

pub fn build_prompt_import_preview(
    source_path: String,
    file_hash: String,
    document: &PromptTransferDocument,
    existing_prompts: &[PromptRecord],
) -> PromptImportPreview {
    let classifications = classify_prompt_import_entries(&document.prompts, existing_prompts);
    let mut summary = PromptImportPreviewSummary {
        total_prompt_count: document.prompts.len(),
        ..PromptImportPreviewSummary::default()
    };
    let items = document
        .prompts
        .iter()
        .zip(classifications)
        .enumerate()
        .map(|(prompt_index, (prompt, classification))| {
            match classification.status {
                PromptImportItemStatus::Ready => summary.ready_prompt_count += 1,
                PromptImportItemStatus::Existing => summary.existing_prompt_count += 1,
                PromptImportItemStatus::Conflict => summary.conflict_prompt_count += 1,
                PromptImportItemStatus::Duplicate => summary.duplicate_prompt_count += 1,
            }
            PromptImportItemPreview {
                prompt_index,
                id: prompt.id.clone(),
                title: prompt.title.clone(),
                content_preview: prompt_content_preview(&prompt.content),
                status: classification.status,
                recommended_action: classification.recommended_action,
                can_create: classification.can_create,
                can_update: classification.can_update,
                matched_prompt_id: classification.matched_prompt_id,
                matched_prompt_title: classification.matched_prompt_title,
                message: classification.message,
            }
        })
        .collect();

    PromptImportPreview {
        source_path,
        file_hash,
        schema_version: document.schema_version,
        summary,
        items,
    }
}

pub fn validate_prompt_import_selections(
    prompt_count: usize,
    selections: Vec<PromptImportSelection>,
) -> AppResult<Vec<PromptImportSelection>> {
    if selections.len() != prompt_count {
        return Err(AppError::new(
            "PROMPT_TRANSFER_SELECTION_INVALID",
            "导入动作必须覆盖文件中的每一条 Prompt",
        )
        .with_detail("expectedCount", prompt_count.to_string())
        .with_detail("receivedCount", selections.len().to_string()));
    }
    let mut indexed = vec![None; prompt_count];
    for selection in selections {
        if selection.prompt_index >= prompt_count {
            return Err(AppError::new(
                "PROMPT_TRANSFER_SELECTION_INVALID",
                "导入动作包含无效的 Prompt 索引",
            )
            .with_detail("promptIndex", selection.prompt_index.to_string()));
        }
        let prompt_index = selection.prompt_index;
        if indexed[prompt_index].replace(selection).is_some() {
            return Err(AppError::new(
                "PROMPT_TRANSFER_SELECTION_INVALID",
                "同一 Prompt 包含重复的导入动作",
            )
            .with_detail("promptIndex", prompt_index.to_string()));
        }
    }
    indexed
        .into_iter()
        .enumerate()
        .map(|(prompt_index, selection)| {
            selection.ok_or_else(|| {
                AppError::new(
                    "PROMPT_TRANSFER_SELECTION_INVALID",
                    "导入动作缺少 Prompt 项目",
                )
                .with_detail("promptIndex", prompt_index.to_string())
            })
        })
        .collect()
}

pub fn prompt_import_action_allowed(
    classification: &PromptImportClassification,
    action: PromptImportAction,
) -> bool {
    match action {
        PromptImportAction::Skip => true,
        PromptImportAction::Create => classification.can_create,
        PromptImportAction::Update => classification.can_update,
    }
}

fn transfer_validation_error(error: AppError, prompt_index: usize, field: &str) -> AppError {
    AppError::new("PROMPT_TRANSFER_INVALID", "Prompts 文件包含无效字段")
        .with_detail("promptIndex", prompt_index.to_string())
        .with_detail("field", field)
        .with_detail("reason", error.message)
}

fn prompt_content_preview(content: &str) -> String {
    const PREVIEW_CHARS: usize = 180;
    let mut preview = content.chars().take(PREVIEW_CHARS).collect::<String>();
    if content.chars().count() > PREVIEW_CHARS {
        preview.push('…');
    }
    preview
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transfer_prompt(
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

    fn record(id: &str, title: &str, content: &str) -> PromptRecord {
        PromptRecord {
            id: id.to_string(),
            title: title.to_string(),
            content: content.to_string(),
            sort_order: 0,
            created_at: "2026-09-02T00:00:00Z".to_string(),
            updated_at: "2026-09-02T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn document_validation_preserves_content_and_requires_contiguous_order() {
        let id = uuid::Uuid::new_v4().to_string();
        let content = "\n  保留正文空白  \n";
        let document = validate_prompt_transfer_document(PromptTransferDocument {
            format: PROMPT_TRANSFER_FORMAT.to_string(),
            schema_version: PROMPT_TRANSFER_SCHEMA_VERSION,
            exported_at: "2026-09-02T00:00:00Z".to_string(),
            prompts: vec![transfer_prompt(&id, " 标题 ", content, 0)],
        })
        .expect("valid prompt document");
        assert_eq!(document.prompts[0].title, "标题");
        assert_eq!(document.prompts[0].content, content);

        let invalid = validate_prompt_transfer_document(PromptTransferDocument {
            format: PROMPT_TRANSFER_FORMAT.to_string(),
            schema_version: PROMPT_TRANSFER_SCHEMA_VERSION,
            exported_at: "2026-09-02T00:00:00Z".to_string(),
            prompts: vec![transfer_prompt(&id, "标题", "正文", 1)],
        })
        .expect_err("non-contiguous order must fail");
        assert_eq!(invalid.code, "PROMPT_TRANSFER_INVALID");
    }

    #[test]
    fn classification_is_id_safe_and_detects_file_duplicates() {
        let existing_id = uuid::Uuid::new_v4().to_string();
        let duplicate_id = uuid::Uuid::new_v4().to_string();
        let ready_id = uuid::Uuid::new_v4().to_string();
        let file_duplicate_id = uuid::Uuid::new_v4().to_string();
        let same_title_id = uuid::Uuid::new_v4().to_string();
        let existing = vec![record(&existing_id, "本地", "当前正文")];
        let imported = vec![
            transfer_prompt(&existing_id, "本地", "导入正文", 0),
            transfer_prompt(&duplicate_id, "本地", "当前正文", 1),
            transfer_prompt(&ready_id, "同名可新建", "正文一", 2),
            transfer_prompt(&file_duplicate_id, "同名可新建", "正文一", 3),
            transfer_prompt(&same_title_id, "本地", "完全不同的正文", 4),
        ];
        let classified = classify_prompt_import_entries(&imported, &existing);
        assert_eq!(classified[0].status, PromptImportItemStatus::Conflict);
        assert!(classified[0].can_update);
        assert_eq!(classified[1].status, PromptImportItemStatus::Duplicate);
        assert_eq!(classified[2].status, PromptImportItemStatus::Ready);
        assert!(classified[2].can_create);
        assert_eq!(classified[3].status, PromptImportItemStatus::Duplicate);
        assert_eq!(classified[4].status, PromptImportItemStatus::Ready);
        assert!(classified[4].can_create);
    }

    #[test]
    fn selection_validation_requires_exactly_one_action_per_item() {
        let duplicate = validate_prompt_import_selections(
            2,
            vec![
                PromptImportSelection {
                    prompt_index: 0,
                    action: PromptImportAction::Skip,
                },
                PromptImportSelection {
                    prompt_index: 0,
                    action: PromptImportAction::Create,
                },
            ],
        )
        .expect_err("duplicate selections must fail");
        assert_eq!(duplicate.code, "PROMPT_TRANSFER_SELECTION_INVALID");
    }
}
