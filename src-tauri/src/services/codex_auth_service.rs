use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use chrono::Utc;
use reqwest::StatusCode;
use serde::Deserialize;

use crate::{
    domain::{
        validate_codex_auth_json, validate_codex_config_toml, CodexAuthJson,
        CodexAuthProfileRecord, CodexAuthProxyPreferences, CodexAuthQuotaRefreshBatchResult,
        CodexAuthQuotaResult, CodexAuthQuotaTier, CodexAuthSwitchResult, DeleteResult,
        UpsertCodexAuthProfileInput,
    },
    error::{AppError, AppResult},
    persistence::{
        delete_codex_auth_profile, get_codex_auth_profile, list_codex_auth_profiles,
        mark_codex_auth_profile_active, update_codex_auth_profile_quota, upsert_codex_auth_profile,
        Database,
    },
    services::PreferencesService,
};

const CODEX_QUOTA_ENDPOINT: &str = "https://chatgpt.com/backend-api/wham/usage";
const CODEX_QUOTA_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct CodexAuthService {
    database: Database,
    preferences_service: PreferencesService,
    refresh_lock: Arc<tokio::sync::Mutex<()>>,
}

impl CodexAuthService {
    pub fn new(database: Database, preferences_service: PreferencesService) -> Self {
        Self {
            database,
            preferences_service,
            refresh_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    pub async fn list_profiles(&self) -> AppResult<Vec<CodexAuthProfileRecord>> {
        self.database
            .read("list_codex_auth_profiles", list_codex_auth_profiles)
            .await
    }

    pub async fn upsert_profile(
        &self,
        input: UpsertCodexAuthProfileInput,
    ) -> AppResult<CodexAuthProfileRecord> {
        self.database
            .write("upsert_codex_auth_profile", move |connection| {
                upsert_codex_auth_profile(connection, input)
            })
            .await
    }

    pub async fn delete_profile(&self, id: String) -> AppResult<DeleteResult> {
        self.database
            .write("delete_codex_auth_profile", move |connection| {
                delete_codex_auth_profile(connection, id)
            })
            .await
    }

    pub async fn import_current_profile<R: tauri::Runtime>(
        &self,
        app_handle: &tauri::AppHandle<R>,
        preferences_service: &PreferencesService,
    ) -> AppResult<CodexAuthProfileRecord> {
        let directory = preferences_service.get_codex_home_directory(app_handle)?;
        let codex_home = directory.path.ok_or_else(|| {
            AppError::new(
                "CODEX_AUTH_HOME_UNAVAILABLE",
                "failed to resolve Codex config directory",
            )
        })?;

        let codex_home_path = PathBuf::from(&codex_home);
        let auth_path = codex_home_path.join("auth.json");
        let config_path = codex_home_path.join("config.toml");

        let auth_json = fs::read_to_string(&auth_path).map_err(|error| {
            AppError::new(
                "CODEX_AUTH_FILE_READ_FAILED",
                "failed to read Codex auth.json",
            )
            .with_detail("path", auth_path.display().to_string())
            .with_detail("reason", error.to_string())
        })?;
        let config_toml = match fs::read_to_string(&config_path) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => {
                return Err(AppError::new(
                    "CODEX_AUTH_FILE_READ_FAILED",
                    "failed to read Codex config.toml",
                )
                .with_detail("path", config_path.display().to_string())
                .with_detail("reason", error.to_string()));
            }
        };

        validate_codex_auth_json(&auth_json)?;
        validate_codex_config_toml(&config_toml)?;

        let name = build_imported_profile_name(&auth_json, &codex_home);
        self.upsert_profile(UpsertCodexAuthProfileInput {
            id: None,
            name,
            description: Some("Imported from current Codex config directory".to_string()),
            codex_home,
            auth_json,
            config_toml,
        })
        .await
    }

    pub async fn switch_profile(&self, id: String) -> AppResult<CodexAuthSwitchResult> {
        let profile = self
            .database
            .read("get_codex_auth_profile_for_switch", {
                let id = id.clone();
                move |connection| get_codex_auth_profile(connection, &id)
            })
            .await?
            .ok_or_else(|| {
                AppError::new(
                    "CODEX_AUTH_PROFILE_NOT_FOUND",
                    "codex auth profile was not found",
                )
            })?;

        validate_codex_auth_json(&profile.auth_json)?;
        validate_codex_config_toml(&profile.config_toml)?;

        let codex_home = PathBuf::from(&profile.codex_home);
        let auth_path = codex_home.join("auth.json");
        let config_path = codex_home.join("config.toml");

        write_live_codex_config(&codex_home, &profile.auth_json, &profile.config_toml)?;

        let active_profile = self
            .database
            .write("mark_codex_auth_profile_active", move |connection| {
                mark_codex_auth_profile_active(connection, id)
            })
            .await?;

        Ok(CodexAuthSwitchResult {
            profile: active_profile,
            auth_path: auth_path.display().to_string(),
            config_path: config_path.display().to_string(),
        })
    }

    pub async fn query_quota(&self, id: String) -> AppResult<CodexAuthProfileRecord> {
        let profile = self
            .database
            .read("get_codex_auth_profile_for_quota", {
                let id = id.clone();
                move |connection| get_codex_auth_profile(connection, &id)
            })
            .await?
            .ok_or_else(|| {
                AppError::new(
                    "CODEX_AUTH_PROFILE_NOT_FOUND",
                    "codex auth profile was not found",
                )
            })?;

        let quota = self.query_quota_for_profile(&profile).await;
        self.database
            .write("update_codex_auth_profile_quota", move |connection| {
                update_codex_auth_profile_quota(connection, id, quota)
            })
            .await
    }

    pub async fn refresh_all_quotas(&self) -> AppResult<CodexAuthQuotaRefreshBatchResult> {
        let _refresh_guard = self.refresh_lock.lock().await;
        let started_at = Utc::now().to_rfc3339();
        let profiles = self.list_profiles().await?;
        let total = profiles.len();
        let mut failed = 0usize;
        let mut refreshed_profiles = Vec::with_capacity(total);

        for profile in profiles {
            let quota = self.query_quota_for_profile(&profile).await;
            if !quota.success {
                failed += 1;
            }

            let profile_id = profile.id.clone();
            let refreshed_profile = self
                .database
                .write(
                    "refresh_all_codex_auth_quotas_update_profile",
                    move |connection| {
                        update_codex_auth_profile_quota(connection, profile_id, quota)
                    },
                )
                .await?;
            refreshed_profiles.push(refreshed_profile);
        }

        Ok(CodexAuthQuotaRefreshBatchResult {
            total,
            refreshed: total.saturating_sub(failed),
            failed,
            started_at,
            finished_at: Utc::now().to_rfc3339(),
            profiles: refreshed_profiles,
        })
    }

    async fn query_quota_for_profile(
        &self,
        profile: &CodexAuthProfileRecord,
    ) -> CodexAuthQuotaResult {
        let queried_at = Utc::now().to_rfc3339();
        let credentials = match parse_codex_credentials_from_profile(profile) {
            Ok(value) => value,
            Err(error) => {
                return CodexAuthQuotaResult {
                    profile_id: profile.id.clone(),
                    success: false,
                    credential_status: "parse_error".to_string(),
                    credential_message: Some(error.message),
                    tiers: vec![],
                    error: Some(error.code),
                    queried_at,
                };
            }
        };

        let (access_token, account_id, stale_message) = credentials;
        let preferences = match self.preferences_service.get_preferences() {
            Ok(value) => value,
            Err(error) => {
                return CodexAuthQuotaResult {
                    profile_id: profile.id.clone(),
                    success: false,
                    credential_status: "valid".to_string(),
                    credential_message: stale_message,
                    tiers: vec![],
                    error: Some(format!("Codex Auth proxy preferences unavailable: {error}")),
                    queried_at,
                };
            }
        };
        let http_client = match build_codex_quota_http_client(&preferences.codex_auth.proxy) {
            Ok(value) => value,
            Err(error) => {
                return CodexAuthQuotaResult {
                    profile_id: profile.id.clone(),
                    success: false,
                    credential_status: "valid".to_string(),
                    credential_message: stale_message,
                    tiers: vec![],
                    error: Some(format!("Codex Auth proxy configuration error: {error}")),
                    queried_at,
                };
            }
        };

        let mut request = http_client
            .get(CODEX_QUOTA_ENDPOINT)
            .timeout(CODEX_QUOTA_TIMEOUT)
            .header("Authorization", format!("Bearer {access_token}"))
            .header("User-Agent", "codex-cli")
            .header("Accept", "application/json");

        if let Some(account_id) = account_id.as_deref().filter(|value| !value.is_empty()) {
            request = request.header("ChatGPT-Account-Id", account_id);
        }

        let response = match request.send().await {
            Ok(value) => value,
            Err(error) => {
                return CodexAuthQuotaResult {
                    profile_id: profile.id.clone(),
                    success: false,
                    credential_status: "valid".to_string(),
                    credential_message: stale_message,
                    tiers: vec![],
                    error: Some(format!("Network error: {error}")),
                    queried_at,
                };
            }
        };

        let status = response.status();
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            return CodexAuthQuotaResult {
                profile_id: profile.id.clone(),
                success: false,
                credential_status: "expired".to_string(),
                credential_message: Some(
                    "Codex OAuth token was rejected. Please re-login with Codex CLI.".to_string(),
                ),
                tiers: vec![],
                error: Some(format!("Authentication failed (HTTP {status})")),
                queried_at,
            };
        }

        if !status.is_success() {
            return CodexAuthQuotaResult {
                profile_id: profile.id.clone(),
                success: false,
                credential_status: "valid".to_string(),
                credential_message: stale_message,
                tiers: vec![],
                error: Some(format!("Codex quota API failed (HTTP {status})")),
                queried_at,
            };
        }

        let body = match response.json::<CodexUsageResponse>().await {
            Ok(value) => value,
            Err(error) => {
                return CodexAuthQuotaResult {
                    profile_id: profile.id.clone(),
                    success: false,
                    credential_status: "valid".to_string(),
                    credential_message: stale_message,
                    tiers: vec![],
                    error: Some(format!("Failed to parse quota response: {error}")),
                    queried_at,
                };
            }
        };

        CodexAuthQuotaResult {
            profile_id: profile.id.clone(),
            success: true,
            credential_status: "valid".to_string(),
            credential_message: None,
            tiers: usage_response_to_tiers(body),
            error: None,
            queried_at,
        }
    }
}

fn build_codex_quota_http_client(proxy: &CodexAuthProxyPreferences) -> AppResult<reqwest::Client> {
    let mode = proxy.mode.trim().to_ascii_lowercase();
    let mut builder = reqwest::Client::builder();

    match mode.as_str() {
        "system" => {}
        "disabled" => {
            builder = builder.no_proxy();
        }
        "manual" => {
            let manual_proxy_url = proxy.manual_proxy_url.trim();
            if manual_proxy_url.is_empty() {
                return Err(AppError::validation("手动代理模式必须填写代理地址")
                    .with_detail("field", "codexAuth.proxy.manualProxyUrl"));
            }
            let lower_proxy_url = manual_proxy_url.to_ascii_lowercase();
            if !lower_proxy_url.starts_with("http://")
                && !lower_proxy_url.starts_with("https://")
                && !lower_proxy_url.starts_with("socks5://")
                && !lower_proxy_url.starts_with("socks5h://")
            {
                return Err(AppError::validation(
                    "Codex Auth 手动代理只支持 http、https、socks5、socks5h",
                )
                .with_detail("field", "codexAuth.proxy.manualProxyUrl"));
            }
            let manual_proxy = reqwest::Proxy::all(manual_proxy_url).map_err(|error| {
                AppError::validation("Codex Auth 手动代理地址无效")
                    .with_detail("field", "codexAuth.proxy.manualProxyUrl")
                    .with_detail("reason", error.to_string())
            })?;
            builder = builder.no_proxy().proxy(manual_proxy);
        }
        _ => {
            return Err(AppError::validation("Codex Auth 额度代理模式无效")
                .with_detail("field", "codexAuth.proxy.mode")
                .with_detail("allowed", "system, manual, disabled"));
        }
    }

    builder.build().map_err(|error| {
        AppError::new(
            "CODEX_AUTH_HTTP_CLIENT_FAILED",
            "failed to build Codex Auth quota HTTP client",
        )
        .with_detail("reason", error.to_string())
    })
}

fn build_imported_profile_name(auth_json: &str, codex_home: &str) -> String {
    let parsed = serde_json::from_str::<CodexAuthJson>(auth_json).ok();
    let suffix = parsed
        .and_then(|auth| auth.tokens)
        .and_then(|tokens| tokens.account_id)
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.chars().take(8).collect::<String>())
        .or_else(|| {
            Path::new(codex_home)
                .file_name()
                .and_then(|value| value.to_str())
                .map(|value| value.to_string())
        })
        .unwrap_or_else(|| "default".to_string());
    format!("Codex Auth {suffix} {}", Utc::now().format("%Y%m%d-%H%M%S"))
}

fn parse_codex_credentials_from_profile(
    profile: &CodexAuthProfileRecord,
) -> AppResult<(String, Option<String>, Option<String>)> {
    let auth: CodexAuthJson = serde_json::from_str(&profile.auth_json).map_err(|error| {
        AppError::new("CODEX_AUTH_PARSE_FAILED", "failed to parse auth.json")
            .with_detail("reason", error.to_string())
    })?;

    if auth.auth_mode.as_deref() != Some("chatgpt") {
        return Err(AppError::new(
            "CODEX_AUTH_QUOTA_UNSUPPORTED",
            "only Codex ChatGPT OAuth auth.json can query official quota",
        ));
    }

    let tokens = auth.tokens.ok_or_else(|| {
        AppError::new(
            "CODEX_AUTH_PARSE_FAILED",
            "auth.json does not contain tokens",
        )
    })?;
    let access_token = tokens
        .access_token
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            AppError::new(
                "CODEX_AUTH_PARSE_FAILED",
                "auth.json tokens.access_token is missing",
            )
        })?;

    let stale_message = auth
        .last_refresh
        .as_deref()
        .filter(|last_refresh| is_codex_token_stale(last_refresh))
        .map(|_| "Codex token may be stale because last_refresh is older than 8 days".to_string());

    Ok((access_token, tokens.account_id, stale_message))
}

fn write_live_codex_config(codex_home: &Path, auth_json: &str, config_toml: &str) -> AppResult<()> {
    fs::create_dir_all(codex_home).map_err(|error| {
        AppError::new(
            "CODEX_AUTH_SWITCH_FAILED",
            "failed to create Codex config directory",
        )
        .with_detail("path", codex_home.display().to_string())
        .with_detail("reason", error.to_string())
    })?;

    let auth_path = codex_home.join("auth.json");
    let config_path = codex_home.join("config.toml");
    let previous_auth = read_existing_file(&auth_path)?;
    let previous_config = read_existing_file(&config_path)?;

    if let Err(error) = write_file_replace(&auth_path, auth_json) {
        rollback_file(&auth_path, previous_auth.as_deref());
        rollback_file(&config_path, previous_config.as_deref());
        return Err(error);
    }
    if let Err(error) = write_file_replace(&config_path, config_toml) {
        rollback_file(&auth_path, previous_auth.as_deref());
        rollback_file(&config_path, previous_config.as_deref());
        return Err(error);
    }

    Ok(())
}

fn read_existing_file(path: &Path) -> AppResult<Option<String>> {
    match fs::read_to_string(path) {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(AppError::new(
            "CODEX_AUTH_SWITCH_FAILED",
            "failed to read existing Codex config file",
        )
        .with_detail("path", path.display().to_string())
        .with_detail("reason", error.to_string())),
    }
}

fn write_file_replace(path: &Path, content: &str) -> AppResult<()> {
    let parent = path.parent().ok_or_else(|| {
        AppError::new(
            "CODEX_AUTH_SWITCH_FAILED",
            "Codex config file path has no parent directory",
        )
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        AppError::new(
            "CODEX_AUTH_SWITCH_FAILED",
            "failed to create Codex config file parent directory",
        )
        .with_detail("path", parent.display().to_string())
        .with_detail("reason", error.to_string())
    })?;

    let filename = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("codex-config");
    let temp_path = parent.join(format!(
        ".{filename}.tmp.{}",
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));

    fs::write(&temp_path, content).map_err(|error| {
        AppError::new(
            "CODEX_AUTH_SWITCH_FAILED",
            "failed to write temporary Codex config file",
        )
        .with_detail("path", temp_path.display().to_string())
        .with_detail("reason", error.to_string())
    })?;

    if path.exists() {
        fs::remove_file(path).map_err(|error| {
            let _ = fs::remove_file(&temp_path);
            AppError::new(
                "CODEX_AUTH_SWITCH_FAILED",
                "failed to replace existing Codex config file",
            )
            .with_detail("path", path.display().to_string())
            .with_detail("reason", error.to_string())
        })?;
    }

    fs::rename(&temp_path, path).map_err(|error| {
        let _ = fs::remove_file(&temp_path);
        AppError::new(
            "CODEX_AUTH_SWITCH_FAILED",
            "failed to move Codex config file into place",
        )
        .with_detail("path", path.display().to_string())
        .with_detail("reason", error.to_string())
    })
}

fn rollback_file(path: &Path, previous: Option<&str>) {
    match previous {
        Some(content) => {
            if let Err(error) = fs::write(path, content) {
                log::error!(
                    target: "bexo::service::codex_auth",
                    "rollback Codex config file failed path={} reason={}",
                    path.display(),
                    error
                );
            }
        }
        None => {
            if path.exists() {
                let _ = fs::remove_file(path);
            }
        }
    }
}

fn is_codex_token_stale(last_refresh: &str) -> bool {
    let Ok(dt) = chrono::DateTime::parse_from_rfc3339(last_refresh) else {
        return false;
    };
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let age_secs = now_secs.saturating_sub(dt.timestamp() as u64);
    age_secs > 8 * 24 * 3600
}

#[derive(Debug, Deserialize)]
struct CodexUsageResponse {
    rate_limit: Option<CodexRateLimit>,
}

#[derive(Debug, Deserialize)]
struct CodexRateLimit {
    primary_window: Option<CodexRateLimitWindow>,
    secondary_window: Option<CodexRateLimitWindow>,
}

#[derive(Debug, Deserialize)]
struct CodexRateLimitWindow {
    used_percent: Option<f64>,
    limit_window_seconds: Option<i64>,
    reset_at: Option<i64>,
}

fn usage_response_to_tiers(response: CodexUsageResponse) -> Vec<CodexAuthQuotaTier> {
    let mut tiers = Vec::new();
    if let Some(rate_limit) = response.rate_limit {
        for window in [rate_limit.primary_window, rate_limit.secondary_window]
            .into_iter()
            .flatten()
        {
            if let Some(utilization) = window.used_percent {
                tiers.push(CodexAuthQuotaTier {
                    name: window
                        .limit_window_seconds
                        .map(window_seconds_to_tier_name)
                        .unwrap_or_else(|| "unknown".to_string()),
                    utilization,
                    resets_at: window.reset_at.and_then(unix_ts_to_iso),
                });
            }
        }
    }
    tiers
}

fn window_seconds_to_tier_name(seconds: i64) -> String {
    match seconds {
        18_000 => "five_hour".to_string(),
        604_800 => "seven_day".to_string(),
        value => {
            let hours = value / 3600;
            if hours >= 24 {
                format!("{}_day", hours / 24)
            } else {
                format!("{}_hour", hours)
            }
        }
    }
}

fn unix_ts_to_iso(timestamp: i64) -> Option<String> {
    chrono::DateTime::from_timestamp(timestamp, 0).map(|value| value.to_rfc3339())
}
