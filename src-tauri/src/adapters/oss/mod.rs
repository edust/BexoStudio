mod alibaba;
mod credentials;
mod v4_signer;
mod xml;

pub use alibaba::AlibabaOssAdapter;
pub use credentials::OssCredentialStore;
pub use v4_signer::OssV4Signer;
pub use xml::{
    parse_error_response, parse_initiate_multipart_upload_response, parse_list_objects_response,
    parse_object_metadata,
};

use std::future::Future;
use std::pin::Pin;

use crate::{
    domain::{OssCredential, OssObjectMetadata, OssObjectPage, OssTargetRecord},
    error::AppResult,
};

#[derive(Debug, Clone)]
pub struct OssObjectListRequest {
    pub target: OssTargetRecord,
    pub credential: OssCredential,
    pub prefix: String,
    pub continuation_token: Option<String>,
    pub page_size: u16,
}

#[derive(Debug, Clone)]
pub struct OssObjectHeadRequest {
    pub target: OssTargetRecord,
    pub credential: OssCredential,
    pub object_key: String,
}

#[derive(Debug, Clone)]
pub struct OssObjectPutRequest {
    pub target: OssTargetRecord,
    pub credential: OssCredential,
    pub object_key: String,
    pub body: Vec<u8>,
    pub content_type: Option<String>,
    pub overwrite: bool,
}

#[derive(Debug, Clone)]
pub struct OssObjectDeleteRequest {
    pub target: OssTargetRecord,
    pub credential: OssCredential,
    pub object_key: String,
}

#[derive(Debug, Clone)]
pub struct OssObjectCopyRequest {
    pub target: OssTargetRecord,
    pub credential: OssCredential,
    pub source_key: String,
    pub target_key: String,
    pub overwrite: bool,
}

#[derive(Debug, Clone)]
pub struct OssObjectGetRequest {
    pub target: OssTargetRecord,
    pub credential: OssCredential,
    pub object_key: String,
    pub range_start: Option<u64>,
    pub range_end: Option<u64>,
}

#[derive(Debug)]
pub struct OssObjectDownload {
    response: reqwest::Response,
}

impl OssObjectDownload {
    pub fn content_length(&self) -> Option<u64> {
        self.response.content_length()
    }

    pub async fn chunk(&mut self) -> Result<Option<Vec<u8>>, reqwest::Error> {
        self.response
            .chunk()
            .await
            .map(|chunk| chunk.map(|bytes| bytes.to_vec()))
    }
}

#[derive(Debug, Clone)]
pub struct OssMultipartInitiateRequest {
    pub target: OssTargetRecord,
    pub credential: OssCredential,
    pub object_key: String,
    pub content_type: Option<String>,
    pub overwrite: bool,
}

#[derive(Debug, Clone)]
pub struct OssMultipartUploadPartRequest {
    pub target: OssTargetRecord,
    pub credential: OssCredential,
    pub object_key: String,
    pub upload_id: String,
    pub part_number: u32,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct OssMultipartCompletePart {
    pub part_number: u32,
    pub etag: String,
}

#[derive(Debug, Clone)]
pub struct OssMultipartCompleteRequest {
    pub target: OssTargetRecord,
    pub credential: OssCredential,
    pub object_key: String,
    pub upload_id: String,
    pub parts: Vec<OssMultipartCompletePart>,
    pub overwrite: bool,
}

#[derive(Debug, Clone)]
pub struct OssMultipartAbortRequest {
    pub target: OssTargetRecord,
    pub credential: OssCredential,
    pub object_key: String,
    pub upload_id: String,
}

#[derive(Debug, Clone)]
pub struct OssMultipartUpload {
    pub upload_id: String,
}

#[derive(Debug, Clone)]
pub struct OssUploadedPart {
    pub part_number: u32,
    pub etag: String,
}

pub trait ObjectStorageAdapter: Send + Sync {
    fn list_objects(
        &self,
        request: OssObjectListRequest,
    ) -> Pin<Box<dyn Future<Output = AppResult<OssObjectPage>> + Send + '_>>;
    fn head_object(
        &self,
        request: OssObjectHeadRequest,
    ) -> Pin<Box<dyn Future<Output = AppResult<OssObjectMetadata>> + Send + '_>>;
    fn put_object(
        &self,
        request: OssObjectPutRequest,
    ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + '_>>;
    fn delete_object(
        &self,
        request: OssObjectDeleteRequest,
    ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + '_>>;
    fn copy_object(
        &self,
        request: OssObjectCopyRequest,
    ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + '_>>;
    fn get_object(
        &self,
        request: OssObjectGetRequest,
    ) -> Pin<Box<dyn Future<Output = AppResult<OssObjectDownload>> + Send + '_>>;
    fn initiate_multipart_upload(
        &self,
        request: OssMultipartInitiateRequest,
    ) -> Pin<Box<dyn Future<Output = AppResult<OssMultipartUpload>> + Send + '_>>;
    fn upload_multipart_part(
        &self,
        request: OssMultipartUploadPartRequest,
    ) -> Pin<Box<dyn Future<Output = AppResult<OssUploadedPart>> + Send + '_>>;
    fn complete_multipart_upload(
        &self,
        request: OssMultipartCompleteRequest,
    ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + '_>>;
    fn abort_multipart_upload(
        &self,
        request: OssMultipartAbortRequest,
    ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + '_>>;
}
