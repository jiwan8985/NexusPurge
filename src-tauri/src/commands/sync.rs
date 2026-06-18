use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinSet;

// H-2: 동시 파일 전송 상한
const MAX_CONCURRENT_FILES: usize = 4;
const MAX_CDN_PURGE_PATHS_PER_REQUEST: usize = 1000;

use crate::adapters::storage::s3::{S3Adapter, MULTIPART_THRESHOLD, PART_SIZE};
use crate::commands::s3::FileItem;
use crate::utils::adapter_cache::AdapterCache;
use crate::utils::config::ProfileStore;
use crate::utils::hash;
use crate::utils::transfer_control::TransferControl;

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncPlan {
    #[serde(rename = "toUpload")]
    pub to_upload:   Vec<FileItem>,
    #[serde(rename = "toSkip")]
    pub to_skip:     Vec<FileItem>,
    #[serde(rename = "toOverwrite")]
    pub to_overwrite: Vec<FileItem>,
    #[serde(rename = "purgeTargets")]
    pub purge_targets: Vec<String>,
    #[serde(rename = "compareMode")]
    pub compare_mode: String,
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
    #[serde(rename = "remoteSize")]
    pub remote_size: Option<u64>,
}

/// 로컬 디렉터리 ↔ S3 prefix 전체 비교 결과
#[derive(Debug, Serialize)]
pub struct SyncResult {
    pub new:       Vec<FileEntry>, // 로컬에만 있음
    pub modified:  Vec<FileEntry>, // 양쪽 있으나 내용 다름
    pub deleted:   Vec<FileEntry>, // S3에만 있음
    pub unchanged: Vec<FileEntry>, // 양쪽 동일
    #[serde(rename = "purgeTargets")]
    pub purge_targets: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UploadItem {
    pub id: String,
    #[serde(rename = "localPath")]
    pub local_path:  String,
    #[serde(rename = "remotePath")]
    pub remote_path: String,
    #[serde(rename = "contentTypeOverride")]
    pub content_type_override: Option<String>,
    #[serde(rename = "cacheControl")]
    pub cache_control: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub headers: std::collections::HashMap<String, String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub metadata: std::collections::HashMap<String, String>,
    #[serde(default, rename = "retryMetadataFailure")]
    #[allow(dead_code)]
    pub retry_metadata_failure: bool,
    /// true인 경우에만 업로드 완료 후 CDN Purge를 수행한다.
    #[serde(default, rename = "isOverwrite")]
    pub is_overwrite: bool,
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
    #[serde(rename = "cdnInvalidationId")]
    cdn_invalidation_id: Option<String>,
    error:            Option<String>,
}

// ─── Commands ─────────────────────────────────────────────────────────────────

/// 파일/폴더 경로 목록을 `(절대경로, 상대경로)` 파일 목록으로 확장.
/// 폴더는 재귀 탐색하여 `폴더명/하위경로` 형태의 상대경로를 반환한다.
async fn expand_paths_to_files(paths: &[String]) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for path_str in paths {
        let p = Path::new(path_str);
        match tokio::fs::metadata(p).await {
            Ok(meta) if meta.is_dir() => {
                let folder_name = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                if let Ok(files) = collect_local_files(p).await {
                    for (rel, abs_path) in files {
                        let full_rel = if folder_name.is_empty() {
                            rel
                        } else {
                            format!("{}/{}", folder_name, rel)
                        };
                        result.push((abs_path.to_string_lossy().into_owned(), full_rel));
                    }
                }
            }
            Ok(_) => {
                let file_name = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                if !file_name.is_empty() {
                    result.push((path_str.clone(), file_name));
                }
            }
            Err(e) => {
                tracing::warn!("경로 접근 실패, 건너뜀: {} ({})", path_str, e);
            }
        }
    }
    result
}

/// 지정된 로컬 파일/폴더 목록과 S3 ETag를 비교해 업로드/스킵/덮어쓰기 플랜 생성.
/// 폴더 경로가 포함된 경우 재귀적으로 파일을 확장하며, 상대 경로 구조를 S3 키에 보존한다.
#[tauri::command]
pub async fn build_sync_plan(
    profile_id:    String,
    local_paths:   Vec<String>,
    remote_prefix: String,
    store: State<'_, ProfileStore>,
    cache: State<'_, AdapterCache>,
) -> Result<SyncPlan, String> {
    let (creds, region, bucket, endpoint) = store
        .get_connection_info(&profile_id)
        .await
        .map_err(|e| e.to_string())?;

    let adapter = cache
        .get_or_create(&profile_id, || async {
            S3Adapter::new(&region, &bucket, &creds, endpoint.as_deref())
                .await
        })
        .await
        .map_err(|e| e.to_string())?;
    let profile = store
        .get_profile(&profile_id)
        .await
        .map_err(|e| e.to_string())?;
    let use_size_fallback = profile.multipart_etag_fallback;

    let mut plan = SyncPlan {
        to_upload:   vec![],
        to_skip:     vec![],
        to_overwrite: vec![],
        purge_targets: vec![],
        compare_mode: if use_size_fallback {
            "etagWithSizeFallback".to_string()
        } else {
            "etag".to_string()
        },
    };

    // 폴더를 재귀적으로 파일로 확장 — (절대경로, 상대경로) 목록
    let expanded = expand_paths_to_files(&local_paths).await;
    tracing::info!(
        "build_sync_plan: 입력 {}개 경로 → 확장 후 {}개 파일",
        local_paths.len(),
        expanded.len()
    );

    let mut tasks: JoinSet<
        Option<(String, String, String, u64, String, String, Option<(u64, Option<String>)>)>,
    > = JoinSet::new();

    for (local_path, rel_path) in expanded {
        let adapter = adapter.clone();
        let prefix  = remote_prefix.clone();

        tasks.spawn(async move {
            let p = Path::new(&local_path);
            let remote_key = format!("{}{}", prefix, rel_path);

            let metadata = tokio::fs::metadata(&local_path).await.ok()?;
            let size = metadata.len();
            let modified = metadata.modified().ok()?;
            let last_modified =
                chrono::DateTime::<chrono::Utc>::from(modified).to_rfc3339();

            let remote_meta = adapter
                .head_object_meta(&remote_key)
                .await
                .ok()
                .flatten()
                .map(|meta| (meta.size, meta.etag));

            // 10MB 이상이면 S3 멀티파트 ETag 형식("hash-N")으로 계산
            let local_etag = if size >= MULTIPART_THRESHOLD {
                hash::calculate_multipart_etag(p, PART_SIZE).await.ok()?
            } else {
                hash::compute_file_md5(&local_path).await.ok()?
            };

            Some((
                local_path,
                rel_path,   // FileItem.name: 폴더 포함 상대 경로
                remote_key,
                size,
                last_modified,
                local_etag,
                remote_meta,
            ))
        });
    }

    while let Some(Ok(Some((
        local_path,
        rel_path,
        remote_key,
        size,
        last_modified,
        local_etag,
        remote_meta,
    )))) = tasks.join_next().await
    {
        let item = FileItem {
            name:          rel_path,   // 폴더 포함 상대 경로 ("folder/sub/file.txt")
            path:          local_path,
            size,
            last_modified,
            is_directory:  false,
            etag:          Some(local_etag.clone()),
            content_type:  None,
        };

        let remote_etag = remote_meta.as_ref().and_then(|(_, etag)| etag.clone());
        let matches_with_fallback = use_size_fallback
            && size >= MULTIPART_THRESHOLD
            && remote_meta
                .as_ref()
                .map(|(remote_size, etag)| {
                    *remote_size == size
                        && etag
                            .as_ref()
                            .map(|value| value.contains('-'))
                            .unwrap_or(false)
                })
                .unwrap_or(false);

        match remote_etag {
            None => {
                if profile.purge_on_new_upload && profile.cdn_provider.is_some() {
                    plan.purge_targets.push(remote_key);
                }
                plan.to_upload.push(item)
            }
            Some(etag) if etag == local_etag || matches_with_fallback => {
                plan.to_skip.push(item)
            }
            Some(_) => {
                plan.purge_targets.push(remote_key);
                plan.to_overwrite.push(item)
            }
        }
    }

    Ok(plan)
}

/// 로컬 디렉터리 ↔ S3 prefix 비교 (크기 stat만 사용, MD5 계산 없음 — 미리보기 전용)
async fn compare_local_remote(
    profile_id:    &str,
    local_dir:     &str,
    remote_prefix: &str,
    store:         &ProfileStore,
    cache:         &AdapterCache,
) -> anyhow::Result<SyncResult> {
    let (creds, region, bucket, endpoint) =
        store.get_connection_info(profile_id).await?;

    let adapter = cache
        .get_or_create(profile_id, || async {
            S3Adapter::new(&region, &bucket, &creds, endpoint.as_deref())
                .await
        })
        .await?;
    let profile = store.get_profile(profile_id).await?;

    // 로컬 파일 목록 + S3 목록 병렬 수집
    use crate::adapters::storage::base::StorageAdapter;
    let (local_files, remote_files) = tokio::try_join!(
        collect_local_files(Path::new(local_dir)),
        adapter.list_objects(remote_prefix),
    )?;

    let mut remote_map: std::collections::HashMap<String, (u64, Option<String>)> =
        remote_files
            .into_iter()
            .map(|f| (f.key.clone(), (f.size, f.etag)))
            .collect();

    let mut result = SyncResult {
        new:           vec![],
        modified:      vec![],
        deleted:       vec![],
        unchanged:     vec![],
        purge_targets: vec![],
    };

    for (relative_path, abs_path) in &local_files {
        let remote_key = if remote_prefix.is_empty() {
            relative_path.clone()
        } else {
            format!(
                "{}{}",
                remote_prefix.trim_end_matches('/'),
                if relative_path.starts_with('/') {
                    relative_path.clone()
                } else {
                    format!("/{}", relative_path)
                }
            )
        };

        let local_size = tokio::fs::metadata(abs_path).await.map(|m| m.len()).unwrap_or(0);

        let entry = FileEntry {
            local_path:  Some(abs_path.to_string_lossy().into_owned()),
            remote_key:  remote_key.clone(),
            size:        local_size,
            local_md5:   None,
            remote_size: remote_map.get(&remote_key).map(|(s, _)| *s),
            remote_etag: remote_map.get(&remote_key).and_then(|(_, e)| e.clone()),
        };

        match remote_map.remove(&remote_key) {
            None => {
                if profile.purge_on_new_upload && profile.cdn_provider.is_some() {
                    result.purge_targets.push(remote_key);
                }
                result.new.push(entry);
            }
            Some((remote_size, _)) if remote_size != local_size => {
                result.purge_targets.push(remote_key);
                result.modified.push(entry);
            }
            Some(_) => {
                result.unchanged.push(entry);
            }
        }
    }

    for (remote_key, (size, etag)) in remote_map {
        result.deleted.push(FileEntry {
            local_path:  None,
            remote_key,
            size,
            local_md5:   None,
            remote_etag: etag,
            remote_size: Some(size),
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
    cache: State<'_, AdapterCache>,
) -> Result<SyncResult, String> {
    compare_local_remote(&profile_id, &local_dir, &remote_prefix, &*store, &*cache)
        .await
        .map_err(|e| e.to_string())
}

/// 업로드 실행 (병렬, 진행률 이벤트 emit, 완료 후 CDN Purge)
/// H-2: Semaphore로 동시 업로드 4개 제한.
/// H-6: CdnCredentials 기반 CDN Purge.
#[tauri::command]
pub async fn start_uploads(
    app:                    AppHandle,
    profile_id:             String,
    items:                  Vec<UploadItem>,
    cdn_distribution_id:    Option<String>,
    cdn_provider:           Option<String>,
    max_concurrent_files:   Option<usize>,
    store: State<'_, ProfileStore>,
    cache: State<'_, AdapterCache>,
) -> Result<(), String> {
    let (creds, region, bucket, endpoint) = store
        .get_connection_info(&profile_id)
        .await
        .map_err(|e| e.to_string())?;

    let adapter = cache
        .get_or_create(&profile_id, || async {
            S3Adapter::new(&region, &bucket, &creds, endpoint.as_deref())
                .await
        })
        .await
        .map_err(|e| e.to_string())?;

    // H-6: CDN 자격증명 사전 조회
    let cdn_info: Option<(String, crate::utils::config::CdnCredentials)> =
        match &cdn_provider {
            Some(prov) => store
                .get_cdn_credentials(&profile_id, prov)
                .await
                .ok()
                .map(|c| (cdn_distribution_id.clone().unwrap_or_default(), c)),
            None => None,
        };
    let cdn_info = Arc::new(cdn_info);

    let concurrent = max_concurrent_files.unwrap_or(MAX_CONCURRENT_FILES).clamp(1, 32);
    let semaphore = Arc::new(Semaphore::new(concurrent));
    let successful_purge_targets = Arc::new(Mutex::new(Vec::<(String, String)>::new()));
    let mut tasks: JoinSet<()> = JoinSet::new();

    for item in items {
        let adapter  = adapter.clone();
        let app      = app.clone();
        let successful_purge_targets = successful_purge_targets.clone();
        let permit   = semaphore.clone().acquire_owned().await.expect("Semaphore 오류");

        tasks.spawn(async move {
            let _permit = permit;
            let id    = item.id.clone();
            let app_p = app.clone();
            let id_p  = id.clone();
            let cancelled = Arc::new(AtomicBool::new(false));
            let done = Arc::new(AtomicBool::new(false));
            spawn_cancel_watcher(&app, &id, cancelled.clone(), done.clone()).await;

            // L-4: 전송 시작 시각 기록
            let start_time = std::time::Instant::now();

            let result = adapter
                .upload_with_options(
                    &item.local_path,
                    &item.remote_path,
                    item.content_type_override.as_deref(),
                    item.cache_control.as_deref(),
                    move |transferred, total| {
                        if cancelled.load(Ordering::Relaxed) {
                            return false;
                        }
                        let progress = if total > 0 {
                            (transferred * 100 / total) as u8
                        } else {
                            0
                        };
                        let elapsed = start_time.elapsed().as_secs_f64();
                        let speed = if elapsed > 0.05 {
                            (transferred as f64 / elapsed) as u64
                        } else {
                            0
                        };
                        let _ = app_p.emit(
                            "transfer:progress",
                            TransferProgressPayload {
                                id:                id_p.clone(),
                                progress,
                                transferred_bytes: transferred,
                                speed,
                                status:            "uploading".into(),
                            },
                        );
                        true
                    },
                )
                .await;

            let (status, error) = match &result {
                Ok(_)  => ("complete".to_string(), None),
                Err(e) if e.to_string().contains("취소") => ("canceled".to_string(), None),
                Err(e) => ("error".to_string(), Some(e.to_string())),
            };

            if result.is_ok() && item.is_overwrite {
                successful_purge_targets
                    .lock()
                    .await
                    .push((id.clone(), item.remote_path.clone()));
            }

            let _ = app.emit(
                "transfer:complete",
                TransferCompletePayload {
                    id: id.clone(),
                    status,
                    cdn_purged: false,
                    cdn_purge_error: None,
                    cdn_invalidation_id: None,
                    error,
                },
            );
            done.store(true, Ordering::Relaxed);
            app.state::<TransferControl>().clear(&id).await;
        });
    }

    while tasks.join_next().await.is_some() {}

    let targets = successful_purge_targets.lock().await.clone();
    if let Some((distribution_id, cdn_creds)) = cdn_info.as_ref() {
        for batch in targets.chunks(MAX_CDN_PURGE_PATHS_PER_REQUEST) {
            let ids: Vec<String> = batch.iter().map(|(id, _)| id.clone()).collect();
            let paths: Vec<String> = batch.iter().map(|(_, path)| path.clone()).collect();
            let purge_result = crate::adapters::cdn::purge_with_credentials(
                distribution_id,
                &paths,
                cdn_creds.clone(),
            )
            .await;
            let (cdn_purged, cdn_purge_error, cdn_invalidation_id) = match purge_result {
                Ok(id) => (true, None, id),
                Err(err) => (false, Some(err.to_string()), None),
            };

            for id in ids {
                let _ = app.emit(
                    "transfer:complete",
                    TransferCompletePayload {
                        id,
                        status: "complete".to_string(),
                        cdn_purged,
                        cdn_purge_error: cdn_purge_error.clone(),
                        cdn_invalidation_id: cdn_invalidation_id.clone(),
                        error: None,
                    },
                );
            }
        }
    }
    Ok(())
}

/// 다운로드 실행 (병렬, 진행률 이벤트 emit)
#[tauri::command]
pub async fn start_downloads(
    app:                  AppHandle,
    profile_id:           String,
    items:                Vec<DownloadItem>,
    max_concurrent_files: Option<usize>,
    store: State<'_, ProfileStore>,
    cache: State<'_, AdapterCache>,
) -> Result<(), String> {
    let (creds, region, bucket, endpoint) = store
        .get_connection_info(&profile_id)
        .await
        .map_err(|e| e.to_string())?;

    let adapter = cache
        .get_or_create(&profile_id, || async {
            S3Adapter::new(&region, &bucket, &creds, endpoint.as_deref())
                .await
        })
        .await
        .map_err(|e| e.to_string())?;

    let concurrent = max_concurrent_files.unwrap_or(MAX_CONCURRENT_FILES).clamp(1, 32);
    let semaphore = Arc::new(Semaphore::new(concurrent));
    let mut tasks: JoinSet<()> = JoinSet::new();

    for item in items {
        let adapter = adapter.clone();
        let app     = app.clone();
        let permit  = semaphore.clone().acquire_owned().await.expect("Semaphore 오류");

        tasks.spawn(async move {
            let _permit = permit;
            let id    = item.id.clone();
            let app_p = app.clone();
            let id_p  = id.clone();
            let cancelled = Arc::new(AtomicBool::new(false));
            let done = Arc::new(AtomicBool::new(false));
            spawn_cancel_watcher(&app, &id, cancelled.clone(), done.clone()).await;

            // L-4: 전송 시작 시각 기록
            let start_time = std::time::Instant::now();

            let result = adapter
                .download_with_cancel(
                    &item.remote_path,
                    &item.local_path,
                    || cancelled.load(Ordering::Relaxed),
                    move |transferred, total| {
                        let progress = if total > 0 {
                            (transferred * 100 / total) as u8
                        } else {
                            50
                        };
                        let elapsed = start_time.elapsed().as_secs_f64();
                        let speed = if elapsed > 0.05 {
                            (transferred as f64 / elapsed) as u64
                        } else {
                            0
                        };
                        let _ = app_p.emit(
                            "transfer:progress",
                            TransferProgressPayload {
                                id:                id_p.clone(),
                                progress,
                                transferred_bytes: transferred,
                                speed,
                                status:            "downloading".into(),
                            },
                        );
                    },
                )
                .await;

            let (status, error) = match result {
                Ok(_)  => ("complete".to_string(), None),
                Err(e) if e.to_string().contains("취소") => ("canceled".to_string(), None),
                Err(e) => ("error".to_string(), Some(e.to_string())),
            };

            let _ = app.emit(
                "transfer:complete",
                TransferCompletePayload {
                    id: id.clone(),
                    status,
                    cdn_purged:      false,
                    cdn_purge_error: None,
                    cdn_invalidation_id: None,
                    error,
                },
            );
            done.store(true, Ordering::Relaxed);
            app.state::<TransferControl>().clear(&id).await;
        });
    }

    while tasks.join_next().await.is_some() {}
    Ok(())
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

async fn spawn_cancel_watcher(
    app: &AppHandle,
    id: &str,
    cancelled: Arc<AtomicBool>,
    done: Arc<AtomicBool>,
) {
    app.state::<TransferControl>().clear(id).await;
    let app_cancel = app.clone();
    let id_cancel = id.to_owned();
    tokio::spawn(async move {
        loop {
            if done.load(Ordering::Relaxed) {
                break;
            }
            if app_cancel
                .state::<TransferControl>()
                .is_cancelled(&id_cancel)
                .await
            {
                cancelled.store(true, Ordering::Relaxed);
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });
}

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

            // M-9: symlink_metadata()로 심볼릭 링크 감지 — 순환 링크 방지를 위해 기본 제외
            let sym_meta = tokio::fs::symlink_metadata(&path).await
                .map_err(|e| anyhow::anyhow!("심링크 메타데이터 읽기 실패: {}", e))?;
            if sym_meta.file_type().is_symlink() {
                tracing::debug!("심볼릭 링크 건너뜀: {}", path.display());
                continue;
            }

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
