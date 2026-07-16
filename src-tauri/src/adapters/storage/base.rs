use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::sync::mpsc;

use crate::commands::s3::FileItem;

// ─── Shared Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteFile {
    pub key:           String,
    pub size:          u64,
    pub etag:          Option<String>,
    pub last_modified: String,
    pub content_type:  Option<String>,
}

/// 진행률 이벤트 (UnboundedSender로 비동기 없이 전송)
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Progress {
    pub transferred: u64,
    pub total:       u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadResult {
    pub key:          String,
    pub etag:         Option<String>,
    pub size:         u64,
    pub is_multipart: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ObjectMeta {
    pub key:           String,
    pub size:          u64,
    pub etag:          Option<String>,
    pub last_modified: String,
    pub content_type:  Option<String>,
}

/// 속성(우클릭) 다이얼로그 — HeadObject 응답 전체를 담는 상세 정보.
/// 크롬 개발자모드에서 보는 응답 헤더 수준의 정보를 그대로 노출한다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3ObjectDetail {
    pub key: String,
    pub etag: Option<String>,
    #[serde(rename = "contentLength")]
    pub content_length: Option<i64>,
    #[serde(rename = "contentType")]
    pub content_type: Option<String>,
    #[serde(rename = "contentEncoding")]
    pub content_encoding: Option<String>,
    #[serde(rename = "contentDisposition")]
    pub content_disposition: Option<String>,
    #[serde(rename = "contentLanguage")]
    pub content_language: Option<String>,
    #[serde(rename = "cacheControl")]
    pub cache_control: Option<String>,
    #[serde(rename = "lastModified")]
    pub last_modified: Option<String>,
    #[serde(rename = "storageClass")]
    pub storage_class: Option<String>,
    #[serde(rename = "serverSideEncryption")]
    pub server_side_encryption: Option<String>,
    #[serde(rename = "sseKmsKeyId")]
    pub sse_kms_key_id: Option<String>,
    #[serde(rename = "versionId")]
    pub version_id: Option<String>,
    #[serde(rename = "replicationStatus")]
    pub replication_status: Option<String>,
    #[serde(rename = "acceptRanges")]
    pub accept_ranges: Option<String>,
    #[serde(rename = "checksumCrc32")]
    pub checksum_crc32: Option<String>,
    #[serde(rename = "checksumSha256")]
    pub checksum_sha256: Option<String>,
    /// 사용자 정의 메타데이터 (x-amz-meta-*)
    pub metadata: std::collections::HashMap<String, String>,
}

// ListResult는 기존 commands/s3.rs 의 FileItem 타입을 그대로 사용 (하위 호환)
pub struct ListResult {
    pub files:        Vec<FileItem>,
    pub next_token:   Option<String>,
    pub is_truncated: bool,
}

// ─── StorageAdapter Trait (AFIT, Rust 1.75+) ─────────────────────────────────
//
// dyn StorageAdapter 는 사용하지 않으므로 AFIT로 정의. 각 구현체 Future 의
// Send 여부는 구체 타입 추론에 맡긴다.

#[allow(dead_code)]
pub trait StorageAdapter: Send + Sync {
    /// prefix 하위 오브젝트 목록 (페이지네이션 없이 최대 1000개)
    async fn list_objects(&self, prefix: &str) -> Result<Vec<RemoteFile>>;

    /// 파일 업로드. 10 MB 이상은 자동으로 멀티파트 업로드
    async fn upload_file(
        &self,
        local: &Path,
        key: &str,
        tx: mpsc::UnboundedSender<Progress>,
    ) -> Result<UploadResult>;

    /// 파일 다운로드 (스트리밍)
    async fn download_file(
        &self,
        key: &str,
        local: &Path,
        tx: mpsc::UnboundedSender<Progress>,
    ) -> Result<()>;

    /// 단일 오브젝트 삭제
    async fn delete_object(&self, key: &str) -> Result<()>;

    /// 오브젝트 이름 변경 (S3: copy → delete)
    async fn rename_object(&self, old_key: &str, new_key: &str) -> Result<()>;

    /// 오브젝트 메타데이터 조회
    async fn head_object(&self, key: &str) -> Result<ObjectMeta>;
}
