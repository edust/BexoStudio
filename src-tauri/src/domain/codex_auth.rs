use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAuthProfileRecord {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub codex_home: String,
    pub auth_json: String,
    pub config_toml: String,
    pub is_active: bool,
    pub last_quota: Option<CodexAuthQuotaResult>,
    pub last_quota_checked_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAuthProfileSummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub codex_home: String,
    pub is_active: bool,
    pub last_quota: Option<CodexAuthQuotaResult>,
    pub last_quota_checked_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAuthProfileDetail {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub codex_home: String,
    pub auth_json: String,
    pub config_toml: String,
    pub is_active: bool,
    pub last_quota: Option<CodexAuthQuotaResult>,
    pub last_quota_checked_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<&CodexAuthProfileRecord> for CodexAuthProfileSummary {
    fn from(profile: &CodexAuthProfileRecord) -> Self {
        Self {
            id: profile.id.clone(),
            name: profile.name.clone(),
            description: profile.description.clone(),
            codex_home: profile.codex_home.clone(),
            is_active: profile.is_active,
            last_quota: profile.last_quota.clone(),
            last_quota_checked_at: profile.last_quota_checked_at.clone(),
            created_at: profile.created_at.clone(),
            updated_at: profile.updated_at.clone(),
        }
    }
}

impl From<CodexAuthProfileRecord> for CodexAuthProfileDetail {
    fn from(profile: CodexAuthProfileRecord) -> Self {
        Self {
            id: profile.id,
            name: profile.name,
            description: profile.description,
            codex_home: profile.codex_home,
            auth_json: profile.auth_json,
            config_toml: profile.config_toml,
            is_active: profile.is_active,
            last_quota: profile.last_quota,
            last_quota_checked_at: profile.last_quota_checked_at,
            created_at: profile.created_at,
            updated_at: profile.updated_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertCodexAuthProfileInput {
    pub id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub codex_home: String,
    pub auth_json: String,
    pub config_toml: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAuthQuotaTier {
    pub name: String,
    pub utilization: f64,
    pub resets_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAuthQuotaResult {
    pub profile_id: String,
    pub success: bool,
    pub credential_status: String,
    pub credential_message: Option<String>,
    pub tiers: Vec<CodexAuthQuotaTier>,
    pub error: Option<String>,
    pub queried_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAuthSwitchResult {
    pub profile: CodexAuthProfileSummary,
    pub auth_path: String,
    pub config_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexAuthQuotaRefreshBatchResult {
    pub total: usize,
    pub refreshed: usize,
    pub failed: usize,
    pub started_at: String,
    pub finished_at: String,
    pub profiles: Vec<CodexAuthProfileSummary>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CodexAuthJson {
    pub auth_mode: Option<String>,
    pub tokens: Option<CodexAuthTokens>,
    pub last_refresh: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CodexAuthTokens {
    pub access_token: Option<String>,
    pub account_id: Option<String>,
}

pub fn validate_codex_auth_json(value: &str) -> AppResult<()> {
    let parsed: serde_json::Value = serde_json::from_str(value).map_err(|error| {
        AppError::new("CODEX_AUTH_PARSE_FAILED", "auth.json is not valid JSON")
            .with_detail("reason", error.to_string())
    })?;

    if !parsed.is_object() {
        return Err(AppError::new(
            "CODEX_AUTH_PARSE_FAILED",
            "auth.json must contain a JSON object",
        ));
    }

    Ok(())
}

pub fn validate_codex_config_toml(value: &str) -> AppResult<()> {
    toml::from_str::<toml::Table>(value).map_err(|error| {
        AppError::new("CODEX_CONFIG_PARSE_FAILED", "config.toml is not valid TOML")
            .with_detail("reason", error.to_string())
    })?;
    Ok(())
}

pub fn ensure_absolute_path(value: &str, error_code: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation("codexHome is required"));
    }

    if !std::path::Path::new(trimmed).is_absolute() {
        return Err(
            AppError::new(error_code, "codexHome must be an absolute path")
                .with_detail("path", trimmed.to_string()),
        );
    }

    Ok(trimmed.to_string())
}
