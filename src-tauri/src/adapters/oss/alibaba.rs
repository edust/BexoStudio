use std::{
    future::Future,
    pin::Pin,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use chrono::{DateTime, Utc};
use percent_encoding::{percent_encode, AsciiSet, NON_ALPHANUMERIC};
use reqwest::{
    header::{HeaderMap, HeaderValue, CONTENT_LENGTH, CONTENT_TYPE, RANGE},
    Client, Method, Response, StatusCode, Url,
};
use tokio::time::sleep;

use crate::{
    domain::{
        OssCredential, OssObjectMetadata, OssObjectPage, OssTargetRecord, OSS_MAX_TRANSFER_RETRIES,
    },
    error::{AppError, AppResult},
};

use super::{
    parse_error_response, parse_initiate_multipart_upload_response, parse_list_objects_response,
    parse_object_metadata, ObjectStorageAdapter, OssMultipartAbortRequest,
    OssMultipartCompleteRequest, OssMultipartInitiateRequest, OssMultipartUpload,
    OssMultipartUploadPartRequest, OssObjectCopyRequest, OssObjectDeleteRequest, OssObjectDownload,
    OssObjectGetRequest, OssObjectHeadRequest, OssObjectListRequest, OssObjectPutRequest,
    OssUploadedPart, OssV4Signer,
};

const OSS_RESPONSE_BODY_LIMIT: usize = 8 * 1024 * 1024;
const OSS_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const OSS_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const OSS_RETRY_BASE_DELAY: Duration = Duration::from_millis(250);
const OSS_COPY_SOURCE_ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~')
    .remove(b'/');

#[derive(Debug, Clone)]
pub struct AlibabaOssAdapter {
    client: Client,
}

impl AlibabaOssAdapter {
    pub fn new() -> AppResult<Self> {
        let client = Client::builder()
            .connect_timeout(OSS_CONNECT_TIMEOUT)
            .timeout(OSS_REQUEST_TIMEOUT)
            .build()
            .map_err(|error| {
                AppError::new(
                    "OSS_CLIENT_INIT_FAILED",
                    "failed to initialize OSS HTTP client",
                )
                .with_detail("reason", error.to_string())
            })?;
        Ok(Self { client })
    }

    pub fn presign_get_url(
        &self,
        target: &OssTargetRecord,
        credential: &OssCredential,
        object_key: &str,
        expires_in_seconds: u64,
        now: DateTime<Utc>,
    ) -> AppResult<String> {
        let url = build_bucket_url(&target.endpoint, &target.bucket, Some(object_key))?;
        let signer = OssV4Signer::new(
            credential.access_key_id.clone(),
            credential.access_key_secret.clone(),
            target.region.clone(),
        )?;
        Ok(signer
            .presign_get(&url, &target.bucket, object_key, expires_in_seconds, now)?
            .to_string())
    }

    async fn list_objects_impl(&self, request: OssObjectListRequest) -> AppResult<OssObjectPage> {
        let mut url = build_bucket_url(&request.target.endpoint, &request.target.bucket, None)?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("list-type", "2");
            query.append_pair("delimiter", "/");
            query.append_pair("encoding-type", "url");
            query.append_pair("max-keys", &request.page_size.to_string());
            if !request.prefix.is_empty() {
                query.append_pair("prefix", &request.prefix);
            }
            if let Some(token) = request.continuation_token.as_deref() {
                query.append_pair("continuation-token", token);
            }
        }
        let response = self
            .execute_request(
                Method::GET,
                url,
                Some(&request.target.bucket),
                None,
                &request.credential,
                &request.target.region,
                HeaderMap::new(),
                &[],
                None,
                true,
            )
            .await?;
        let body = read_bounded_body(response).await?;
        parse_list_objects_response(&body)
    }

    async fn head_object_impl(
        &self,
        request: OssObjectHeadRequest,
    ) -> AppResult<OssObjectMetadata> {
        let url = build_bucket_url(
            &request.target.endpoint,
            &request.target.bucket,
            Some(&request.object_key),
        )?;
        let response = self
            .execute_request(
                Method::HEAD,
                url,
                Some(&request.target.bucket),
                Some(&request.object_key),
                &request.credential,
                &request.target.region,
                HeaderMap::new(),
                &[],
                None,
                true,
            )
            .await?;
        Ok(parse_object_metadata(
            response.headers(),
            request.object_key,
        ))
    }

    async fn put_object_impl(&self, request: OssObjectPutRequest) -> AppResult<()> {
        let url = build_bucket_url(
            &request.target.endpoint,
            &request.target.bucket,
            Some(&request.object_key),
        )?;
        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_LENGTH,
            header_value(request.body.len().to_string(), "content length")?,
        );
        headers.insert(
            CONTENT_TYPE,
            header_value(
                request
                    .content_type
                    .as_deref()
                    .unwrap_or("application/octet-stream")
                    .to_string(),
                "content type",
            )?,
        );
        if !request.overwrite {
            headers.insert("x-oss-forbid-overwrite", HeaderValue::from_static("true"));
        }
        let response = self
            .execute_request(
                Method::PUT,
                url,
                Some(&request.target.bucket),
                Some(&request.object_key),
                &request.credential,
                &request.target.region,
                headers,
                &["content-length"],
                Some(request.body),
                false,
            )
            .await?;
        drain_success_body(response).await
    }

    async fn delete_object_impl(&self, request: OssObjectDeleteRequest) -> AppResult<()> {
        let url = build_bucket_url(
            &request.target.endpoint,
            &request.target.bucket,
            Some(&request.object_key),
        )?;
        let response = self
            .execute_request(
                Method::DELETE,
                url,
                Some(&request.target.bucket),
                Some(&request.object_key),
                &request.credential,
                &request.target.region,
                HeaderMap::new(),
                &[],
                None,
                true,
            )
            .await?;
        drain_success_body(response).await
    }

    async fn copy_object_impl(&self, request: OssObjectCopyRequest) -> AppResult<()> {
        let url = build_bucket_url(
            &request.target.endpoint,
            &request.target.bucket,
            Some(&request.target_key),
        )?;
        let mut headers = HeaderMap::new();
        let source = format!(
            "/{}/{}",
            request.target.bucket,
            percent_encode(request.source_key.as_bytes(), OSS_COPY_SOURCE_ENCODE_SET)
        );
        headers.insert("x-oss-copy-source", header_value(source, "copy source")?);
        if !request.overwrite {
            headers.insert("x-oss-forbid-overwrite", HeaderValue::from_static("true"));
        }
        let response = self
            .execute_request(
                Method::PUT,
                url,
                Some(&request.target.bucket),
                Some(&request.target_key),
                &request.credential,
                &request.target.region,
                headers,
                &[],
                None,
                false,
            )
            .await?;
        drain_success_body(response).await
    }

    async fn get_object_impl(&self, request: OssObjectGetRequest) -> AppResult<OssObjectDownload> {
        let url = build_bucket_url(
            &request.target.endpoint,
            &request.target.bucket,
            Some(&request.object_key),
        )?;
        let mut headers = HeaderMap::new();
        let range_requested = request.range_start.is_some() || request.range_end.is_some();
        if let Some(start) = request.range_start {
            let range = match request.range_end {
                Some(end) if end < start => {
                    return Err(AppError::validation("download range is invalid"));
                }
                Some(end) => format!("bytes={start}-{end}"),
                None => format!("bytes={start}-"),
            };
            headers.insert(RANGE, header_value(range, "download range")?);
        } else if request.range_end.is_some() {
            return Err(AppError::validation("download range must include a start"));
        }
        let response = self
            .execute_request(
                Method::GET,
                url,
                Some(&request.target.bucket),
                Some(&request.object_key),
                &request.credential,
                &request.target.region,
                headers,
                &["range"],
                None,
                true,
            )
            .await?;
        if range_requested && response.status() != StatusCode::PARTIAL_CONTENT {
            return Err(AppError::new(
                "OSS_RANGE_UNSUPPORTED",
                "OSS did not return a partial-content response for the requested range",
            )
            .with_detail("status", response.status().as_u16().to_string()));
        }
        Ok(OssObjectDownload { response })
    }

    async fn initiate_multipart_upload_impl(
        &self,
        request: OssMultipartInitiateRequest,
    ) -> AppResult<OssMultipartUpload> {
        let mut url = build_bucket_url(
            &request.target.endpoint,
            &request.target.bucket,
            Some(&request.object_key),
        )?;
        url.set_query(Some("uploads"));
        let mut headers = HeaderMap::new();
        if let Some(content_type) = request.content_type.as_deref() {
            headers.insert(
                CONTENT_TYPE,
                header_value(content_type.to_string(), "content type")?,
            );
        }
        headers.insert(CONTENT_LENGTH, HeaderValue::from_static("0"));
        if !request.overwrite {
            headers.insert("x-oss-forbid-overwrite", HeaderValue::from_static("true"));
        }
        let response = self
            .execute_request(
                Method::POST,
                url,
                Some(&request.target.bucket),
                Some(&request.object_key),
                &request.credential,
                &request.target.region,
                headers,
                &["content-length"],
                None,
                false,
            )
            .await?;
        let body = read_bounded_body(response).await?;
        Ok(OssMultipartUpload {
            upload_id: parse_initiate_multipart_upload_response(&body)?,
        })
    }

    async fn upload_multipart_part_impl(
        &self,
        request: OssMultipartUploadPartRequest,
    ) -> AppResult<OssUploadedPart> {
        let mut url = build_bucket_url(
            &request.target.endpoint,
            &request.target.bucket,
            Some(&request.object_key),
        )?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("partNumber", &request.part_number.to_string());
            query.append_pair("uploadId", &request.upload_id);
        }
        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_LENGTH,
            header_value(request.body.len().to_string(), "content length")?,
        );
        let response = self
            .execute_request(
                Method::PUT,
                url,
                Some(&request.target.bucket),
                Some(&request.object_key),
                &request.credential,
                &request.target.region,
                headers,
                &["content-length"],
                Some(request.body),
                true,
            )
            .await?;
        let etag = response
            .headers()
            .get("etag")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string)
            .ok_or_else(|| {
                AppError::new(
                    "OSS_RESPONSE_INVALID",
                    "OSS upload part response did not contain an ETag",
                )
            })?;
        Ok(OssUploadedPart {
            part_number: request.part_number,
            etag,
        })
    }

    async fn complete_multipart_upload_impl(
        &self,
        request: OssMultipartCompleteRequest,
    ) -> AppResult<()> {
        if request.parts.is_empty() {
            return Err(AppError::validation(
                "multipart upload requires at least one part",
            ));
        }
        let mut parts = request.parts;
        parts.sort_by_key(|part| part.part_number);
        let body = build_complete_multipart_xml(&parts)?;
        let mut url = build_bucket_url(
            &request.target.endpoint,
            &request.target.bucket,
            Some(&request.object_key),
        )?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("uploadId", &request.upload_id);
        }
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/xml"));
        headers.insert(
            CONTENT_LENGTH,
            header_value(body.len().to_string(), "content length")?,
        );
        if !request.overwrite {
            headers.insert("x-oss-forbid-overwrite", HeaderValue::from_static("true"));
        }
        let response = self
            .execute_request(
                Method::POST,
                url,
                Some(&request.target.bucket),
                Some(&request.object_key),
                &request.credential,
                &request.target.region,
                headers,
                &["content-length"],
                Some(body),
                false,
            )
            .await?;
        drain_success_body(response).await
    }

    async fn abort_multipart_upload_impl(
        &self,
        request: OssMultipartAbortRequest,
    ) -> AppResult<()> {
        let mut url = build_bucket_url(
            &request.target.endpoint,
            &request.target.bucket,
            Some(&request.object_key),
        )?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("uploadId", &request.upload_id);
        }
        let response = self
            .execute_request(
                Method::DELETE,
                url,
                Some(&request.target.bucket),
                Some(&request.object_key),
                &request.credential,
                &request.target.region,
                HeaderMap::new(),
                &[],
                None,
                true,
            )
            .await?;
        drain_success_body(response).await
    }

    async fn execute_request(
        &self,
        method: Method,
        url: Url,
        bucket: Option<&str>,
        object_key: Option<&str>,
        credential: &OssCredential,
        region: &str,
        base_headers: HeaderMap,
        additional_headers: &[&str],
        body: Option<Vec<u8>>,
        retryable_operation: bool,
    ) -> AppResult<Response> {
        for attempt in 0..OSS_MAX_TRANSFER_RETRIES {
            let mut headers = base_headers.clone();
            let signer = OssV4Signer::new(
                credential.access_key_id.clone(),
                credential.access_key_secret.clone(),
                region.to_string(),
            )?;
            signer.sign(
                &method,
                &url,
                bucket,
                object_key,
                &mut headers,
                additional_headers,
                Utc::now(),
            )?;

            let mut builder = self
                .client
                .request(method.clone(), url.clone())
                .headers(headers);
            if let Some(body) = body.as_ref() {
                builder = builder.body(body.clone());
            }
            let response = builder.send().await.map_err(map_network_error);
            match response {
                Ok(response) if response.status().is_success() => return Ok(response),
                Ok(response) => {
                    let status = response.status();
                    let body = match read_bounded_body(response).await {
                        Ok(body) => body,
                        Err(error) => {
                            if retryable_operation
                                && error.retryable.unwrap_or(false)
                                && attempt + 1 < OSS_MAX_TRANSFER_RETRIES
                            {
                                sleep(retry_delay(attempt)).await;
                                continue;
                            }
                            return Err(error);
                        }
                    };
                    let error = parse_error_response(status, &body);
                    if !retryable_operation
                        || !error.retryable.unwrap_or(false)
                        || attempt + 1 >= OSS_MAX_TRANSFER_RETRIES
                    {
                        return Err(error);
                    }
                    sleep(retry_delay(attempt)).await;
                }
                Err(error) => {
                    if !retryable_operation
                        || !error.retryable.unwrap_or(false)
                        || attempt + 1 >= OSS_MAX_TRANSFER_RETRIES
                    {
                        return Err(error);
                    }
                    sleep(retry_delay(attempt)).await;
                }
            }
        }
        Err(AppError::new(
            "OSS_NETWORK_ERROR",
            "OSS request exhausted its retry budget",
        ))
    }
}

impl ObjectStorageAdapter for AlibabaOssAdapter {
    fn list_objects(
        &self,
        request: OssObjectListRequest,
    ) -> Pin<Box<dyn Future<Output = AppResult<OssObjectPage>> + Send + '_>> {
        Box::pin(self.list_objects_impl(request))
    }

    fn head_object(
        &self,
        request: OssObjectHeadRequest,
    ) -> Pin<Box<dyn Future<Output = AppResult<OssObjectMetadata>> + Send + '_>> {
        Box::pin(self.head_object_impl(request))
    }

    fn put_object(
        &self,
        request: OssObjectPutRequest,
    ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + '_>> {
        Box::pin(self.put_object_impl(request))
    }

    fn delete_object(
        &self,
        request: OssObjectDeleteRequest,
    ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + '_>> {
        Box::pin(self.delete_object_impl(request))
    }

    fn copy_object(
        &self,
        request: OssObjectCopyRequest,
    ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + '_>> {
        Box::pin(self.copy_object_impl(request))
    }

    fn get_object(
        &self,
        request: OssObjectGetRequest,
    ) -> Pin<Box<dyn Future<Output = AppResult<OssObjectDownload>> + Send + '_>> {
        Box::pin(self.get_object_impl(request))
    }

    fn initiate_multipart_upload(
        &self,
        request: OssMultipartInitiateRequest,
    ) -> Pin<Box<dyn Future<Output = AppResult<OssMultipartUpload>> + Send + '_>> {
        Box::pin(self.initiate_multipart_upload_impl(request))
    }

    fn upload_multipart_part(
        &self,
        request: OssMultipartUploadPartRequest,
    ) -> Pin<Box<dyn Future<Output = AppResult<OssUploadedPart>> + Send + '_>> {
        Box::pin(self.upload_multipart_part_impl(request))
    }

    fn complete_multipart_upload(
        &self,
        request: OssMultipartCompleteRequest,
    ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + '_>> {
        Box::pin(self.complete_multipart_upload_impl(request))
    }

    fn abort_multipart_upload(
        &self,
        request: OssMultipartAbortRequest,
    ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + '_>> {
        Box::pin(self.abort_multipart_upload_impl(request))
    }
}

pub fn build_service_url(endpoint: &str) -> AppResult<Url> {
    let mut url = reqwest::Url::parse(endpoint).map_err(|error| {
        AppError::validation("endpoint is invalid").with_detail("reason", error.to_string())
    })?;
    url.set_path("/");
    url.set_query(None);
    Ok(url)
}

pub fn build_bucket_url(endpoint: &str, bucket: &str, object_key: Option<&str>) -> AppResult<Url> {
    let mut url = build_service_url(endpoint)?;
    let endpoint_host = url
        .host_str()
        .ok_or_else(|| AppError::validation("endpoint host is missing"))?
        .to_string();
    let bucket_host = if endpoint_host.strip_prefix(&format!("{bucket}.")).is_some() {
        endpoint_host
    } else {
        format!("{bucket}.{endpoint_host}")
    };
    url.set_host(Some(&bucket_host)).map_err(|_| {
        AppError::validation("endpoint cannot be combined with bucket host")
            .with_detail("bucket", bucket)
    })?;
    if let Some(object_key) = object_key {
        url.set_path(&format!("/{object_key}"));
    }
    Ok(url)
}

fn build_complete_multipart_xml(parts: &[super::OssMultipartCompletePart]) -> AppResult<Vec<u8>> {
    let mut body = String::from("<CompleteMultipartUpload>");
    for part in parts {
        if part.part_number == 0 || part.part_number > 10_000 || part.etag.is_empty() {
            return Err(AppError::validation("multipart part metadata is invalid"));
        }
        body.push_str("<Part><PartNumber>");
        body.push_str(&part.part_number.to_string());
        body.push_str("</PartNumber><ETag>");
        body.push_str(&xml_escape(&part.etag));
        body.push_str("</ETag></Part>");
    }
    body.push_str("</CompleteMultipartUpload>");
    Ok(body.into_bytes())
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn header_value(value: String, field: &str) -> AppResult<HeaderValue> {
    HeaderValue::from_str(&value).map_err(|error| {
        AppError::validation(format!("{field} header is invalid"))
            .with_detail("reason", error.to_string())
    })
}

async fn drain_success_body(response: Response) -> AppResult<()> {
    read_bounded_body(response).await.map(|_| ())
}

async fn read_bounded_body(response: Response) -> AppResult<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > OSS_RESPONSE_BODY_LIMIT as u64)
    {
        return Err(AppError::new(
            "OSS_RESPONSE_INVALID",
            "OSS response body exceeded the safety limit",
        ));
    }
    let body = response.bytes().await.map_err(map_network_error)?;
    if body.len() > OSS_RESPONSE_BODY_LIMIT {
        return Err(AppError::new(
            "OSS_RESPONSE_INVALID",
            "OSS response body exceeded the safety limit",
        ));
    }
    Ok(body.to_vec())
}

fn map_network_error(error: reqwest::Error) -> AppError {
    let retryable = error.is_timeout() || error.is_connect();
    AppError::new(
        if error.is_timeout() {
            "OSS_NETWORK_TIMEOUT"
        } else {
            "OSS_NETWORK_ERROR"
        },
        "OSS network request failed",
    )
    .with_detail("reason", error.to_string())
    .retryable(retryable)
}

fn retry_delay(attempt: u32) -> Duration {
    let exponent = 1_u64 << attempt.min(4);
    let jitter = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::from(duration.subsec_millis()) % 100)
        .unwrap_or(0);
    OSS_RETRY_BASE_DELAY
        .saturating_mul(exponent as u32)
        .saturating_add(Duration::from_millis(jitter))
}

#[cfg(test)]
mod tests {
    use super::{build_bucket_url, build_complete_multipart_xml, build_service_url};

    #[test]
    fn builds_virtual_hosted_bucket_urls_without_leaking_query_state() {
        let service = build_service_url("https://oss-cn-hangzhou.aliyuncs.com").unwrap();
        assert_eq!(service.as_str(), "https://oss-cn-hangzhou.aliyuncs.com/");

        let object = build_bucket_url(
            "https://oss-cn-hangzhou.aliyuncs.com",
            "example-bucket",
            Some("folder/测试.txt"),
        )
        .unwrap();
        assert_eq!(
            object.host_str(),
            Some("example-bucket.oss-cn-hangzhou.aliyuncs.com")
        );
        assert!(object.path().contains("folder"));
        assert!(object.query().is_none());
    }

    #[test]
    fn keeps_already_bucket_qualified_custom_endpoints_stable() {
        let object = build_bucket_url(
            "https://example-bucket.custom-oss.example.com",
            "example-bucket",
            None,
        )
        .unwrap();
        assert_eq!(
            object.host_str(),
            Some("example-bucket.custom-oss.example.com")
        );
    }

    #[test]
    fn builds_sorted_complete_multipart_xml() {
        let body = build_complete_multipart_xml(&[
            super::super::OssMultipartCompletePart {
                part_number: 2,
                etag: "etag-2".into(),
            },
            super::super::OssMultipartCompletePart {
                part_number: 1,
                etag: "\"etag-1\"".into(),
            },
        ])
        .unwrap();
        let body = String::from_utf8(body).unwrap();
        assert!(
            body.find("<PartNumber>1</PartNumber>").unwrap()
                < body.find("<PartNumber>2</PartNumber>").unwrap()
        );
        assert!(body.contains("&quot;etag-1&quot;"));
    }
}
