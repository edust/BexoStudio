use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

use super::validate_optional_uuid;

pub const OSS_AUTH_MODE_ACCESS_KEY: &str = "access_key";
pub const OSS_DEFAULT_PAGE_SIZE: u16 = 100;
pub const OSS_MAX_PAGE_SIZE: u16 = 1_000;
pub const OSS_MAX_PREFIX_BYTES: usize = 1_024;
pub const OSS_MAX_OBJECT_KEY_BYTES: usize = 1_024;
pub const OSS_MAX_FOLDER_NAME_CHARS: usize = 254;
pub const OSS_MAX_ENDPOINT_BYTES: usize = 512;
pub const OSS_MAX_TRANSFER_RETRIES: u32 = 3;
pub const OSS_DEFAULT_DOWNLOAD_URL_EXPIRES_SECONDS: u64 = 900;
pub const OSS_MIN_DOWNLOAD_URL_EXPIRES_SECONDS: u64 = 60;
pub const OSS_MAX_DOWNLOAD_URL_EXPIRES_SECONDS: u64 = 604_800;
pub const OSS_TRANSFER_PROGRESS_EVENT_NAME: &str = "oss://transfer-progress";
pub const OSS_TRANSFER_STATE_EVENT_NAME: &str = "oss://transfer-state-changed";

#[derive(Debug, Clone)]
pub struct OssCredential {
    pub access_key_id: String,
    pub access_key_secret: String,
}

#[derive(Debug, Clone)]
pub struct OssAccountRecord {
    pub id: String,
    pub display_name: String,
    pub access_key_id_hint: String,
    pub credential_ref: String,
    pub last_probe_status: String,
    pub last_probe_error: Option<String>,
    pub last_probe_at: Option<String>,
    pub is_disabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OssAccountSummary {
    pub id: String,
    pub display_name: String,
    pub auth_mode: String,
    pub access_key_id_hint: String,
    pub last_probe_status: String,
    pub last_probe_error: Option<String>,
    pub last_probe_at: Option<String>,
    pub is_disabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<&OssAccountRecord> for OssAccountSummary {
    fn from(value: &OssAccountRecord) -> Self {
        Self {
            id: value.id.clone(),
            display_name: value.display_name.clone(),
            auth_mode: OSS_AUTH_MODE_ACCESS_KEY.to_string(),
            access_key_id_hint: value.access_key_id_hint.clone(),
            last_probe_status: value.last_probe_status.clone(),
            last_probe_error: value.last_probe_error.clone(),
            last_probe_at: value.last_probe_at.clone(),
            is_disabled: value.is_disabled,
            created_at: value.created_at.clone(),
            updated_at: value.updated_at.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertOssAccountInput {
    pub id: Option<String>,
    pub display_name: String,
    pub access_key_id: String,
    pub access_key_secret: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpsertOssAccountMetadataInput {
    pub id: Option<String>,
    pub display_name: String,
    pub access_key_id_hint: String,
    pub credential_ref: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OssTargetRecord {
    pub id: String,
    pub account_id: String,
    pub display_name: String,
    pub bucket: String,
    pub region: String,
    pub endpoint: String,
    pub prefix: String,
    pub is_default: bool,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertOssTargetInput {
    pub id: Option<String>,
    pub account_id: String,
    pub display_name: String,
    pub bucket: String,
    pub region: String,
    pub endpoint: String,
    pub prefix: Option<String>,
    pub is_default: Option<bool>,
    pub sort_order: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OssObjectEntry {
    pub key: String,
    pub kind: String,
    pub size: Option<u64>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub storage_class: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OssObjectPage {
    pub objects: Vec<OssObjectEntry>,
    pub common_prefixes: Vec<String>,
    pub next_continuation_token: Option<String>,
    pub is_truncated: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListOssObjectsInput {
    pub target_id: String,
    pub prefix: Option<String>,
    pub continuation_token: Option<String>,
    pub page_size: Option<u16>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateOssFolderInput {
    pub target_id: String,
    pub parent_prefix: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateOssFolderResult {
    pub target_id: String,
    pub object_key: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OssProbeResult {
    pub target_id: String,
    pub bucket: String,
    pub reachable: bool,
    pub can_list_objects: bool,
    pub message: String,
    pub checked_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OssObjectMetadata {
    pub key: String,
    pub size: Option<u64>,
    pub etag: Option<String>,
    pub content_type: Option<String>,
    pub last_modified: Option<String>,
    pub storage_class: Option<String>,
    pub version_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetOssObjectDownloadUrlInput {
    pub target_id: String,
    pub object_key: String,
    pub expires_in_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetOssObjectDownloadUrlResult {
    pub target_id: String,
    pub object_key: String,
    pub url: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadOssObjectInput {
    pub target_id: String,
    pub object_key: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestOssTargetInput {
    pub target_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OssTransferStart {
    pub operation_id: String,
    pub task_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OssTransferTaskRecord {
    pub id: String,
    pub account_id: String,
    pub target_id: String,
    pub operation: String,
    pub object_key: String,
    pub local_path: String,
    pub status: String,
    pub bytes_completed: u64,
    pub total_bytes: u64,
    #[serde(skip_serializing)]
    pub upload_id: Option<String>,
    #[serde(skip_serializing)]
    pub checkpoint_json: Option<String>,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OssTransferTaskView {
    #[serde(flatten)]
    pub task: OssTransferTaskRecord,
    pub account_display_name: String,
    pub target_display_name: String,
    pub bucket: String,
    pub region: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartOssUploadInput {
    pub target_id: String,
    pub local_path: String,
    pub object_key: String,
    pub overwrite: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartOssDownloadInput {
    pub target_id: String,
    pub object_key: String,
    pub local_path: String,
    pub overwrite: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OssOperationInput {
    pub operation_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteOssObjectsInput {
    pub target_id: String,
    pub object_keys: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyOssObjectInput {
    pub target_id: String,
    pub source_key: String,
    pub target_key: String,
    pub overwrite: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyOssObjectResult {
    pub source_key: String,
    pub target_key: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteOssObjectsResult {
    pub deleted_keys: Vec<String>,
}

pub fn normalize_oss_account_input(
    input: UpsertOssAccountInput,
) -> AppResult<(String, Option<String>, OssCredential)> {
    let id = validate_optional_uuid("id", input.id)?;
    let display_name = normalize_display_name(input.display_name)?;
    let access_key_id = normalize_access_key_id(input.access_key_id)?;
    let access_key_secret = input
        .access_key_secret
        .map(normalize_access_key_secret)
        .transpose()?;

    if id.is_none() && access_key_secret.is_none() {
        return Err(AppError::validation(
            "accessKeySecret is required when creating an OSS account",
        )
        .with_detail("field", "accessKeySecret"));
    }

    let credential = OssCredential {
        access_key_id,
        access_key_secret: access_key_secret.unwrap_or_default(),
    };

    Ok((display_name, id, credential))
}

pub fn normalize_oss_metadata_input(
    input: UpsertOssAccountMetadataInput,
) -> AppResult<UpsertOssAccountMetadataInput> {
    Ok(UpsertOssAccountMetadataInput {
        id: validate_optional_uuid("id", input.id)?,
        display_name: normalize_display_name(input.display_name)?,
        access_key_id_hint: normalize_access_key_id_hint(input.access_key_id_hint)?,
        credential_ref: normalize_credential_ref(input.credential_ref)?,
    })
}

pub fn normalize_oss_target_input(input: UpsertOssTargetInput) -> AppResult<UpsertOssTargetInput> {
    let display_name = normalize_display_name(input.display_name)?;
    let account_id = validate_required_uuid("accountId", input.account_id)?;
    let bucket = normalize_bucket(input.bucket)?;
    let region = normalize_region(input.region)?;
    let endpoint = normalize_endpoint(input.endpoint)?;
    let prefix = normalize_prefix(input.prefix.unwrap_or_default())?;
    let sort_order = input.sort_order.unwrap_or(0);
    if !(0..=1_000_000).contains(&sort_order) {
        return Err(
            AppError::validation("sortOrder is out of range").with_detail("field", "sortOrder")
        );
    }

    Ok(UpsertOssTargetInput {
        id: validate_optional_uuid("id", input.id)?,
        account_id,
        display_name,
        bucket,
        region,
        endpoint,
        prefix: Some(prefix),
        is_default: Some(input.is_default.unwrap_or(false)),
        sort_order: Some(sort_order),
    })
}

pub fn validate_account_id(value: String) -> AppResult<String> {
    validate_required_uuid("accountId", value)
}

pub fn validate_target_id(value: String) -> AppResult<String> {
    validate_required_uuid("targetId", value)
}

pub fn validate_operation_id(value: String) -> AppResult<String> {
    validate_required_uuid("operationId", value)
}

pub fn normalize_object_key(value: String) -> AppResult<String> {
    let value = value.to_string();
    if value.is_empty() {
        return Err(AppError::validation("objectKey is required").with_detail("field", "objectKey"));
    }
    if value.starts_with('/') || value.len() > OSS_MAX_OBJECT_KEY_BYTES {
        return Err(AppError::validation("objectKey is invalid").with_detail("field", "objectKey"));
    }
    if value.chars().any(|character| character.is_control()) {
        return Err(
            AppError::validation("objectKey contains unsupported control characters")
                .with_detail("field", "objectKey"),
        );
    }
    Ok(value)
}

pub fn normalize_oss_folder_name(value: String) -> AppResult<String> {
    let value = value.trim().to_string();
    if value.is_empty()
        || value.chars().count() > OSS_MAX_FOLDER_NAME_CHARS
        || value == "."
        || value == ".."
        || value.contains('/')
        || value.contains('\\')
    {
        return Err(AppError::new(
            "OSS_FOLDER_NAME_INVALID",
            "folder name must be one path segment",
        )
        .with_detail("field", "name"));
    }
    if value.chars().any(|character| character.is_control()) {
        return Err(AppError::new(
            "OSS_FOLDER_NAME_INVALID",
            "folder name contains unsupported control characters",
        )
        .with_detail("field", "name"));
    }
    Ok(value)
}

pub fn normalize_prefix(value: String) -> AppResult<String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Ok(String::new());
    }
    if value.starts_with('/') || value.len() > OSS_MAX_PREFIX_BYTES {
        return Err(AppError::validation("prefix is invalid").with_detail("field", "prefix"));
    }
    if value.chars().any(|character| character.is_control()) {
        return Err(
            AppError::validation("prefix contains unsupported control characters")
                .with_detail("field", "prefix"),
        );
    }
    if value.ends_with('/') {
        Ok(value)
    } else {
        Ok(format!("{value}/"))
    }
}

pub fn normalize_page_size(value: Option<u16>) -> AppResult<u16> {
    let value = value.unwrap_or(OSS_DEFAULT_PAGE_SIZE);
    if !(1..=OSS_MAX_PAGE_SIZE).contains(&value) {
        return Err(AppError::validation("pageSize is out of range")
            .with_detail("field", "pageSize")
            .with_detail("max", OSS_MAX_PAGE_SIZE.to_string()));
    }
    Ok(value)
}

pub fn normalize_download_url_expires(value: Option<u64>) -> AppResult<u64> {
    let value = value.unwrap_or(OSS_DEFAULT_DOWNLOAD_URL_EXPIRES_SECONDS);
    if !(OSS_MIN_DOWNLOAD_URL_EXPIRES_SECONDS..=OSS_MAX_DOWNLOAD_URL_EXPIRES_SECONDS)
        .contains(&value)
    {
        return Err(AppError::validation("expiresInSeconds is out of range")
            .with_detail("field", "expiresInSeconds")
            .with_detail("min", OSS_MIN_DOWNLOAD_URL_EXPIRES_SECONDS.to_string())
            .with_detail("max", OSS_MAX_DOWNLOAD_URL_EXPIRES_SECONDS.to_string()));
    }
    Ok(value)
}

pub fn normalize_endpoint(value: String) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > OSS_MAX_ENDPOINT_BYTES {
        return Err(AppError::validation("endpoint is invalid").with_detail("field", "endpoint"));
    }
    let candidate = if value.contains("://") {
        value.to_string()
    } else {
        format!("https://{value}")
    };
    let mut url = reqwest::Url::parse(&candidate).map_err(|error| {
        AppError::validation("endpoint is invalid")
            .with_detail("field", "endpoint")
            .with_detail("reason", error.to_string())
    })?;
    if url.scheme() != "https" {
        return Err(
            AppError::validation("endpoint must use https").with_detail("field", "endpoint")
        );
    }
    if url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || (url.path() != "" && url.path() != "/")
    {
        return Err(AppError::validation("endpoint is invalid").with_detail("field", "endpoint"));
    }
    url.set_path("");
    Ok(url.to_string().trim_end_matches('/').to_string())
}

pub fn normalize_bucket(value: String) -> AppResult<String> {
    let value = value.trim().to_string();
    let valid = (3..=63).contains(&value.len())
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
        && value
            .as_bytes()
            .first()
            .is_some_and(|value| value.is_ascii_alphanumeric())
        && value
            .as_bytes()
            .last()
            .is_some_and(|value| value.is_ascii_alphanumeric());
    if !valid {
        return Err(AppError::validation("bucket is invalid").with_detail("field", "bucket"));
    }
    Ok(value)
}

pub fn normalize_region(value: String) -> AppResult<String> {
    let value = value.trim().to_ascii_lowercase();
    let valid = (2..=64).contains(&value.len())
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        });
    if !valid {
        return Err(AppError::validation("region is invalid").with_detail("field", "region"));
    }
    if value.starts_with("oss-") {
        return Err(AppError::validation(
            "region must be an OSS region ID such as cn-shanghai, not an endpoint name",
        )
        .with_detail("field", "region")
        .with_detail("example", "cn-shanghai"));
    }
    Ok(value)
}

pub fn normalize_display_name(value: String) -> AppResult<String> {
    let value = value.trim().to_string();
    if value.is_empty() || value.chars().count() > 80 {
        return Err(
            AppError::validation("displayName is invalid").with_detail("field", "displayName")
        );
    }
    reject_unsupported_controls(&value, "displayName")?;
    Ok(value)
}

pub fn normalize_access_key_id(value: String) -> AppResult<String> {
    let value = value.trim().to_string();
    if !(8..=128).contains(&value.len()) || value.chars().any(|character| character.is_whitespace())
    {
        return Err(
            AppError::validation("accessKeyId is invalid").with_detail("field", "accessKeyId")
        );
    }
    Ok(value)
}

pub fn normalize_access_key_secret(value: String) -> AppResult<String> {
    if value.is_empty()
        || value.len() > 256
        || value.chars().any(|character| character.is_control())
    {
        return Err(AppError::validation("accessKeySecret is invalid")
            .with_detail("field", "accessKeySecret"));
    }
    Ok(value)
}

pub fn normalize_access_key_id_hint(value: String) -> AppResult<String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(AppError::validation("accessKeyIdHint is required")
            .with_detail("field", "accessKeyIdHint"));
    }
    Ok(value)
}

pub fn normalize_credential_ref(value: String) -> AppResult<String> {
    let value = value.trim().to_string();
    if value.is_empty()
        || value.len() > 256
        || value.chars().any(|character| character.is_control())
    {
        return Err(
            AppError::validation("credentialRef is invalid").with_detail("field", "credentialRef")
        );
    }
    Ok(value)
}

pub fn mask_access_key_id(value: &str) -> String {
    let characters: Vec<char> = value.chars().collect();
    if characters.len() <= 8 {
        return "••••".to_string();
    }
    let prefix: String = characters.iter().take(4).collect();
    let suffix: String = characters
        .iter()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{prefix}••••{suffix}")
}

pub fn credential_ref_for_account(account_id: &str) -> String {
    format!("BexoStudio/OSS/{account_id}")
}

fn validate_required_uuid(field: &str, value: String) -> AppResult<String> {
    validate_optional_uuid(field, Some(value))?.ok_or_else(|| {
        AppError::validation(format!("{field} is required")).with_detail("field", field)
    })
}

fn reject_unsupported_controls(value: &str, field: &str) -> AppResult<()> {
    if value.chars().any(|character| {
        character == '\0' || (character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    }) {
        return Err(AppError::validation(format!(
            "{field} contains unsupported control characters"
        ))
        .with_detail("field", field));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        mask_access_key_id, normalize_bucket, normalize_download_url_expires, normalize_endpoint,
        normalize_object_key, normalize_oss_folder_name, normalize_oss_target_input,
        normalize_prefix, normalize_region, UpsertOssTargetInput,
        OSS_DEFAULT_DOWNLOAD_URL_EXPIRES_SECONDS, OSS_MAX_DOWNLOAD_URL_EXPIRES_SECONDS,
        OSS_MIN_DOWNLOAD_URL_EXPIRES_SECONDS,
    };

    #[test]
    fn endpoint_and_prefix_are_normalized_without_silent_path_changes() {
        assert_eq!(
            normalize_endpoint("oss-cn-hangzhou.aliyuncs.com/".into()).unwrap(),
            "https://oss-cn-hangzhou.aliyuncs.com"
        );
        assert_eq!(normalize_prefix("assets".into()).unwrap(), "assets/");
        assert!(normalize_prefix("/assets".into()).is_err());
    }

    #[test]
    fn bucket_region_and_object_key_validation_rejects_unsafe_values() {
        assert!(normalize_bucket("example-bucket".into()).is_ok());
        assert!(normalize_region("cn-hangzhou".into()).is_ok());
        assert!(normalize_region("oss-cn-hangzhou".into()).is_err());
        assert!(normalize_object_key("folder/file.txt".into()).is_ok());
        assert!(normalize_object_key("/folder/file.txt".into()).is_err());
        assert!(normalize_bucket("Bad_Bucket".into()).is_err());
    }

    #[test]
    fn folder_name_validation_accepts_one_segment_and_rejects_paths() {
        assert_eq!(
            normalize_oss_folder_name(" images ".into()).unwrap(),
            "images"
        );
        assert!(normalize_oss_folder_name("nested/images".into()).is_err());
        assert!(normalize_oss_folder_name("nested\\images".into()).is_err());
        assert!(normalize_oss_folder_name(".".into()).is_err());
        assert!(normalize_oss_folder_name("..".into()).is_err());
        assert!(normalize_oss_folder_name("bad\0name".into()).is_err());
        assert!(normalize_oss_folder_name("a".repeat(255)).is_err());
    }

    #[test]
    fn target_input_normalizes_default_fields() {
        let input = normalize_oss_target_input(UpsertOssTargetInput {
            id: None,
            account_id: "550e8400-e29b-41d4-a716-446655440000".into(),
            display_name: " Assets ".into(),
            bucket: "example-bucket".into(),
            region: "CN-HANGZHOU".into(),
            endpoint: "oss-cn-hangzhou.aliyuncs.com".into(),
            prefix: Some("assets".into()),
            is_default: None,
            sort_order: None,
        })
        .unwrap();

        assert_eq!(input.prefix.as_deref(), Some("assets/"));
        assert_eq!(input.region, "cn-hangzhou");
        assert!(!input.is_default.unwrap());
    }

    #[test]
    fn access_key_hint_is_not_reversible_from_short_values() {
        assert_eq!(mask_access_key_id("short"), "••••");
        assert_eq!(mask_access_key_id("LTAI1234567890"), "LTAI••••7890");
    }

    #[test]
    fn download_url_expiration_has_safe_default_and_bounds() {
        assert_eq!(
            normalize_download_url_expires(None).unwrap(),
            OSS_DEFAULT_DOWNLOAD_URL_EXPIRES_SECONDS
        );
        assert_eq!(
            normalize_download_url_expires(Some(OSS_MIN_DOWNLOAD_URL_EXPIRES_SECONDS)).unwrap(),
            OSS_MIN_DOWNLOAD_URL_EXPIRES_SECONDS
        );
        assert_eq!(
            normalize_download_url_expires(Some(OSS_MAX_DOWNLOAD_URL_EXPIRES_SECONDS)).unwrap(),
            OSS_MAX_DOWNLOAD_URL_EXPIRES_SECONDS
        );
        assert!(normalize_download_url_expires(Some(59)).is_err());
        assert!(normalize_download_url_expires(Some(604_801)).is_err());
    }
}
