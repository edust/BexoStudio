use percent_encoding::percent_decode_str;
use quick_xml::{events::Event, Reader};

use crate::{
    domain::{OssObjectEntry, OssObjectMetadata, OssObjectPage},
    error::{AppError, AppResult},
};

pub fn parse_list_objects_response(body: &[u8]) -> AppResult<OssObjectPage> {
    let mut reader = Reader::from_reader(body);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut current_element = String::new();
    let mut current_object: Option<OssObjectEntry> = None;
    let mut objects = Vec::new();
    let mut common_prefixes = Vec::new();
    let mut next_continuation_token = None;
    let mut is_truncated = false;
    let mut in_common_prefix = false;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                current_element = String::from_utf8_lossy(event.name().as_ref()).to_string();
                match current_element.as_str() {
                    "Contents" => {
                        current_object = Some(OssObjectEntry {
                            key: String::new(),
                            kind: "object".to_string(),
                            size: None,
                            etag: None,
                            last_modified: None,
                            storage_class: None,
                        });
                    }
                    "CommonPrefixes" => in_common_prefix = true,
                    _ => {}
                }
            }
            Ok(Event::Text(text)) => {
                let value = text
                    .decode()
                    .map_err(|error| xml_error("failed to decode OSS XML text", error.to_string()))?
                    .into_owned();
                match current_element.as_str() {
                    "Key" => {
                        if let Some(object) = current_object.as_mut() {
                            object.key = decode_oss_value(&value);
                        } else if in_common_prefix {
                            common_prefixes.push(decode_oss_value(&value));
                        }
                    }
                    "Prefix" if in_common_prefix => common_prefixes.push(decode_oss_value(&value)),
                    "Size" => {
                        if let Some(object) = current_object.as_mut() {
                            object.size = Some(parse_u64(&value, "Size")?);
                        }
                    }
                    "ETag" => {
                        if let Some(object) = current_object.as_mut() {
                            object.etag = Some(strip_quotes(&value));
                        }
                    }
                    "LastModified" => {
                        if let Some(object) = current_object.as_mut() {
                            object.last_modified = Some(value);
                        }
                    }
                    "StorageClass" => {
                        if let Some(object) = current_object.as_mut() {
                            object.storage_class = Some(value);
                        }
                    }
                    "NextContinuationToken" => {
                        next_continuation_token = Some(decode_oss_value(&value))
                    }
                    "IsTruncated" => is_truncated = value.eq_ignore_ascii_case("true"),
                    _ => {}
                }
            }
            Ok(Event::End(event)) => {
                let element = String::from_utf8_lossy(event.name().as_ref()).to_string();
                if element == "Contents" {
                    if let Some(object) = current_object.take() {
                        objects.push(object);
                    }
                } else if element == "CommonPrefixes" {
                    in_common_prefix = false;
                }
                current_element.clear();
            }
            Ok(Event::Eof) => break,
            Err(error) => {
                return Err(xml_error(
                    "failed to parse OSS object list response",
                    error.to_string(),
                ));
            }
            _ => {}
        }
        buffer.clear();
    }

    Ok(OssObjectPage {
        objects,
        common_prefixes,
        next_continuation_token,
        is_truncated,
    })
}

pub fn parse_initiate_multipart_upload_response(body: &[u8]) -> AppResult<String> {
    let mut reader = Reader::from_reader(body);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut current_element = String::new();
    let mut upload_id = None;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                current_element = String::from_utf8_lossy(event.name().as_ref()).to_string();
            }
            Ok(Event::Text(text)) => {
                let value = text
                    .decode()
                    .map_err(|error| {
                        xml_error("failed to decode OSS multipart response", error.to_string())
                    })?
                    .into_owned();
                if current_element == "UploadId" {
                    upload_id = Some(value);
                }
            }
            Ok(Event::End(_)) => current_element.clear(),
            Ok(Event::Eof) => break,
            Err(error) => {
                return Err(xml_error(
                    "failed to parse OSS multipart response",
                    error.to_string(),
                ));
            }
            _ => {}
        }
        buffer.clear();
    }

    upload_id.filter(|value| !value.is_empty()).ok_or_else(|| {
        AppError::new(
            "OSS_RESPONSE_INVALID",
            "OSS multipart response did not contain an upload ID",
        )
    })
}

pub fn parse_object_metadata(
    headers: &reqwest::header::HeaderMap,
    object_key: String,
) -> OssObjectMetadata {
    OssObjectMetadata {
        key: object_key,
        size: headers
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok()),
        etag: headers
            .get("etag")
            .and_then(|value| value.to_str().ok())
            .map(strip_quotes),
        content_type: headers
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
        last_modified: headers
            .get(reqwest::header::LAST_MODIFIED)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
        storage_class: headers
            .get("x-oss-storage-class")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
        version_id: headers
            .get("x-oss-version-id")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
    }
}

pub fn parse_error_response(status: reqwest::StatusCode, body: &[u8]) -> AppError {
    let mut reader = Reader::from_reader(body);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut current_element = String::new();
    let mut code = None;
    let mut message = None;
    let mut request_id = None;
    let mut endpoint = None;
    let mut host_id = None;
    let mut bucket = None;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                current_element = String::from_utf8_lossy(event.name().as_ref()).to_string();
            }
            Ok(Event::Text(text)) => {
                if let Ok(value) = text.decode() {
                    match current_element.as_str() {
                        "Code" => code = Some(value.into_owned()),
                        "Message" => message = Some(value.into_owned()),
                        "RequestId" => request_id = Some(value.into_owned()),
                        "Endpoint" => endpoint = Some(value.into_owned()),
                        "HostId" => host_id = Some(value.into_owned()),
                        "Bucket" => bucket = Some(value.into_owned()),
                        _ => {}
                    }
                }
            }
            Ok(Event::End(_)) => current_element.clear(),
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buffer.clear();
    }

    let status_code = status.as_u16().to_string();
    let oss_code = code.unwrap_or_else(|| format!("HTTP_{status_code}"));
    let app_code = match oss_code.as_str() {
        "FileAlreadyExists" => "OSS_UPLOAD_CONFLICT",
        "NoSuchKey" => "OSS_OBJECT_NOT_FOUND",
        "NoSuchBucket" => "OSS_BUCKET_NOT_FOUND",
        "IncorrectEndpoint" | "PermanentRedirect" | "AuthorizationHeaderMalformed" => {
            "OSS_ENDPOINT_MISMATCH"
        }
        "InvalidAccessKeyId" | "SignatureDoesNotMatch" | "SecurityTokenInvalid" => {
            "OSS_AUTH_FAILED"
        }
        _ => match status {
            reqwest::StatusCode::UNAUTHORIZED => "OSS_AUTH_FAILED",
            reqwest::StatusCode::FORBIDDEN => "OSS_ACCESS_DENIED",
            reqwest::StatusCode::NOT_FOUND => "OSS_OBJECT_OR_BUCKET_NOT_FOUND",
            reqwest::StatusCode::PRECONDITION_FAILED => "OSS_UPLOAD_CONFLICT",
            reqwest::StatusCode::REQUEST_TIMEOUT
            | reqwest::StatusCode::TOO_MANY_REQUESTS
            | reqwest::StatusCode::BAD_GATEWAY
            | reqwest::StatusCode::SERVICE_UNAVAILABLE
            | reqwest::StatusCode::GATEWAY_TIMEOUT => "OSS_REMOTE_RETRYABLE",
            _ => "OSS_REMOTE_ERROR",
        },
    };
    let message = match (oss_code.as_str(), endpoint.as_deref()) {
        (
            "IncorrectEndpoint" | "PermanentRedirect" | "AuthorizationHeaderMalformed",
            Some(endpoint),
        ) => {
            format!("Bucket 需要使用 Endpoint：{endpoint}")
        }
        (_, _) => message.unwrap_or_else(|| "Alibaba OSS request failed".to_string()),
    };
    let mut error = AppError::new(app_code, message)
        .with_detail("status", status_code)
        .with_detail("ossCode", oss_code);
    if let Some(request_id) = request_id {
        error = error.with_detail("requestId", request_id);
    }
    if let Some(endpoint) = endpoint {
        error = error.with_detail("endpoint", endpoint);
    }
    if let Some(host_id) = host_id {
        error = error.with_detail("hostId", host_id);
    }
    if let Some(bucket) = bucket {
        error = error.with_detail("bucket", bucket);
    }
    error.retryable(matches!(
        status,
        reqwest::StatusCode::REQUEST_TIMEOUT
            | reqwest::StatusCode::TOO_MANY_REQUESTS
            | reqwest::StatusCode::BAD_GATEWAY
            | reqwest::StatusCode::SERVICE_UNAVAILABLE
            | reqwest::StatusCode::GATEWAY_TIMEOUT
    ))
}

fn strip_quotes(value: &str) -> String {
    value.trim_matches('"').to_string()
}

fn decode_oss_value(value: &str) -> String {
    percent_decode_str(value).decode_utf8_lossy().into_owned()
}

fn parse_u64(value: &str, field: &str) -> AppResult<u64> {
    value.parse::<u64>().map_err(|error| {
        AppError::new(
            "OSS_RESPONSE_INVALID",
            "OSS response contained an invalid number",
        )
        .with_detail("field", field)
        .with_detail("reason", error.to_string())
    })
}

fn xml_error(message: &str, reason: String) -> AppError {
    AppError::new("OSS_RESPONSE_INVALID", message).with_detail("reason", reason)
}

#[cfg(test)]
mod tests {
    use super::{parse_error_response, parse_list_objects_response};

    #[test]
    fn parses_object_page_and_common_prefixes() {
        let body = br#"<ListBucketResult>
          <IsTruncated>true</IsTruncated>
          <NextContinuationToken>next%20token</NextContinuationToken>
          <Contents>
            <Key>docs/readme.txt</Key>
            <LastModified>2026-07-12T00:00:00.000Z</LastModified>
            <ETag>&quot;etag-1&quot;</ETag>
            <Size>42</Size>
            <StorageClass>Standard</StorageClass>
          </Contents>
          <CommonPrefixes><Prefix>docs/images/</Prefix></CommonPrefixes>
        </ListBucketResult>"#;
        let page = parse_list_objects_response(body).unwrap();
        assert!(page.is_truncated);
        assert_eq!(page.next_continuation_token.as_deref(), Some("next token"));
        assert_eq!(page.objects[0].key, "docs/readme.txt");
        assert_eq!(page.objects[0].etag.as_deref(), Some("etag-1"));
        assert_eq!(page.common_prefixes, vec!["docs/images/"]);
    }

    #[test]
    fn maps_oss_errors_without_exposing_request_body() {
        let error = parse_error_response(
            reqwest::StatusCode::FORBIDDEN,
            br#"<Error><Code>AccessDenied</Code><Message>denied</Message><RequestId>req-1</RequestId></Error>"#,
        );
        assert_eq!(error.code, "OSS_ACCESS_DENIED");
        assert_eq!(
            error.details.as_ref().unwrap().get("ossCode").unwrap(),
            "AccessDenied"
        );
        assert!(!error.message.contains("Authorization"));
    }

    #[test]
    fn maps_endpoint_errors_with_server_endpoint_hint() {
        let error = parse_error_response(
            reqwest::StatusCode::BAD_REQUEST,
            br#"<Error><Code>IncorrectEndpoint</Code><Message>wrong endpoint</Message><Endpoint>oss-cn-shanghai.aliyuncs.com</Endpoint><HostId>season-x-2020.oss-cn-shanghai.aliyuncs.com</HostId><Bucket>season-x-2020</Bucket></Error>"#,
        );
        assert_eq!(error.code, "OSS_ENDPOINT_MISMATCH");
        assert_eq!(
            error.message,
            "Bucket 需要使用 Endpoint：oss-cn-shanghai.aliyuncs.com"
        );
        assert_eq!(
            error.details.as_ref().unwrap().get("endpoint").unwrap(),
            "oss-cn-shanghai.aliyuncs.com"
        );
        assert_eq!(
            error.details.as_ref().unwrap().get("bucket").unwrap(),
            "season-x-2020"
        );
    }

    #[test]
    fn maps_forbid_overwrite_conflicts_to_a_stable_app_code() {
        let error = parse_error_response(
            reqwest::StatusCode::PRECONDITION_FAILED,
            br#"<Error><Code>PreconditionFailed</Code><Message>object already exists</Message></Error>"#,
        );
        assert_eq!(error.code, "OSS_UPLOAD_CONFLICT");
    }
}
