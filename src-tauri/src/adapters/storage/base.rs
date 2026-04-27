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
