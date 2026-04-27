use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, State};
use tokio::task::JoinSet;

use crate::adapters::storage::s3::{S3Adapter, MULTIPART_THRESHOLD, PART_SIZE};
use crate::commands::s3::FileItem;
use crate::utils::config::ProfileStore;
use crate::utils::hash;

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncPlan {
    #[serde(rename = "toUpload")]
    pub to_upload:   Vec<FileItem>,
    #[serde(rename = "toSkip")]
    pub to_skip:     Vec<FileItem>,
    #[serde(rename = "toOverwrite")]
    pub to_overwrite: Vec<FileItem>,
}

#[derive(Debug, Serialize)]
pub struct FileEntry {
    #[serde(rename = "localPath")]
    pub local_path:  Option<String>,
    #[serde(rename = "remoteKey")]
    pub remote_key:  String,
    pub size:        u64,
    #[serde(rename = "localMd5")]
    pub local_md5:   Option<String>,
    #[serde(rename = "remoteEtag")]
    pub remote_etag: Option<String>,
}

/// 로컬 디렉터리 ↔ S3 prefix 전체 비교 결과
#[derive(Debug, Serialize)]
pub struct SyncResult {
    pub new:       Vec<FileEntry>, // 로컬에만 있음
    pub modified:  Vec<FileEntry>, // 양쪽 있으나 내용 다름
    pub deleted:   Vec<FileEntry>, // S3에만 있음
    pub unchanged: Vec<FileEntry>, // 양쪽 동일
}

#[derive(Debug, Deserialize)]
pub struct UploadItem {
    pub id: String,
    #[serde(rename = "localPath")]
    pub local_path:  String,
    #[serde(rename = "remotePath")]
    pub remote_path: String,
}

#[derive(Debug, Deserialize)]
pub struct DownloadItem {
    pub id: String,
    #[serde(rename = "remotePath")]
    pub remote_path: String,
    #[serde(rename = "localPath")]
    pub local_path:  String,
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
    id:      String,
    status:  String,
    #[serde(rename = "cdnPurged")]
    cdn_purged:       bool,
    #[serde(rename = "cdnPurgeError")]
    cdn_purge_error:  Option<String>,
    error:            Option<String>,
}

// ─── Commands ─────────────────────────────────────────────────────────────────

/// 지정된 로컬 파일 목록과 S3 ETag를 비교해 업로드/스킵/덮어쓰기 플랜 생성
#[tauri::command]
pub async fn build_sync_plan(
    profile_id:    String,
    local_paths:   Vec<String>,
    remote_prefix: String,
    store: State<'_, ProfileStore>,
) -> Result<SyncPlan, String> {
    let (creds, region, bucket, endpoint) = store
        .get_connection_info(&profile_id)
        .await
        .map_err(|e| e.to_string())?;

    let adapter = S3Adapter::new(&region, &bucket, &creds, endpoint.as_deref())
        .map_err(|e| e.to_string())?;

    let mut plan = SyncPlan {
        to_upload:   vec![],
        to_skip:     vec![],
        to_overwrite: vec![],
    };
    let mut tasks: JoinSet<
        Option<(String, String, String, u64, String, String, Option<String>)>,
    > = JoinSet::new();

    for local_path in local_paths {
        let adapter = adapter.clone();
        let prefix = remote_prefix.clone();

        tasks.spawn(async move {
            let path = Path::new(&local_path);
            let file_name = path.file_name()?.to_str()?.to_string();
            let remote_key = format!("{}{}", prefix, file_name);

            let metadata = tokio::fs::metadata(&local_path).await.ok()?;
            let size = metadata.len();
            let modified = metadata.modified().ok()?;
            let last_modified =
                chrono::DateTime::<chrono::Utc>::from(modified).to_rfc3339();

            let remote_etag = adapter.head_object_etag(&remote_key).await.ok().flatten();

            // 파일 크기에 따라 비교할 ETag를 결정:
            // 10MB 이상이면 S3 멀티파트 ETag 형식("hash-N")으로 계산해야 원격과 일치한다.
            let local_etag = if size >= MULTIPART_THRESHOLD {
                hash::calculate_multipart_etag(path, PART_SIZE).await.ok()?
            } else {
                hash::compute_file_md5(&local_path).await.ok()?
            };

            Some((
                local_path,
                file_name,
                remote_key,
                size,
                last_modified,
                local_etag,
                remote_etag,
            ))
        });
    }

    while let Some(Ok(Some((
        local_path,
        file_name,
        _remote_key,
        size,
        last_modified,
        local_etag,
        remote_etag,
    )))) = tasks.join_next().await
    {
        let item = FileItem {
            name:          file_name,
            path:          local_path,
            size,
            last_modified,
            is_directory:  false,
            etag:          Some(local_etag.clone()),
            content_type:  None,
        };

        match remote_etag {
            None => plan.to_upload.push(item),
            Some(etag) if etag == local_etag => plan.to_skip.push(item),
            Some(_) => plan.to_overwrite.push(item),
        }
    }

    Ok(plan)
}

/// 로컬 디렉터리 전체 ↔ S3 prefix 를 비교해 new / modified / deleted / unchanged 분류
/// (sync_preview Tauri 커맨드의 핵심 로직)
async fn compare_local_remote(
    profile_id:    &str,
    local_dir:     &str,
    remote_prefix: &str,
    store:         &ProfileStore,
) -> anyhow::Result<SyncResult> {
    let (creds, region, bucket, endpoint) =
        store.get_connection_info(profile_id).await?;
    let adapter = S3Adapter::new(&region, &bucket, &creds, endpoint.as_deref())?;

    // ── 로컬 파일 목록 수집 ──────────────────────────────────────────────
    let local_files = collect_local_files(Path::new(local_dir)).await?;

    // ── S3 오브젝트 목록 수집 ────────────────────────────────────────────
    use crate::adapters::storage::base::StorageAdapter;
    let remote_files = adapter.list_objects(remote_prefix).await?;

    // remote key → etag 맵
    let mut remote_map: std::collections::HashMap<String, (u64, Option<String>)> =
        remote_files
            .into_iter()
            .map(|f| (f.key.clone(), (f.size, f.etag)))
            .collect();

    // ── MD5 병렬 계산 + 분류 ─────────────────────────────────────────────
    let mut tasks: JoinSet<Option<FileEntry>> = JoinSet::new();

    for (relative_path, abs_path) in &local_files {
        let rel = relative_path.clone();
        let abs = abs_path.clone();
        let remote_key = if remote_prefix.is_empty() {
            rel.clone()
        } else {
            format!(
                "{}{}",
                remote_prefix.trim_end_matches('/'),
                if rel.starts_with('/') { rel.clone() } else { format!("/{}", rel) }
            )
        };
        let remote_meta = remote_map.remove(&remote_key);

        tasks.spawn(async move {
            let metadata = tokio::fs::metadata(&abs).await.ok()?;
            let size = metadata.len();

            let local_etag = if size >= MULTIPART_THRESHOLD {
                hash::calculate_multipart_etag(&abs, PART_SIZE).await.ok()?
            } else {
                hash::calculate_md5(&abs).await.ok()?
            };

            Some(FileEntry {
                local_path:  Some(abs.to_string_lossy().into_owned()),
                remote_key,
                size,
                local_md5:   Some(local_etag),
                remote_etag: remote_meta.map(|(_, etag)| etag).flatten(),
            })
        });
    }

    let mut result = SyncResult {
        new:       vec![],
        modified:  vec![],
        deleted:   vec![],
        unchanged: vec![],
    };

    while let Some(Ok(Some(entry))) = tasks.join_next().await {
        match &entry.remote_etag {
            None => result.new.push(entry),
            Some(etag) if entry.local_md5.as_deref() == Some(etag.as_str()) => {
                result.unchanged.push(entry);
            }
            Some(_) => result.modified.push(entry),
        }
    }

    // S3에만 남은 항목 → deleted
    for (remote_key, (size, etag)) in remote_map {
        result.deleted.push(FileEntry {
            local_path:  None,
            remote_key,
            size,
            local_md5:   None,
            remote_etag: etag,
        });
    }

    Ok(result)
}

/// Dry-run: 로컬 디렉터리 전체 ↔ S3 prefix 비교 결과 반환 (실제 전송 없음)
#[tauri::command]
pub async fn sync_preview(
    profile_id:    String,
    local_dir:     String,
    remote_prefix: String,
    store: State<'_, ProfileStore>,
) -> Result<SyncResult, String> {
    compare_local_remote(&profile_id, &local_dir, &remote_prefix, &store)
        .await
        .map_err(|e| e.to_string())
}

/// 업로드 실행 (병렬, 진행률 이벤트 emit, 완료 후 CDN Purge)
#[tauri::command]
pub async fn start_uploads(
    app:                  AppHandle,
    profile_id:           String,
    items:                Vec<UploadItem>,
    cdn_distribution_id:  Option<String>,
    cdn_provider:         Option<String>,
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

            let (cdn_purged, cdn_purge_error) = if result.is_ok() {
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

/// 다운로드 실행 (병렬, 진행률 이벤트 emit)
#[tauri::command]
pub async fn start_downloads(
    app:        AppHandle,
    profile_id: String,
    items:      Vec<DownloadItem>,
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
                .download_with_progress(
                    &item.remote_path,
                    &item.local_path,
                    move |transferred, total| {
                        let progress = if total > 0 {
                            (transferred * 100 / total) as u8
                        } else {
                            50
                        };
                        let _ = app_p.emit(
                            "transfer:progress",
                            TransferProgressPayload {
                                id:                id_p.clone(),
                                progress,
                                transferred_bytes: transferred,
                                speed:             0,
                                status:            "downloading".into(),
                            },
                        );
                    },
                )
                .await;

            let (status, error) = match result {
                Ok(_) => ("complete".to_string(), None),
                Err(e) => ("error".to_string(), Some(e.to_string())),
            };

            let _ = app.emit(
                "transfer:complete",
                TransferCompletePayload {
                    id,
                    status,
                    cdn_purged:      false,
                    cdn_purge_error: None,
                    error,
                },
            );
        });
    }

    while tasks.join_next().await.is_some() {}
    Ok(())
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// 로컬 디렉터리를 재귀적으로 순회해 (상대경로, 절대경로) 목록 반환
async fn collect_local_files(dir: &Path) -> anyhow::Result<Vec<(String, PathBuf)>> {
    let mut result: Vec<(String, PathBuf)> = Vec::new();
    let mut stack: Vec<PathBuf> = vec![dir.to_path_buf()];

    while let Some(current) = stack.pop() {
        let mut entries = tokio::fs::read_dir(&current)
            .await
            .map_err(|e| anyhow::anyhow!("디렉터리 읽기 실패 {}: {}", current.display(), e))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| anyhow::anyhow!("엔트리 읽기 실패: {}", e))?
        {
            let path = entry.path();
            let meta = entry
                .metadata()
                .await
                .map_err(|e| anyhow::anyhow!("메타데이터 읽기 실패: {}", e))?;

            if meta.is_dir() {
                stack.push(path);
            } else {
                let relative = path
                    .strip_prefix(dir)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/"); // Windows 경로 구분자 정규화
                result.push((relative, path));
            }
        }
    }

    Ok(result)
}
