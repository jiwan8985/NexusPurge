use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tokio::task::JoinSet;

use crate::adapters::storage::s3::S3Adapter;
use crate::utils::config::{ProfileConfig, ProfileStore};

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

/// upload_files 전용 아이템 — is_overwrite 로 CDN Purge 여부를 개별 제어
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
        files:                       result.files,
        next_continuation_token:     result.next_token,
        is_truncated:                result.is_truncated,
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

/// 파일 업로드 — is_overwrite == true 인 항목만 CDN Purge 트리거.
/// start_uploads (sync.rs) 와 달리 항목별로 Purge 여부를 제어한다.
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

    let mut tasks: JoinSet<()> = JoinSet::new();

    for item in items {
        let adapter = adapter.clone();
        let app = app.clone();
        let dist_id = cdn_distribution_id.clone();
        let provider = cdn_provider.clone();
        let creds_clone = creds.clone();

        tasks.spawn(async move {
            let id = item.id.clone();
            let app_p = app.clone();
            let id_p = id.clone();

            let result = adapter
                .upload_with_progress(
                    &item.local_path,
                    &item.remote_path,
                    move |transferred, total| {
                        let progress = if total > 0 {
                            (transferred * 100 / total) as u8
                        } else {
                            0
                        };
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
                Ok(_) => ("complete".to_string(), None),
                Err(e) => ("error".to_string(), Some(e.to_string())),
            };

            // is_overwrite == true 인 경우에만 CDN Purge 실행
            let (cdn_purged, cdn_purge_error) =
                if result.is_ok() && item.is_overwrite {
                    if let (Some(dist), Some(prov)) = (dist_id, provider) {
                        match crate::adapters::cdn::purge_with_credentials(
                            &prov,
                            &dist,
                            &[item.remote_path.clone()],
                            creds_clone,
                        )
                        .await
                        {
                            Ok(_) => (true, None),
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
