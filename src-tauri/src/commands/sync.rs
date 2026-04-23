use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::{AppHandle, Emitter, State};
use tokio::task::JoinSet;
use crate::adapters::storage::s3::S3Adapter;
use crate::utils::config::ProfileStore;
use crate::utils::hash;
use crate::commands::s3::FileItem;

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncPlan {
    #[serde(rename = "toUpload")]
    pub to_upload: Vec<FileItem>,
    #[serde(rename = "toSkip")]
    pub to_skip: Vec<FileItem>,
    #[serde(rename = "toOverwrite")]
    pub to_overwrite: Vec<FileItem>,
}

#[derive(Debug, Deserialize)]
pub struct UploadItem {
    pub id: String,
    #[serde(rename = "localPath")]
    pub local_path: String,
    #[serde(rename = "remotePath")]
    pub remote_path: String,
}

#[derive(Debug, Deserialize)]
pub struct DownloadItem {
    pub id: String,
    #[serde(rename = "remotePath")]
    pub remote_path: String,
    #[serde(rename = "localPath")]
    pub local_path: String,
}

#[derive(Debug, Serialize, Clone)]
struct TransferProgressPayload {
    id: String,
    progress: u8,
    #[serde(rename = "transferredBytes")]
    transferred_bytes: u64,
    speed: u64,
    status: String,
}

#[derive(Debug, Serialize, Clone)]
struct TransferCompletePayload {
    id: String,
    status: String,
    #[serde(rename = "cdnPurged")]
    cdn_purged: bool,
    #[serde(rename = "cdnPurgeError")]
    cdn_purge_error: Option<String>,
    error: Option<String>,
}

// ─── Commands ─────────────────────────────────────────────────────────────────

/// 로컬 파일과 S3 ETag를 비교해 업로드/스킵/덮어쓰기 플랜 생성
#[tauri::command]
pub async fn build_sync_plan(
    profile_id: String,
    local_paths: Vec<String>,
    remote_prefix: String,
    store: State<'_, ProfileStore>,
) -> Result<SyncPlan, String> {
    let (creds, region, bucket, endpoint) = store
        .get_connection_info(&profile_id)
        .await
        .map_err(|e| e.to_string())?;

    let adapter = S3Adapter::new(&region, &bucket, &creds, endpoint.as_deref())
        .map_err(|e| e.to_string())?;

    let mut plan = SyncPlan { to_upload: vec![], to_skip: vec![], to_overwrite: vec![] };
    let mut tasks: JoinSet<Option<(String, String, String, u64, String, String, Option<String>)>> = JoinSet::new();

    for local_path in local_paths {
        let adapter = adapter.clone();
        let prefix = remote_prefix.clone();

        tasks.spawn(async move {
            let path = Path::new(&local_path);
            let file_name = path.file_name()?.to_str()?.to_string();
            let remote_key = format!("{}{}", prefix, file_name);

            let local_md5 = hash::compute_file_md5(&local_path).await.ok()?;
            let metadata = tokio::fs::metadata(&local_path).await.ok()?;
            let size = metadata.len();
            let modified = metadata.modified().ok()?;
            let last_modified = chrono::DateTime::<chrono::Utc>::from(modified).to_rfc3339();

            let remote_etag = adapter.head_object(&remote_key).await.ok().flatten();

            Some((local_path, file_name, remote_key, size, last_modified, local_md5, remote_etag))
        });
    }

    while let Some(Ok(Some((local_path, file_name, _remote_key, size, last_modified, local_md5, remote_etag)))) =
        tasks.join_next().await
    {
        let item = FileItem {
            name: file_name,
            path: local_path,
            size,
            last_modified,
            is_directory: false,
            etag: Some(local_md5.clone()),
            content_type: None,
        };

        match remote_etag {
            None => plan.to_upload.push(item),
            Some(etag) if etag == local_md5 => plan.to_skip.push(item),
            Some(_) => plan.to_overwrite.push(item),
        }
    }

    Ok(plan)
}

/// 업로드 실행 (병렬, 진행률 이벤트 emit, 완료 후 CDN Purge)
#[tauri::command]
pub async fn start_uploads(
    app: AppHandle,
    profile_id: String,
    items: Vec<UploadItem>,
    cdn_distribution_id: Option<String>,
    cdn_provider: Option<String>,
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
                .upload_file(&item.local_path, &item.remote_path, move |transferred, total| {
                    let progress = if total > 0 { (transferred * 100 / total) as u8 } else { 0 };
                    let _ = app_p.emit("transfer:progress", TransferProgressPayload {
                        id: id_p.clone(),
                        progress,
                        transferred_bytes: transferred,
                        speed: 0,
                        status: "uploading".into(),
                    });
                })
                .await;

            let (status, error) = match &result {
                Ok(_) => ("complete".to_string(), None),
                Err(e) => ("error".to_string(), Some(e.to_string())),
            };

            let (cdn_purged, cdn_purge_error) = if result.is_ok() {
                if let (Some(dist), Some(prov)) = (dist_id, provider) {
                    match crate::adapters::cdn::purge_with_credentials(
                        &prov, &dist, &[item.remote_path.clone()], creds_clone,
                    ).await {
                        Ok(_) => (true, None),
                        Err(e) => (false, Some(e.to_string())),
                    }
                } else {
                    (false, None)
                }
            } else {
                (false, None)
            };

            let _ = app.emit("transfer:complete", TransferCompletePayload {
                id, status, cdn_purged, cdn_purge_error, error,
            });
        });
    }

    while tasks.join_next().await.is_some() {}
    Ok(())
}

/// 다운로드 실행 (병렬, 진행률 이벤트 emit)
#[tauri::command]
pub async fn start_downloads(
    app: AppHandle,
    profile_id: String,
    items: Vec<DownloadItem>,
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

        tasks.spawn(async move {
            let id = item.id.clone();
            let app_p = app.clone();
            let id_p = id.clone();

            let result = adapter
                .download_file(&item.remote_path, &item.local_path, move |transferred, total| {
                    let progress = if total > 0 { (transferred * 100 / total) as u8 } else { 50 };
                    let _ = app_p.emit("transfer:progress", TransferProgressPayload {
                        id: id_p.clone(),
                        progress,
                        transferred_bytes: transferred,
                        speed: 0,
                        status: "downloading".into(),
                    });
                })
                .await;

            let (status, error) = match result {
                Ok(_) => ("complete".to_string(), None),
                Err(e) => ("error".to_string(), Some(e.to_string())),
            };

            let _ = app.emit("transfer:complete", TransferCompletePayload {
                id, status, cdn_purged: false, cdn_purge_error: None, error,
            });
        });
    }

    while tasks.join_next().await.is_some() {}
    Ok(())
}
