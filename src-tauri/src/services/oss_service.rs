use std::sync::Arc;

use crate::{
    adapters::{
        AlibabaOssAdapter, ObjectStorageAdapter, OssObjectCopyRequest, OssObjectDeleteRequest,
        OssObjectHeadRequest, OssObjectListRequest, OssObjectPutRequest,
    },
    domain::{
        credential_ref_for_account, mask_access_key_id, normalize_download_url_expires,
        normalize_object_key, normalize_oss_account_input, normalize_oss_folder_name,
        normalize_oss_metadata_input, normalize_oss_target_input, normalize_page_size,
        normalize_prefix, validate_account_id, validate_target_id, CopyOssObjectInput,
        CopyOssObjectResult, CreateOssFolderInput, CreateOssFolderResult, DeleteOssObjectsInput,
        DeleteOssObjectsResult, DeleteResult, GetOssObjectDownloadUrlInput,
        GetOssObjectDownloadUrlResult, HeadOssObjectInput, ListOssObjectsInput, OssAccountRecord,
        OssAccountSummary, OssCredential, OssObjectMetadata, OssObjectPage, OssProbeResult,
        OssTargetRecord, TestOssTargetInput, UpsertOssAccountInput, UpsertOssTargetInput,
    },
    error::{AppError, AppResult},
    persistence::{
        delete_oss_account, delete_oss_target, ensure_oss_account_deletable,
        ensure_oss_target_identity_editable, get_oss_account, get_oss_target, list_oss_accounts,
        list_oss_targets, update_oss_account_probe, upsert_oss_account, upsert_oss_target,
        Database,
    },
    services::OssCredentialService,
};
use chrono::{Duration, Utc};

const MAX_CONTINUATION_TOKEN_BYTES: usize = 4 * 1024;

#[derive(Clone)]
pub struct OssService {
    database: Database,
    credential_service: OssCredentialService,
    adapter: Arc<AlibabaOssAdapter>,
}

impl OssService {
    pub fn new(database: Database) -> AppResult<Self> {
        Ok(Self {
            database,
            credential_service: OssCredentialService::new(),
            adapter: Arc::new(AlibabaOssAdapter::new()?),
        })
    }

    pub async fn list_accounts(&self) -> AppResult<Vec<OssAccountSummary>> {
        self.database
            .read("list_oss_accounts", list_oss_accounts)
            .await
            .map(|accounts| accounts.iter().map(OssAccountSummary::from).collect())
    }

    pub async fn upsert_account(
        &self,
        input: UpsertOssAccountInput,
    ) -> AppResult<OssAccountSummary> {
        let (display_name, requested_id, mut credential) = normalize_oss_account_input(input)?;
        let account_id = requested_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let existing = if let Some(id) = requested_id {
            self.database
                .read("get_oss_account_for_update", {
                    let id = id.clone();
                    move |connection| get_oss_account(connection, id)
                })
                .await?
                .ok_or_else(|| {
                    AppError::new("OSS_ACCOUNT_NOT_FOUND", "OSS account was not found")
                        .with_detail("accountId", account_id.clone())
                })?
                .into()
        } else {
            None
        };

        let credential_ref = existing
            .as_ref()
            .map(|account: &OssAccountRecord| account.credential_ref.clone())
            .unwrap_or_else(|| credential_ref_for_account(&account_id));
        let previous_credential = if let Some(account) = existing.as_ref() {
            Some(self.read_credential(account.credential_ref.clone()).await?)
        } else {
            None
        };
        if credential.access_key_secret.is_empty() {
            credential.access_key_secret = previous_credential
                .as_ref()
                .map(|value| value.access_key_secret.clone())
                .ok_or_else(|| {
                    AppError::validation(
                        "accessKeySecret is required when no saved OSS credential exists",
                    )
                    .with_detail("field", "accessKeySecret")
                })?;
        }
        let metadata =
            normalize_oss_metadata_input(crate::domain::UpsertOssAccountMetadataInput {
                id: Some(account_id.clone()),
                display_name,
                access_key_id_hint: mask_access_key_id(&credential.access_key_id),
                credential_ref: credential_ref.clone(),
            })?;

        self.write_credential(credential_ref.clone(), credential.clone())
            .await?;

        let saved = self
            .database
            .write("upsert_oss_account", move |connection| {
                upsert_oss_account(connection, metadata)
            })
            .await;
        match saved {
            Ok(account) => Ok(OssAccountSummary::from(&account)),
            Err(error) => {
                let rollback = match previous_credential {
                    Some(previous) => self.write_credential(credential_ref, previous).await,
                    None => self.delete_credential(credential_ref).await,
                };
                if let Err(rollback_error) = rollback {
                    log::error!(
                        target: "bexo::service::oss",
                        "OSS credential rollback failed account_id={} code={} reason={}",
                        account_id,
                        rollback_error.code,
                        rollback_error.message
                    );
                    return Err(error.with_detail(
                        "credentialRollback",
                        "failed; secure credential state requires repair",
                    ));
                }
                Err(error)
            }
        }
    }

    pub async fn delete_account(&self, account_id: String) -> AppResult<DeleteResult> {
        let account_id = validate_account_id(account_id)?;
        let account = self
            .database
            .read("get_oss_account_for_delete", {
                let account_id = account_id.clone();
                move |connection| get_oss_account(connection, account_id)
            })
            .await?
            .ok_or_else(|| {
                AppError::new("OSS_ACCOUNT_NOT_FOUND", "OSS account was not found")
                    .with_detail("accountId", account_id.clone())
            })?;
        self.database
            .read("ensure_oss_account_deletable", {
                let account_id = account_id.clone();
                move |connection| ensure_oss_account_deletable(connection, account_id)
            })
            .await?;
        let previous = self.read_credential(account.credential_ref.clone()).await?;
        self.delete_credential(account.credential_ref.clone())
            .await?;

        let result = self
            .database
            .write("delete_oss_account", {
                let account_id = account_id.clone();
                move |connection| delete_oss_account(connection, account_id)
            })
            .await;
        match result {
            Ok(result) => Ok(result),
            Err(error) => {
                if let Err(rollback_error) = self
                    .write_credential(account.credential_ref, previous)
                    .await
                {
                    log::error!(
                        target: "bexo::service::oss",
                        "OSS credential restore after account deletion failure failed account_id={} code={} reason={}",
                        account_id,
                        rollback_error.code,
                        rollback_error.message
                    );
                    return Err(error.with_detail(
                        "credentialRollback",
                        "failed; secure credential state requires repair",
                    ));
                }
                Err(error)
            }
        }
    }

    pub async fn list_targets(&self, account_id: String) -> AppResult<Vec<OssTargetRecord>> {
        let account_id = validate_account_id(account_id)?;
        self.ensure_account_exists(account_id.clone()).await?;
        self.database
            .read("list_oss_targets", move |connection| {
                list_oss_targets(connection, account_id)
            })
            .await
    }

    pub async fn upsert_target(&self, input: UpsertOssTargetInput) -> AppResult<OssTargetRecord> {
        let input = normalize_oss_target_input(input)?;
        self.ensure_account_exists(input.account_id.clone()).await?;
        if let Some(target_id) = input.id.clone() {
            if let Some(existing) = self
                .database
                .read("get_oss_target_for_account_move_check", {
                    let target_id = target_id.clone();
                    move |connection| get_oss_target(connection, target_id)
                })
                .await?
            {
                if existing.account_id != input.account_id {
                    return Err(AppError::new(
                        "OSS_TARGET_ACCOUNT_IMMUTABLE",
                        "an OSS target cannot be moved to another account",
                    )
                    .with_detail("targetId", target_id));
                }
                let next_prefix = input.prefix.clone().unwrap_or_default();
                if existing.bucket != input.bucket
                    || existing.region != input.region
                    || existing.endpoint != input.endpoint
                    || existing.prefix != next_prefix
                {
                    self.database
                        .read("ensure_oss_target_identity_editable", {
                            let target_id = target_id.clone();
                            move |connection| {
                                ensure_oss_target_identity_editable(connection, target_id)
                            }
                        })
                        .await?;
                }
            }
        }
        self.database
            .write("upsert_oss_target", move |connection| {
                upsert_oss_target(connection, input)
            })
            .await
    }

    pub async fn delete_target(&self, target_id: String) -> AppResult<DeleteResult> {
        let target_id = validate_target_id(target_id)?;
        self.database
            .write("delete_oss_target", move |connection| {
                delete_oss_target(connection, target_id)
            })
            .await
    }

    pub async fn list_objects(&self, input: ListOssObjectsInput) -> AppResult<OssObjectPage> {
        let target_id = validate_target_id(input.target_id)?;
        let target = self.load_target(target_id).await?;
        let account = self.load_account(target.account_id.clone()).await?;
        if account.is_disabled {
            return Err(
                AppError::new("OSS_ACCOUNT_DISABLED", "OSS account is disabled")
                    .with_detail("accountId", account.id),
            );
        }
        let prefix = normalize_prefix(input.prefix.unwrap_or_else(|| target.prefix.clone()))?;
        ensure_prefix_within_target(&target, &prefix)?;
        let continuation_token = validate_continuation_token(input.continuation_token)?;
        let credential = self.read_credential(account.credential_ref).await?;
        self.adapter
            .list_objects(OssObjectListRequest {
                target,
                credential,
                prefix,
                continuation_token,
                page_size: normalize_page_size(input.page_size)?,
            })
            .await
    }

    pub async fn head_object(&self, input: HeadOssObjectInput) -> AppResult<OssObjectMetadata> {
        let target_id = validate_target_id(input.target_id)?;
        let object_key = normalize_object_key(input.object_key)?;
        let target = self.load_target(target_id).await?;
        ensure_key_within_target(&target, &object_key)?;
        let account = self.load_account(target.account_id.clone()).await?;
        let credential = self.read_credential(account.credential_ref).await?;
        self.adapter
            .head_object(OssObjectHeadRequest {
                target,
                credential,
                object_key,
            })
            .await
    }

    pub async fn get_object_download_url(
        &self,
        input: GetOssObjectDownloadUrlInput,
    ) -> AppResult<GetOssObjectDownloadUrlResult> {
        let target_id = validate_target_id(input.target_id)?;
        let object_key = normalize_object_key(input.object_key)?;
        let expires_in_seconds = normalize_download_url_expires(input.expires_in_seconds)?;
        let target = self.load_target(target_id.clone()).await?;
        ensure_key_within_target(&target, &object_key)?;
        let account = self.load_account(target.account_id.clone()).await?;
        if account.is_disabled {
            return Err(
                AppError::new("OSS_ACCOUNT_DISABLED", "OSS account is disabled")
                    .with_detail("accountId", account.id),
            );
        }
        let credential = self.read_credential(account.credential_ref).await?;
        let now = Utc::now();
        let expires_at = now
            + Duration::seconds(i64::try_from(expires_in_seconds).map_err(|error| {
                AppError::validation("expiresInSeconds is invalid")
                    .with_detail("reason", error.to_string())
            })?);
        let url = self.adapter.presign_get_url(
            &target,
            &credential,
            &object_key,
            expires_in_seconds,
            now,
        )?;

        Ok(GetOssObjectDownloadUrlResult {
            target_id,
            object_key,
            url,
            expires_at: expires_at.to_rfc3339(),
        })
    }

    pub async fn create_folder(
        &self,
        input: CreateOssFolderInput,
    ) -> AppResult<CreateOssFolderResult> {
        let target_id = validate_target_id(input.target_id)?;
        let target = self.load_target(target_id.clone()).await?;
        let parent_prefix = normalize_prefix(input.parent_prefix)?;
        ensure_prefix_within_target(&target, &parent_prefix)?;
        let folder_name = normalize_oss_folder_name(input.name)?;
        let object_key = normalize_object_key(format!("{parent_prefix}{folder_name}/"))?;
        ensure_key_within_target(&target, &object_key)?;

        let account = self.load_account(target.account_id.clone()).await?;
        if account.is_disabled {
            return Err(
                AppError::new("OSS_ACCOUNT_DISABLED", "OSS account is disabled")
                    .with_detail("accountId", account.id),
            );
        }
        let credential = self.read_credential(account.credential_ref).await?;
        if let Err(error) = self
            .adapter
            .put_object(OssObjectPutRequest {
                target,
                credential,
                object_key: object_key.clone(),
                body: Vec::new(),
                content_type: None,
                overwrite: false,
            })
            .await
        {
            if error.code == "OSS_UPLOAD_CONFLICT" {
                return Err(AppError::new(
                    "OSS_FOLDER_ALREADY_EXISTS",
                    "OSS folder already exists",
                )
                .with_detail("targetId", target_id)
                .with_detail("objectKey", object_key));
            }
            return Err(error);
        }

        Ok(CreateOssFolderResult {
            target_id,
            object_key,
        })
    }

    pub async fn test_target(&self, input: TestOssTargetInput) -> AppResult<OssProbeResult> {
        let target_id = validate_target_id(input.target_id)?;
        let target = self.load_target(target_id.clone()).await?;
        let account = self.load_account(target.account_id.clone()).await?;
        let credential = self.read_credential(account.credential_ref).await?;
        let checked_at = Utc::now().to_rfc3339();
        let result = self
            .adapter
            .list_objects(OssObjectListRequest {
                target: target.clone(),
                credential,
                prefix: target.prefix.clone(),
                continuation_token: None,
                page_size: 1,
            })
            .await;
        match result {
            Ok(_) => {
                if let Err(update_error) = self
                    .database
                    .write("update_oss_account_probe_success", {
                        let account_id = account.id.clone();
                        move |connection| {
                            update_oss_account_probe(
                                connection,
                                account_id,
                                "success".to_string(),
                                None,
                            )
                            .map(|_| ())
                        }
                    })
                    .await
                {
                    log::warn!(
                        target: "bexo::service::oss",
                        "failed to persist OSS probe success target_id={} code={} reason={}",
                        target_id,
                        update_error.code,
                        update_error.message
                    );
                }
                Ok(OssProbeResult {
                    target_id,
                    bucket: target.bucket,
                    reachable: true,
                    can_list_objects: true,
                    message: "OSS target is reachable".to_string(),
                    checked_at,
                })
            }
            Err(error) => {
                let probe_message = error.message.clone();
                if let Err(update_error) = self
                    .database
                    .write("update_oss_account_probe_failure", {
                        let account_id = account.id.clone();
                        let probe_message = probe_message.clone();
                        move |connection| {
                            update_oss_account_probe(
                                connection,
                                account_id,
                                "failed".to_string(),
                                Some(probe_message),
                            )
                            .map(|_| ())
                        }
                    })
                    .await
                {
                    log::warn!(
                        target: "bexo::service::oss",
                        "failed to persist OSS probe failure target_id={} code={} reason={}",
                        target_id,
                        update_error.code,
                        update_error.message
                    );
                }
                Err(error.with_detail("targetId", target_id))
            }
        }
    }

    pub async fn delete_objects(
        &self,
        input: DeleteOssObjectsInput,
    ) -> AppResult<DeleteOssObjectsResult> {
        if input.object_keys.is_empty() || input.object_keys.len() > 100 {
            return Err(
                AppError::validation("objectKeys must contain between 1 and 100 objects")
                    .with_detail("field", "objectKeys"),
            );
        }
        let target = self
            .load_target(validate_target_id(input.target_id)?)
            .await?;
        let account = self.load_account(target.account_id.clone()).await?;
        let credential = self.read_credential(account.credential_ref).await?;
        let mut object_keys = Vec::with_capacity(input.object_keys.len());
        for object_key in input.object_keys {
            let object_key = normalize_object_key(object_key)?;
            ensure_key_within_target(&target, &object_key)?;
            if !object_keys.contains(&object_key) {
                object_keys.push(object_key);
            }
        }
        let mut deleted_keys = Vec::with_capacity(object_keys.len());
        for object_key in object_keys {
            if let Err(error) = self
                .adapter
                .delete_object(OssObjectDeleteRequest {
                    target: target.clone(),
                    credential: credential.clone(),
                    object_key: object_key.clone(),
                })
                .await
            {
                return Err(error
                    .with_detail("deletedCount", deleted_keys.len().to_string())
                    .with_detail("failedObjectKey", object_key));
            }
            deleted_keys.push(object_key);
        }
        Ok(DeleteOssObjectsResult { deleted_keys })
    }

    pub async fn copy_object(&self, input: CopyOssObjectInput) -> AppResult<CopyOssObjectResult> {
        let target = self
            .load_target(validate_target_id(input.target_id)?)
            .await?;
        let source_key = normalize_object_key(input.source_key)?;
        let target_key = normalize_object_key(input.target_key)?;
        ensure_key_within_target(&target, &source_key)?;
        ensure_key_within_target(&target, &target_key)?;
        if source_key == target_key {
            return Err(AppError::validation(
                "sourceKey and targetKey must be different",
            ));
        }
        let account = self.load_account(target.account_id.clone()).await?;
        let credential = self.read_credential(account.credential_ref).await?;
        self.adapter
            .copy_object(OssObjectCopyRequest {
                target,
                credential,
                source_key: source_key.clone(),
                target_key: target_key.clone(),
                overwrite: input.overwrite,
            })
            .await?;
        Ok(CopyOssObjectResult {
            source_key,
            target_key,
        })
    }

    async fn ensure_account_exists(&self, account_id: String) -> AppResult<OssAccountRecord> {
        self.load_account(account_id).await
    }

    async fn load_account(&self, account_id: String) -> AppResult<OssAccountRecord> {
        let requested_id = validate_account_id(account_id)?;
        let query_id = requested_id.clone();
        self.database
            .read("get_oss_account", move |connection| {
                get_oss_account(connection, query_id)
            })
            .await?
            .ok_or_else(|| {
                AppError::new("OSS_ACCOUNT_NOT_FOUND", "OSS account was not found")
                    .with_detail("accountId", requested_id)
            })
    }

    async fn load_target(&self, target_id: String) -> AppResult<OssTargetRecord> {
        let requested_id = validate_target_id(target_id)?;
        let query_id = requested_id.clone();
        self.database
            .read("get_oss_target", move |connection| {
                get_oss_target(connection, query_id)
            })
            .await?
            .ok_or_else(|| {
                AppError::new("OSS_TARGET_NOT_FOUND", "OSS target was not found")
                    .with_detail("targetId", requested_id)
            })
    }

    async fn read_credential(&self, credential_ref: String) -> AppResult<OssCredential> {
        self.credential_service.read(credential_ref).await
    }

    async fn write_credential(
        &self,
        credential_ref: String,
        credential: OssCredential,
    ) -> AppResult<()> {
        self.credential_service
            .write(credential_ref, credential)
            .await
    }

    async fn delete_credential(&self, credential_ref: String) -> AppResult<()> {
        self.credential_service.delete(credential_ref).await
    }
}

fn validate_continuation_token(token: Option<String>) -> AppResult<Option<String>> {
    let Some(token) = token else {
        return Ok(None);
    };
    if token.is_empty()
        || token.len() > MAX_CONTINUATION_TOKEN_BYTES
        || token.chars().any(|character| character.is_control())
    {
        return Err(AppError::validation("continuationToken is invalid")
            .with_detail("field", "continuationToken"));
    }
    Ok(Some(token))
}

fn ensure_prefix_within_target(target: &OssTargetRecord, prefix: &str) -> AppResult<()> {
    if !target.prefix.is_empty() && !prefix.starts_with(&target.prefix) {
        return Err(AppError::new(
            "OSS_PREFIX_OUT_OF_SCOPE",
            "requested prefix is outside the configured OSS target",
        )
        .with_detail("targetId", target.id.clone()));
    }
    Ok(())
}

fn ensure_key_within_target(target: &OssTargetRecord, key: &str) -> AppResult<()> {
    if !target.prefix.is_empty() && !key.starts_with(&target.prefix) {
        return Err(AppError::new(
            "OSS_OBJECT_OUT_OF_SCOPE",
            "requested object is outside the configured OSS target",
        )
        .with_detail("targetId", target.id.clone()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ensure_key_within_target, ensure_prefix_within_target};
    use crate::domain::OssTargetRecord;

    fn target(prefix: &str) -> OssTargetRecord {
        OssTargetRecord {
            id: "550e8400-e29b-41d4-a716-446655440000".into(),
            account_id: "550e8400-e29b-41d4-a716-446655440001".into(),
            display_name: "Assets".into(),
            bucket: "example-bucket".into(),
            region: "cn-hangzhou".into(),
            endpoint: "https://oss-cn-hangzhou.aliyuncs.com".into(),
            prefix: prefix.into(),
            is_default: true,
            sort_order: 0,
            created_at: "2026-07-12T00:00:00Z".into(),
            updated_at: "2026-07-12T00:00:00Z".into(),
        }
    }

    #[test]
    fn target_prefix_limits_object_and_listing_scope() {
        let target = target("assets/");
        assert!(ensure_prefix_within_target(&target, "assets/images/").is_ok());
        assert!(ensure_key_within_target(&target, "assets/readme.txt").is_ok());
        assert!(ensure_prefix_within_target(&target, "private/").is_err());
        assert!(ensure_key_within_target(&target, "private.txt").is_err());
    }
}
