use chrono::{DateTime, Utc};
use hmac::{Hmac, KeyInit, Mac};
use percent_encoding::{percent_decode_str, percent_encode, AsciiSet, NON_ALPHANUMERIC};
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue, HOST},
    Method, Url,
};
use sha2::{Digest, Sha256};

use crate::{
    domain::OSS_MAX_DOWNLOAD_URL_EXPIRES_SECONDS,
    error::{AppError, AppResult},
};

const ALGORITHM: &str = "OSS4-HMAC-SHA256";
const SERVICE: &str = "oss";
const REQUEST_TYPE: &str = "aliyun_v4_request";
const UNSIGNED_PAYLOAD: &str = "UNSIGNED-PAYLOAD";
const OSS_URI_ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~')
    .remove(b'/');
const OSS_QUERY_ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone)]
pub struct OssV4Signer {
    access_key_id: String,
    access_key_secret: String,
    region: String,
}

impl OssV4Signer {
    pub fn new(
        access_key_id: String,
        access_key_secret: String,
        region: String,
    ) -> AppResult<Self> {
        if access_key_id.is_empty() || access_key_secret.is_empty() || region.is_empty() {
            return Err(AppError::validation(
                "OSS V4 signer credentials are incomplete",
            ));
        }
        if region.starts_with("oss-") {
            return Err(AppError::validation(
                "OSS V4 region must be a region ID such as cn-shanghai, not an endpoint name",
            )
            .with_detail("region", region));
        }
        Ok(Self {
            access_key_id,
            access_key_secret,
            region,
        })
    }

    pub fn sign(
        &self,
        method: &Method,
        url: &Url,
        bucket: Option<&str>,
        object_key: Option<&str>,
        headers: &mut HeaderMap,
        additional_headers: &[&str],
        now: DateTime<Utc>,
    ) -> AppResult<()> {
        let datetime = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date = now.format("%Y%m%d").to_string();
        headers.insert(
            HeaderName::from_static("x-oss-date"),
            HeaderValue::from_str(&datetime).map_err(|error| {
                AppError::new("OSS_SIGNING_FAILED", "failed to build OSS date header")
                    .with_detail("reason", error.to_string())
            })?,
        );
        headers.insert(
            HeaderName::from_static("x-oss-content-sha256"),
            HeaderValue::from_static(UNSIGNED_PAYLOAD),
        );

        let signed_headers = signed_header_names(headers, additional_headers);
        let canonical_request =
            build_canonical_request(method, url, bucket, object_key, headers, &signed_headers);
        let scope = format!("{date}/{}/{SERVICE}/{REQUEST_TYPE}", self.region);
        let string_to_sign = format!(
            "{ALGORITHM}\n{datetime}\n{scope}\n{}",
            hex_lower(&Sha256::digest(canonical_request.as_bytes()))
        );
        let signature = calculate_signature(
            &self.access_key_secret,
            &date,
            &self.region,
            &string_to_sign,
        )?;

        let mut authorization = format!("{ALGORITHM} Credential={}/{scope}", self.access_key_id);
        if !signed_headers.additional.is_empty() {
            authorization.push_str(",AdditionalHeaders=");
            authorization.push_str(&signed_headers.additional.join(";"));
        }
        authorization.push_str(",Signature=");
        authorization.push_str(&signature);
        headers.insert(
            HeaderName::from_static("authorization"),
            HeaderValue::from_str(&authorization).map_err(|error| {
                AppError::new(
                    "OSS_SIGNING_FAILED",
                    "failed to build OSS authorization header",
                )
                .with_detail("reason", error.to_string())
            })?,
        );
        Ok(())
    }

    pub fn presign_get(
        &self,
        url: &Url,
        bucket: &str,
        object_key: &str,
        expires_in_seconds: u64,
        now: DateTime<Utc>,
    ) -> AppResult<Url> {
        if expires_in_seconds == 0 || expires_in_seconds > OSS_MAX_DOWNLOAD_URL_EXPIRES_SECONDS {
            return Err(
                AppError::validation("OSS presigned URL expiration is out of range")
                    .with_detail("expiresInSeconds", expires_in_seconds.to_string()),
            );
        }
        let host = url
            .host_str()
            .ok_or_else(|| AppError::validation("OSS presigned URL host is missing"))?;
        let host = match url.port() {
            Some(port) => format!("{host}:{port}"),
            None => host.to_string(),
        };
        let datetime = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date = now.format("%Y%m%d").to_string();
        let scope = format!("{date}/{}/{SERVICE}/{REQUEST_TYPE}", self.region);
        let credential = format!("{}/{scope}", self.access_key_id);
        let mut signed_url = url.clone();
        {
            let mut query = signed_url.query_pairs_mut();
            query.append_pair("x-oss-additional-headers", "host");
            query.append_pair("x-oss-credential", &credential);
            query.append_pair("x-oss-date", &datetime);
            query.append_pair("x-oss-expires", &expires_in_seconds.to_string());
            query.append_pair("x-oss-signature-version", ALGORITHM);
        }
        let mut headers = HeaderMap::new();
        headers.insert(
            HOST,
            HeaderValue::from_str(&host).map_err(|error| {
                AppError::new("OSS_SIGNING_FAILED", "failed to build OSS host header")
                    .with_detail("reason", error.to_string())
            })?,
        );
        let signed_headers = signed_header_names(&headers, &["host"]);
        let canonical_request = build_canonical_request(
            &Method::GET,
            &signed_url,
            Some(bucket),
            Some(object_key),
            &headers,
            &signed_headers,
        );
        let string_to_sign = format!(
            "{ALGORITHM}\n{datetime}\n{scope}\n{}",
            hex_lower(&Sha256::digest(canonical_request.as_bytes()))
        );
        let signature = calculate_signature(
            &self.access_key_secret,
            &date,
            &self.region,
            &string_to_sign,
        )?;
        signed_url
            .query_pairs_mut()
            .append_pair("x-oss-signature", &signature);
        Ok(signed_url)
    }
}

#[derive(Debug, Default)]
struct SignedHeaderNames {
    canonical: Vec<String>,
    additional: Vec<String>,
}

fn signed_header_names(headers: &HeaderMap, additional_headers: &[&str]) -> SignedHeaderNames {
    let mut canonical = Vec::new();
    let mut additional = Vec::new();
    let additional_set: std::collections::BTreeSet<String> = additional_headers
        .iter()
        .map(|header| header.to_ascii_lowercase())
        .collect();

    for (name, _) in headers {
        let name = name.as_str().to_ascii_lowercase();
        if is_default_signed_header(&name) {
            canonical.push(name);
        } else if additional_set.contains(&name) {
            canonical.push(name.clone());
            additional.push(name);
        }
    }

    canonical.sort();
    canonical.dedup();
    additional.sort();
    additional.dedup();
    SignedHeaderNames {
        canonical,
        additional,
    }
}

fn is_default_signed_header(name: &str) -> bool {
    name == "content-type" || name == "content-md5" || name.starts_with("x-oss-")
}

fn build_canonical_request(
    method: &Method,
    url: &Url,
    bucket: Option<&str>,
    object_key: Option<&str>,
    headers: &HeaderMap,
    signed_headers: &SignedHeaderNames,
) -> String {
    let mut resource = "/".to_string();
    if let Some(bucket) = bucket {
        resource.push_str(bucket);
        resource.push('/');
    }
    if let Some(object_key) = object_key {
        resource.push_str(object_key);
    }
    let canonical_uri = percent_encode(resource.as_bytes(), OSS_URI_ENCODE_SET).to_string();
    let canonical_query = canonical_query(url);
    let canonical_headers = signed_headers
        .canonical
        .iter()
        .map(|name| {
            let values = headers
                .get_all(name)
                .iter()
                .map(|value| value.to_str().unwrap_or_default().trim())
                .collect::<Vec<_>>()
                .join(",");
            format!("{name}:{values}\n")
        })
        .collect::<String>();
    let payload = headers
        .get("x-oss-content-sha256")
        .and_then(|value| value.to_str().ok())
        .unwrap_or(UNSIGNED_PAYLOAD);

    format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        method.as_str(),
        canonical_uri,
        canonical_query,
        canonical_headers,
        signed_headers.additional.join(";"),
        payload
    )
}

fn canonical_query(url: &Url) -> String {
    let Some(raw_query) = url.query() else {
        return String::new();
    };
    let mut parameters = raw_query
        .split('&')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let (raw_key, raw_value, has_value) = match part.split_once('=') {
                Some((key, value)) => (key, value, true),
                None => (part, "", false),
            };
            let key = encode_query_component(raw_key);
            let value = encode_query_component(raw_value);
            (key, value, has_value)
        })
        .collect::<Vec<_>>();
    parameters.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    parameters
        .into_iter()
        .map(|(key, value, has_value)| {
            if has_value {
                format!("{key}={value}")
            } else {
                key
            }
        })
        .collect::<Vec<_>>()
        .join("&")
}

fn encode_query_component(value: &str) -> String {
    let decoded = percent_decode_str(value).decode_utf8_lossy();
    percent_encode(decoded.as_bytes(), OSS_QUERY_ENCODE_SET).to_string()
}

fn calculate_signature(
    access_key_secret: &str,
    date: &str,
    region: &str,
    string_to_sign: &str,
) -> AppResult<String> {
    let date_key = hmac_bytes(
        format!("aliyun_v4{access_key_secret}").as_bytes(),
        date.as_bytes(),
    )?;
    let region_key = hmac_bytes(&date_key, region.as_bytes())?;
    let service_key = hmac_bytes(&region_key, SERVICE.as_bytes())?;
    let signing_key = hmac_bytes(&service_key, REQUEST_TYPE.as_bytes())?;
    Ok(hex_lower(&hmac_bytes(
        &signing_key,
        string_to_sign.as_bytes(),
    )?))
}

fn hmac_bytes(key: &[u8], value: &[u8]) -> AppResult<Vec<u8>> {
    let mut mac = HmacSha256::new_from_slice(key).map_err(|error| {
        AppError::new("OSS_SIGNING_FAILED", "failed to initialize OSS HMAC")
            .with_detail("reason", error.to_string())
    })?;
    mac.update(value);
    Ok(mac.finalize().into_bytes().to_vec())
}

fn hex_lower(value: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(value.len() * 2);
    for byte in value {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use reqwest::{
        header::{HeaderMap, HeaderValue},
        Method, Url,
    };

    use super::{build_canonical_request, canonical_query, OssV4Signer, ALGORITHM};

    #[test]
    fn rejects_endpoint_name_as_signing_region() {
        let error = OssV4Signer::new(
            "LTAI****************".into(),
            "secret".into(),
            "oss-cn-shanghai".into(),
        )
        .expect_err("endpoint name must not be used as V4 signing region");
        assert_eq!(error.code, "VALIDATION_ERROR");
        assert!(error.message.contains("region ID"));
    }

    #[test]
    fn canonical_query_sorts_encoded_parameters_and_preserves_flags() {
        let url = Url::parse(
            "https://examplebucket.oss-cn-hangzhou.aliyuncs.com/?uploads&prefix=a%20b&partNumber=2",
        )
        .unwrap();
        assert_eq!(canonical_query(&url), "partNumber=2&prefix=a%20b&uploads");
    }

    #[test]
    fn canonical_query_escapes_slashes_like_the_oss_v4_sdk() {
        let url = Url::parse(
            "https://examplebucket.oss-cn-shanghai.aliyuncs.com/?delimiter=%2F&prefix=capcut-world-2026%2Fai-photo%2F",
        )
        .unwrap();
        assert_eq!(
            canonical_query(&url),
            "delimiter=%2F&prefix=capcut-world-2026%2Fai-photo%2F"
        );
    }

    #[test]
    fn v4_signer_matches_official_put_object_signature_vector() {
        let signer = OssV4Signer::new(
            "LTAI****************".into(),
            "yourAccessKeySecret".into(),
            "cn-hangzhou".into(),
        )
        .unwrap();
        let url =
            Url::parse("https://examplebucket.oss-cn-hangzhou.aliyuncs.com/exampleobject").unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            "content-disposition",
            HeaderValue::from_static("attachment"),
        );
        headers.insert("content-length", HeaderValue::from_static("3"));
        headers.insert(
            "content-md5",
            HeaderValue::from_static("ICy5YqxZB1uWSwcVLSNLcA=="),
        );
        headers.insert("content-type", HeaderValue::from_static("text/plain"));
        signer
            .sign(
                &Method::PUT,
                &url,
                Some("examplebucket"),
                Some("exampleobject"),
                &mut headers,
                &["content-disposition", "content-length"],
                Utc.with_ymd_and_hms(2025, 4, 11, 6, 41, 24).unwrap(),
            )
            .unwrap();
        let authorization = headers.get("authorization").unwrap().to_str().unwrap();
        assert!(authorization.contains(
            "Signature=053edbf550ebd239b32a9cdfd93b0b2b3f2d223083aa61f75e9ac16856d61f23"
        ));
    }

    #[test]
    fn canonical_request_uses_bucket_and_object_key_in_resource_path() {
        let url = Url::parse(
            "https://examplebucket.oss-cn-hangzhou.aliyuncs.com/folder/%E6%B5%8B%E8%AF%95.txt",
        )
        .unwrap();
        let headers = HeaderMap::from_iter([(
            "x-oss-content-sha256".parse().unwrap(),
            HeaderValue::from_static("UNSIGNED-PAYLOAD"),
        )]);
        let signed_headers = super::signed_header_names(&headers, &[]);
        let canonical = build_canonical_request(
            &Method::GET,
            &url,
            Some("examplebucket"),
            Some("folder/测试.txt"),
            &headers,
            &signed_headers,
        );
        assert!(canonical.starts_with("GET\n/examplebucket/folder/%E6%B5%8B%E8%AF%95.txt\n"));
    }

    #[test]
    fn presigned_get_url_contains_required_query_without_secret() {
        let signer = OssV4Signer::new(
            "LTAI****************".into(),
            "yourAccessKeySecret".into(),
            "cn-shanghai".into(),
        )
        .unwrap();
        let url = Url::parse(
            "https://examplebucket.oss-cn-shanghai.aliyuncs.com/folder/%E6%B5%8B%E8%AF%95.txt",
        )
        .unwrap();
        let signed = signer
            .presign_get(
                &url,
                "examplebucket",
                "folder/测试.txt",
                900,
                Utc.with_ymd_and_hms(2025, 4, 11, 6, 41, 24).unwrap(),
            )
            .unwrap();
        let query: std::collections::HashMap<_, _> = signed.query_pairs().into_owned().collect();
        assert_eq!(query.get("x-oss-expires").map(String::as_str), Some("900"));
        assert_eq!(
            query.get("x-oss-additional-headers").map(String::as_str),
            Some("host")
        );
        assert_eq!(
            query.get("x-oss-signature-version").map(String::as_str),
            Some(ALGORITHM)
        );
        assert!(query
            .get("x-oss-signature")
            .is_some_and(|value| value.len() == 64));
        assert!(!signed.as_str().contains("yourAccessKeySecret"));
    }
}
