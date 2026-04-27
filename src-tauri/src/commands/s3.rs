use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::adapters::storage::s3::S3Adapter;
use crate::utils::config::{AwsCredentials, ProfileConfig, ProfileStore};

// ─── 동시 파일 전송 상한 (H-2) ───────────────────────────────────────────────
const MAX_CONCURRENT_FILES: usize = 4;

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileItem {
    pub name: String,
    pub path: String,
    pub size: u64,
    #[serde(rename = "lastModified")]
    pub last_modified: String,
    #[serde(rename = "isDirectory")]
    pub is_directory: bool,
    pub etag: Option<String>,
    #[serde(rename = "contentType")]
    pub content_type: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct S3ListResponse {
    pub files: Vec<FileItem>,
    #[serde(rename = "nextContinuationToken")]
    pub next_continuation_token: Option<String>,
    #[serde(rename = "isTruncated")]
    pub is_truncated: bool,
}

/// upload_files 전용 아이템 — is_overwrite 로 CDN Purge 여부를 개별 제어 (C-1)
#[derive(Debug, Deserialize)]
pub struct UploadFileItem {
    pub id: String,
    #[serde(rename = "localPath")]
    pub local_path:  String,
    #[serde(rename = "remotePath")]
    pub remote_path: String,
    /// true → 기존 파일 덮어쓰기, CDN Purge 트리거
    #[serde(rename = "isOverwrite")]
    pub is_overwrite: bool,
}

#[derive(Debug, Serialize, Clone)]
struct TransferProgressPayload {
    id:                String,
    progress:          u8,
    #[serde(rename = "transferredBytes")]
    transferred_bytes: u64,
    speed:             u64,
    status:            String,
}

#[derive(Debug, Serialize, Clone)]
struct TransferCompletePayload {
    id:     String,
    status: String,
    #[serde(rename = "cdnPurged")]
    cdn_purged:      bool,
    #[serde(rename = "cdnPurgeError")]
    cdn_purge_error: Option<String>,
    error:           Option<String>,
}

// ─── Profile Commands ─────────────────────────────────────────────────────────

#[tauri::command]
pub async fn load_profiles(
    store: State<'_, ProfileStore>,
) -> Result<Vec<ProfileConfig>, String> {
    store.load_all().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_profile(
    profile: ProfileConfig,
    store: State<'_, ProfileStore>,
) -> Result<(), String> {
    store.save(profile).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_profile(
    id: String,
    store: State<'_, ProfileStore>,
) -> Result<(), String> {
    store.delete(&id).await.map_err(|e| e.to_string())
}

// ─── Connection ───────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn connect_s3(
    profile_id: String,
    store: State<'_, ProfileStore>,
) -> Result<(), String> {
    let (creds, region, bucket, endpoint) = store
        .get_connection_info(&profile_id)
        .await
        .map_err(|e| e.to_string())?;

    S3Adapter::new(&region, &bucket, &creds, endpoint.as_deref())
        .map_err(|e| e.to_string())?
        .verify_access()
        .await
        .map_err(|e| e.to_string())
}

/// H-3: 프로필 저장 없이 입력값으로 직접 연결 테스트
#[tauri::command]
pub async fn test_s3_connection(
    region:     String,
    bucket:     String,
    access_key: String,
    secret_key: String,
    endpoint:   Option<String>,
) -> Result<(), String> {
    let creds = AwsCredentials {
        access_key_id: access_key,
        secret_access_key: secret_key,
    };
    S3Adapter::new(&region, &bucket, &creds, endpoint.as_deref())
        .map_err(|e| e.to_string())?
        .verify_access()
        .await
        .map_err(|e| e.to_string())
}

// ─── S3 Object Operations ─────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_s3_objects(
    profile_id: String,
    prefix: String,
    store: State<'_, ProfileStore>,
) -> Result<S3ListResponse, String> {
    let (creds, region, bucket, endpoint) = store
        .get_connection_info(&profile_id)
        .await
        .map_err(|e| e.to_string())?;

    let adapter = S3Adapter::new(&region, &bucket, &creds, endpoint.as_deref())
        .map_err(|e| e.to_string())?;

    let result = adapter
        .list_objects_raw(&prefix)
        .await
        .map_err(|e| e.to_string())?;

    Ok(S3ListResponse {
        files:                   result.files,
        next_continuation_token: result.next_token,
        is_truncated:            result.is_truncated,
    })
}

#[tauri::command]
pub async fn delete_s3_objects(
    profile_id: String,
    keys: Vec<String>,
    store: State<'_, ProfileStore>,
) -> Result<(), String> {
    let (creds, region, bucket, endpoint) = store
        .get_connection_info(&profile_id)
        .await
        .map_err(|e| e.to_string())?;

    S3Adapter::new(&region, &bucket, &creds, endpoint.as_deref())
        .map_err(|e| e.to_string())?
        .delete_objects(&keys)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn put_s3_object(
    profile_id:   String,
    key:          String,
    content:      Vec<u8>,
    content_type: String,
    store: State<'_, ProfileStore>,
) -> Result<(), String> {
    let (creds, region, bucket, endpoint) = store
        .get_connection_info(&profile_id)
        .await
        .map_err(|e| e.to_string())?;

    S3Adapter::new(&region, &bucket, &creds, endpoint.as_deref())
        .map_err(|e| e.to_string())?
        .put_object(&key, content, &content_type)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_presigned_url(
    profile_id:        String,
    key:               String,
    expires_in_seconds: u64,
    store: State<'_, ProfileStore>,
) -> Result<String, String> {
    let (creds, region, bucket, endpoint) = store
        .get_connection_info(&profile_id)
        .await
        .map_err(|e| e.to_string())?;

    S3Adapter::new(&region, &bucket, &creds, endpoint.as_deref())
        .map_err(|e| e.to_string())?
        .presign_get(&key, expires_in_seconds)
        .await
        .map_err(|e| e.to_string())
}

/// H-1: S3 오브젝트 이름 변경 (CopyObject + DeleteObject)
#[tauri::command]
pub async fn rename_s3_object(
    profile_id: String,
    old_key:    String,
    new_key:    String,
    store: State<'_, ProfileStore>,
) -> Result<(), String> {
    let (creds, region, bucket, endpoint) = store
        .get_connection_info(&profile_id)
        .await
        .map_err(|e| e.to_string())?;

    S3Adapter::new(&region, &bucket, &creds, endpoint.as_deref())
        .map_err(|e| e.to_string())?
        .rename_object(&old_key, &new_key)
        .await
        .map_err(|e| e.to_string())
}

// ─── Local FS Operations (H-1) ────────────────────────────────────────────────

/// OS별 홈 디렉토리 반환 (macOS/Linux: ~/  Windows: C:\Users\...)
#[tauri::command]
pub fn get_home_dir() -> String {
    dirs::home_dir()
        .unwrap_or_else(|| {
            #[cfg(windows)]
            { std::path::PathBuf::from("C:\\") }
            #[cfg(not(windows))]
            { std::path::PathBuf::from("/") }
        })
        .to_string_lossy()
        .into_owned()
}

#[tauri::command]
pub async fn list_local_dir(path: String) -> Result<Vec<FileItem>, String> {
    let dir = std::path::Path::new(&path);
    if !dir.is_dir() {
        return Err(format!("디렉터리가 아닙니다: {}", path));
    }
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("디렉터리 읽기 실패: {}", e))?;

    let mut files: Vec<FileItem> = entries
        .filter_map(|e| e.ok())
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                return None;
            }
            let last_modified = meta
                .modified()
                .ok()
                .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339())
                .unwrap_or_default();
            Some(FileItem {
                name,
                path: entry.path().to_string_lossy().into_owned(),
                size: if meta.is_file() { meta.len() } else { 0 },
                last_modified,
                is_directory: meta.is_dir(),
                etag: None,
                content_type: None,
            })
        })
        .collect();

    files.sort_by(|a, b| {
        b.is_directory
            .cmp(&a.is_directory)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(files)
}

/// H-1: 로컬 디렉터리 생성
#[tauri::command]
pub async fn create_local_dir(path: String) -> Result<(), String> {
    tokio::fs::create_dir_all(&path)
        .await
        .map_err(|e| format!("폴더 생성 실패: {}", e))
}

/// H-1: 로컬 파일/폴더 삭제
#[tauri::command]
pub async fn delete_local_files(paths: Vec<String>) -> Result<(), String> {
    for path in &paths {
        let p = std::path::Path::new(path);
        if p.is_dir() {
            tokio::fs::remove_dir_all(p)
                .await
                .map_err(|e| format!("폴더 삭제 실패 ({}): {}", path, e))?;
        } else {
            tokio::fs::remove_file(p)
                .await
                .map_err(|e| format!("파일 삭제 실패 ({}): {}", path, e))?;
        }
    }
    Ok(())
}

/// H-1: 로컬 파일/폴더 이름 변경 (새 이름만 받음, 같은 디렉터리 내)
#[tauri::command]
pub async fn rename_local_file(old_path: String, new_name: String) -> Result<(), String> {
    let old = std::path::Path::new(&old_path);
    let new = old
        .parent()
        .ok_or_else(|| "부모 디렉터리를 찾을 수 없음".to_string())?
        .join(&new_name);
    tokio::fs::rename(&old, &new)
        .await
        .map_err(|e| format!("이름 변경 실패: {}", e))
}

// ─── Settings Commands (H-7) ──────────────────────────────────────────────────

/// H-7: 마지막 연결 프로파일 ID 저장
#[tauri::command]
pub async fn save_last_profile_id(
    id: String,
    store: State<'_, ProfileStore>,
) -> Result<(), String> {
    store.save_last_profile_id(&id).await.map_err(|e| e.to_string())
}

/// H-7: 마지막 연결 프로파일 ID 조회
#[tauri::command]
pub async fn get_last_profile_id(
    store: State<'_, ProfileStore>,
) -> Result<Option<String>, String> {
    store.get_last_profile_id().await.map_err(|e| e.to_string())
}

// ─── Upload (H-2 Semaphore + H-6 CdnCredentials) ─────────────────────────────

/// 파일 업로드 — is_overwrite == true 인 항목만 CDN Purge 트리거.
/// H-2: Semaphore로 동시 업로드 4개 제한.
/// H-6: CdnCredentials 기반 CDN Purge (Akamai 지원).
#[tauri::command]
pub async fn upload_files(
    app:                 AppHandle,
    profile_id:          String,
    items:               Vec<UploadFileItem>,
    cdn_distribution_id: Option<String>,
    cdn_provider:        Option<String>,
    store: State<'_, ProfileStore>,
) -> Result<(), String> {
    let (creds, region, bucket, endpoint) = store
        .get_connection_info(&profile_id)
        .await
        .map_err(|e| e.to_string())?;

    let adapter = S3Adapter::new(&region, &bucket, &creds, endpoint.as_deref())
        .map_err(|e| e.to_string())?;

    // H-6: CDN 자격증명 사전 조회 (태스크 외부에서 한 번만 실행)
    let cdn_info: Option<(String, crate::utils::config::CdnCredentials)> =
        match (&cdn_distribution_id, &cdn_provider) {
            (Some(dist), Some(prov)) => store
                .get_cdn_credentials(&profile_id, prov)
                .await
                .ok()
                .map(|c| (dist.clone(), c)),
            _ => None,
        };
    let cdn_info = Arc::new(cdn_info);

    // H-2: 동시 실행 제한
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_FILES));
    let mut tasks: JoinSet<()> = JoinSet::new();

    for item in items {
        let adapter   = adapter.clone();
        let app       = app.clone();
        let cdn_info  = cdn_info.clone();
        let permit    = semaphore.clone().acquire_owned().await.expect("Semaphore 오류");

        tasks.spawn(async move {
            let _permit = permit;
            let id   = item.id.clone();
            let app_p = app.clone();
            let id_p  = id.clone();

            let result = adapter
                .upload_with_progress(
                    &item.local_path,
                    &item.remote_path,
                    move |transferred, total| {
                        let progress = if total > 0 { (transferred * 100 / total) as u8 } else { 0 };
                        let _ = app_p.emit(
                            "transfer:progress",
                            TransferProgressPayload {
                                id:                id_p.clone(),
                                progress,
                                transferred_bytes: transferred,
                                speed:             0,
                                status:            "uploading".into(),
                            },
                        );
                    },
                )
                .await;

            let (status, error) = match &result {
                Ok(_)  => ("complete".to_string(), None),
                Err(e) => ("error".to_string(), Some(e.to_string())),
            };

            // is_overwrite == true 인 경우에만 CDN Purge 실행 (C-1)
            let (cdn_purged, cdn_purge_error) =
                if result.is_ok() && item.is_overwrite {
                    if let Some((dist, cdn_creds)) = cdn_info.as_ref() {
                        match crate::adapters::cdn::purge_with_credentials(
                            dist,
                            &[item.remote_path.clone()],
                            cdn_creds.clone(),
                        )
                        .await
                        {
                            Ok(_)  => (true, None),
                            Err(e) => (false, Some(e.to_string())),
                        }
                    } else {
                        (false, None)
                    }
                } else {
                    (false, None)
                };

            let _ = app.emit(
                "transfer:complete",
                TransferCompletePayload {
                    id,
                    status,
                    cdn_purged,
                    cdn_purge_error,
                    error,
                },
            );
        });
    }

    while tasks.join_next().await.is_some() {}
    Ok(())
}
