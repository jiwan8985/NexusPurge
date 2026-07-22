use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::adapters::storage::s3::S3Adapter;
use crate::utils::adapter_cache::AdapterCache;
use crate::utils::config::{AppSettings, AwsCredentials, ProfileConfig, ProfileStore};
use crate::utils::crypto;
use crate::utils::transfer_control::TransferControl;

// ─── 동시 파일 전송 상한 (H-2) ───────────────────────────────────────────────
const MAX_CONCURRENT_FILES: usize = 8;

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
    #[serde(rename = "cdnInvalidationId")]
    cdn_invalidation_id: Option<String>,
    error:           Option<String>,
}

#[derive(Debug, Serialize)]
pub struct S3ConnectionTestResult {
    pub success: bool,
    pub warnings: Vec<String>,
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
    cache: State<'_, AdapterCache>,
) -> Result<(), String> {
    // L-2: 자격증명이 바뀔 수 있으므로 캐시 무효화
    cache.invalidate(&profile.id).await;
    store.save(profile).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_profile(
    id: String,
    store: State<'_, ProfileStore>,
    cache: State<'_, AdapterCache>,
) -> Result<(), String> {
    cache.invalidate(&id).await;
    store.delete(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cancel_transfer(id: String, control: State<'_, TransferControl>) -> Result<(), String> {
    control.cancel(&id).await;
    Ok(())
}

// ─── Connection ───────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn connect_s3(
    profile_id: String,
    store: State<'_, ProfileStore>,
) -> Result<S3ConnectionTestResult, String> {
    let (creds, region, bucket, endpoint) = store
        .get_connection_info(&profile_id)
        .await
        .map_err(|e| e.to_string())?;
    let profile = store
        .get_profile(&profile_id)
        .await
        .map_err(|e| e.to_string())?;
    let base_prefix = profile.base_prefix.as_deref().unwrap_or("");

    let warnings = S3Adapter::new(&region, &bucket, &creds, endpoint.as_deref())
        .await
        .map_err(|e| e.to_string())?
        .verify_access(base_prefix)
        .await
        .map_err(|e| e.to_string())?;

    Ok(S3ConnectionTestResult { success: true, warnings })
}

/// H-3: 프로필 저장 없이 입력값으로 직접 연결 테스트
#[tauri::command]
pub async fn test_s3_connection(
    region:     String,
    bucket:     String,
    base_prefix: Option<String>,
    access_key: String,
    secret_key: String,
    endpoint:   Option<String>,
) -> Result<S3ConnectionTestResult, String> {
    let creds = AwsCredentials {
        access_key_id: access_key.trim().to_owned(),
        secret_access_key: secret_key.trim().to_owned(),
    };
    if creds.access_key_id.is_empty() {
        return Err("Access Key ID is required".to_string());
    }
    if creds.secret_access_key.is_empty() {
        return Err("Secret Access Key is required".to_string());
    }
    let region = region.trim().to_owned();
    let bucket = bucket.trim().to_owned();
    let endpoint = endpoint
        .map(|value| value.trim().trim_end_matches('/').to_owned())
        .filter(|value| !value.is_empty());
    let warnings = S3Adapter::new(&region, &bucket, &creds, endpoint.as_deref())
        .await
        .map_err(|e| e.to_string())?
        .verify_access(base_prefix.as_deref().unwrap_or(""))
        .await
        .map_err(|e| e.to_string())?;

    Ok(S3ConnectionTestResult { success: true, warnings })
}

// ─── S3 Object Operations ─────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_s3_objects(
    profile_id: String,
    prefix: String,
    store: State<'_, ProfileStore>,
    cache: State<'_, AdapterCache>,
) -> Result<S3ListResponse, String> {
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

/// 폴더 다운로드용: prefix 하위 전체 객체 키 조회 (재귀)
#[tauri::command]
pub async fn list_s3_keys(
    profile_id: String,
    prefix: String,
    store: State<'_, ProfileStore>,
    cache: State<'_, AdapterCache>,
) -> Result<Vec<String>, String> {
    let (creds, region, bucket, endpoint) = store
        .get_connection_info(&profile_id)
        .await
        .map_err(|e| e.to_string())?;

    cache
        .get_or_create(&profile_id, || async {
            S3Adapter::new(&region, &bucket, &creds, endpoint.as_deref())
                .await
        })
        .await
        .map_err(|e| e.to_string())?
        .list_keys_recursive(&prefix)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_s3_objects(
    profile_id: String,
    keys: Vec<String>,
    store: State<'_, ProfileStore>,
    cache: State<'_, AdapterCache>,
) -> Result<Vec<String>, String> {
    let (creds, region, bucket, endpoint) = store
        .get_connection_info(&profile_id)
        .await
        .map_err(|e| e.to_string())?;

    cache
        .get_or_create(&profile_id, || async {
            S3Adapter::new(&region, &bucket, &creds, endpoint.as_deref())
                .await
        })
        .await
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
    cache: State<'_, AdapterCache>,
) -> Result<(), String> {
    crate::utils::validate::validate_s3_key(&key)?;

    let (creds, region, bucket, endpoint) = store
        .get_connection_info(&profile_id)
        .await
        .map_err(|e| e.to_string())?;

    cache
        .get_or_create(&profile_id, || async {
            S3Adapter::new(&region, &bucket, &creds, endpoint.as_deref())
                .await
        })
        .await
        .map_err(|e| e.to_string())?
        .put_object(&key, content, &content_type)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_presigned_url(
    profile_id:         String,
    key:                String,
    expires_in_seconds: u64,
    store: State<'_, ProfileStore>,
    cache: State<'_, AdapterCache>,
) -> Result<String, String> {
    let (creds, region, bucket, endpoint) = store
        .get_connection_info(&profile_id)
        .await
        .map_err(|e| e.to_string())?;

    cache
        .get_or_create(&profile_id, || async {
            S3Adapter::new(&region, &bucket, &creds, endpoint.as_deref())
                .await
        })
        .await
        .map_err(|e| e.to_string())?
        .presign_get(&key, expires_in_seconds)
        .await
        .map_err(|e| e.to_string())
}

/// 속성(우클릭) 다이얼로그 — S3 객체의 전체 HeadObject 응답(상세 헤더)을 반환.
/// 고객사 요청: 크롬 개발자모드 Network 탭에서 보는 수준의 상세 정보 제공.
#[tauri::command]
pub async fn get_s3_object_detail(
    profile_id: String,
    key:        String,
    store: State<'_, ProfileStore>,
    cache: State<'_, AdapterCache>,
) -> Result<Option<crate::adapters::storage::base::S3ObjectDetail>, String> {
    let (creds, region, bucket, endpoint) = store
        .get_connection_info(&profile_id)
        .await
        .map_err(|e| e.to_string())?;

    cache
        .get_or_create(&profile_id, || async {
            S3Adapter::new(&region, &bucket, &creds, endpoint.as_deref())
                .await
        })
        .await
        .map_err(|e| e.to_string())?
        .head_object_full(&key)
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
    cache: State<'_, AdapterCache>,
) -> Result<(), String> {
    // 사용자가 새로 정하는 이름만 검증 (old_key는 기존 객체)
    crate::utils::validate::validate_s3_key(&new_key)?;

    let (creds, region, bucket, endpoint) = store
        .get_connection_info(&profile_id)
        .await
        .map_err(|e| e.to_string())?;

    cache
        .get_or_create(&profile_id, || async {
            S3Adapter::new(&region, &bucket, &creds, endpoint.as_deref())
                .await
        })
        .await
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
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                return None;
            }
            // M-9: symlink_metadata()로 심볼릭 링크 감지 — 링크는 목록에서 제외
            let sym_meta = std::fs::symlink_metadata(&path).ok()?;
            if sym_meta.file_type().is_symlink() {
                return None;
            }
            let meta = entry.metadata().ok()?;
            let last_modified = meta
                .modified()
                .ok()
                .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339())
                .unwrap_or_default();
            Some(FileItem {
                name,
                path: path.to_string_lossy().into_owned(),
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

/// 앱 전역 설정 조회 (감사 로그 상세 레벨 등) — SettingsModal 마운트 시 1회 호출
#[tauri::command]
pub async fn get_app_settings(store: State<'_, ProfileStore>) -> Result<AppSettings, String> {
    store.get_app_settings().await.map_err(|e| e.to_string())
}

/// CDN API 감사 로그 상세 레벨(응답 본문 포함 여부) 저장 — 즉시 audit_level 전역 상태에도 반영
#[tauri::command]
pub async fn save_detailed_audit_log(
    enabled: bool,
    store: State<'_, ProfileStore>,
) -> Result<(), String> {
    store
        .save_detailed_audit_log(enabled)
        .await
        .map_err(|e| e.to_string())?;
    crate::utils::audit_level::set(enabled);
    Ok(())
}

// ─── Upload (H-2 Semaphore + H-6 CdnCredentials) ─────────────────────────────

/// 파일 업로드 — is_overwrite == true 인 항목만 CDN Purge 트리거.
/// H-2: Semaphore로 동시 업로드 4개 제한.
/// H-6: CdnCredentials 기반 CDN Purge (Akamai 지원).
#[tauri::command]
pub async fn upload_files(
    app:                  AppHandle,
    profile_id:           String,
    items:                Vec<UploadFileItem>,
    cdn_distribution_id:  Option<String>,
    cdn_provider:         Option<String>,
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

    // H-6: CDN 자격증명 사전 조회 (태스크 외부에서 한 번만 실행)
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
    let mut tasks: JoinSet<()> = JoinSet::new();

    for item in items {
        let adapter   = adapter.clone();
        let app       = app.clone();
        let cdn_info  = cdn_info.clone();
        let permit    = semaphore.clone().acquire_owned().await.expect("Semaphore 오류");

        tasks.spawn(async move {
            let _permit = permit;
            let id    = item.id.clone();
            let app_p = app.clone();
            let id_p  = id.clone();
            let cancelled = Arc::new(AtomicBool::new(false));
            let done = Arc::new(AtomicBool::new(false));
            let cancel_probe = cancelled.clone();
            let done_probe = done.clone();
            let app_cancel = app.clone();
            let id_cancel = id.clone();

            app.state::<TransferControl>().clear(&id).await;
            tokio::spawn(async move {
                loop {
                    if done_probe.load(Ordering::Relaxed) {
                        break;
                    }
                    if app_cancel
                        .state::<TransferControl>()
                        .is_cancelled(&id_cancel)
                        .await
                    {
                        cancel_probe.store(true, Ordering::Relaxed);
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            });

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
                        let progress = if total > 0 { (transferred * 100 / total) as u8 } else { 0 };
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

            // is_overwrite == true 인 경우에만 CDN Purge 실행 (C-1)
            let (cdn_purged, cdn_purge_error, cdn_invalidation_id) =
                if result.is_ok() && item.is_overwrite {
                    if let Some((dist, cdn_creds)) = cdn_info.as_ref() {
                        match crate::adapters::cdn::purge_with_credentials(
                            dist,
                            &[item.remote_path.clone()],
                            cdn_creds.clone(),
                        )
                        .await
                        {
                            Ok(id)  => (true, None, id),
                            Err(e) => (false, Some(e.to_string()), None),
                        }
                    } else {
                        (false, None, None)
                    }
                } else {
                    (false, None, None)
                };

            let _ = app.emit(
                "transfer:complete",
                TransferCompletePayload {
                    id: id.clone(),
                    status,
                    cdn_purged,
                    cdn_purge_error,
                    cdn_invalidation_id,
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

// ───  프로필 파일(JSON) Import ─────────────────────────────────────────

/// 테스트 프로필 파일 형식 (profile-sample.json 참고)
#[derive(Debug, Deserialize)]
pub struct ProfileFile {
    pub name: String,
    pub region: String,
    pub bucket: String,
    #[serde(default, rename = "basePrefix")]
    pub base_prefix: Option<String>,
    #[serde(rename = "accessKeyId")]
    pub access_key_id: String,
    #[serde(rename = "secretAccessKey")]
    pub secret_access_key: String,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default, rename = "cdnBasePath")]
    pub cdn_base_path: Option<String>,
    /// 연결 직후 기본 선택될 CDN (미지정 시 cdns의 첫 항목)
    #[serde(default, rename = "defaultCdn")]
    pub default_cdn: Option<String>,
    #[serde(default)]
    pub cdns: ProfileFileCdns,
}

#[derive(Debug, Default, Deserialize)]
pub struct ProfileFileCdns {
    #[serde(default)]
    pub cloudfront: Option<ProfileFileCloudfront>,
    #[serde(default)]
    pub akamai: Option<ProfileFileAkamai>,
    #[serde(default)]
    pub kt: Option<ProfileFileSolbox>,
    #[serde(default)]
    pub lguplus: Option<ProfileFileSolbox>,
    #[serde(default)]
    pub hyosung: Option<ProfileFileHyosung>,
}

#[derive(Debug, Deserialize)]
pub struct ProfileFileHyosung {
    /// X-ITX-Security-Principal
    #[serde(rename = "apiKey")]
    pub api_key: String,
    /// X-ITX-Security-Secret
    #[serde(rename = "apiSecret")]
    pub api_secret: String,
    /// CDN 서비스 ID (예: TID_18656)
    #[serde(rename = "serviceId")]
    pub service_id: String,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub domain: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProfileFileCloudfront {
    #[serde(rename = "distributionId")]
    pub distribution_id: String,
    #[serde(default)]
    pub domain: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProfileFileAkamai {
    pub host: String,
    #[serde(rename = "clientToken")]
    pub client_token: String,
    #[serde(rename = "clientSecret")]
    pub client_secret: String,
    #[serde(rename = "accessToken")]
    pub access_token: String,
    #[serde(default, rename = "cpCode")]
    pub cp_code: Option<String>,
    #[serde(default)]
    pub domain: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProfileFileSolbox {
    pub username: String,
    pub password: String,
    #[serde(rename = "serviceName")]
    pub service_name: String,
    #[serde(default, rename = "volumeName")]
    pub volume_name: Option<String>,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub domain: Option<String>,
    /// "cloudcdn" | "volume" — cloudcdn이면 전체 Purge 시 Purge by Service 사용
    #[serde(default, rename = "serviceType")]
    pub service_type: Option<String>,
}

/// JSON 프로필 파일을 가져와 저장한다.
/// 하나의 파일에 여러 CDN(cloudfront/akamai/kt/lguplus)을 담을 수 있으며,
/// 시크릿(S3 secret, CDN password 등)은 저장 시 OS keyring으로 이동한다.
#[tauri::command]
pub async fn import_profile_file(
    content: String,
    store: State<'_, ProfileStore>,
    cache: State<'_, AdapterCache>,
) -> Result<ProfileConfig, String> {
    let file: ProfileFile = serde_json::from_str(&content)
        .map_err(|e| format!("프로필 파일 파싱 실패: {}", e))?;

    let mut cdn_providers: Vec<crate::utils::config::CdnProviderConfig> = Vec::new();
    let mut profile = ProfileConfig {
        id: uuid::Uuid::new_v4().to_string(),
        name: file.name,
        scope: None,
        permissions: None,
        region: file.region,
        bucket: file.bucket,
        base_prefix: file.base_prefix,
        access_key_id: Some(file.access_key_id),
        secret_access_key: Some(file.secret_access_key),
        endpoint: file.endpoint,
        cdn_provider: None,
        cdn_providers: Vec::new(),
        cdn_distribution_id: None,
        cdn_domain: None,
        cdn_base_path: file.cdn_base_path,
        purge_on_new_upload: false,
        purge_policy: None,
        upload_policy: None,
        metadata_policy: None,
        log_shipping: None,
        auth_binding: None,
        default_cache_control: None,
        content_type_override: None,
        multipart_etag_fallback: false,
        akamai_client_token: None,
        akamai_client_secret: None,
        akamai_access_token: None,
        akamai_host: None,
        akamai_cp_code: None,
        lguplus_username: None,
        lguplus_password: None,
        lguplus_service_name: None,
        lguplus_volume_name: None,
        lguplus_endpoint: None,
        lguplus_service_type: None,
        kt_username: None,
        kt_password: None,
        kt_service_name: None,
        kt_volume_name: None,
        kt_endpoint: None,
        kt_service_type: None,
        hyosung_api_key: None,
        hyosung_api_secret: None,
        hyosung_endpoint: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    if let Some(cf) = file.cdns.cloudfront {
        profile.cdn_distribution_id = Some(cf.distribution_id.clone());
        cdn_providers.push(crate::utils::config::CdnProviderConfig {
            provider: "cloudfront".into(),
            display_name: None,
            enabled: true,
            distribution_id: Some(cf.distribution_id),
            domain: cf.domain,
        });
    }
    if let Some(ak) = file.cdns.akamai {
        profile.akamai_host = Some(ak.host);
        profile.akamai_client_token = Some(ak.client_token);
        profile.akamai_client_secret = Some(ak.client_secret);
        profile.akamai_access_token = Some(ak.access_token);
        profile.akamai_cp_code = ak.cp_code;
        cdn_providers.push(crate::utils::config::CdnProviderConfig {
            provider: "akamai".into(),
            display_name: None,
            enabled: true,
            distribution_id: None,
            domain: ak.domain,
        });
    }
    if let Some(kt) = file.cdns.kt {
        profile.kt_username = Some(kt.username);
        profile.kt_password = Some(kt.password);
        profile.kt_service_name = Some(kt.service_name);
        profile.kt_volume_name = kt.volume_name;
        profile.kt_endpoint = kt.endpoint;
        profile.kt_service_type = kt.service_type;
        cdn_providers.push(crate::utils::config::CdnProviderConfig {
            provider: "kt".into(),
            display_name: None,
            enabled: true,
            distribution_id: None,
            domain: kt.domain,
        });
    }
    if let Some(lgu) = file.cdns.lguplus {
        profile.lguplus_username = Some(lgu.username);
        profile.lguplus_password = Some(lgu.password);
        profile.lguplus_service_name = Some(lgu.service_name);
        profile.lguplus_volume_name = lgu.volume_name;
        profile.lguplus_endpoint = lgu.endpoint;
        profile.lguplus_service_type = lgu.service_type;
        cdn_providers.push(crate::utils::config::CdnProviderConfig {
            provider: "lguplus".into(),
            display_name: None,
            enabled: true,
            distribution_id: None,
            domain: lgu.domain,
        });
    }
    if let Some(hyosung) = file.cdns.hyosung {
        profile.hyosung_api_key = Some(hyosung.api_key);
        profile.hyosung_api_secret = Some(hyosung.api_secret);
        profile.hyosung_endpoint = hyosung.endpoint;
        // Service ID는 provider별 distribution_id 슬롯에 보관 (CloudFront와 공존 가능)
        if profile.cdn_distribution_id.is_none() {
            profile.cdn_distribution_id = Some(hyosung.service_id.clone());
        }
        cdn_providers.push(crate::utils::config::CdnProviderConfig {
            provider: "hyosung".into(),
            display_name: None,
            enabled: true,
            distribution_id: Some(hyosung.service_id),
            domain: hyosung.domain,
        });
    }

    // 기본 CDN: defaultCdn 지정값이 목록에 있으면 사용, 없으면 첫 항목
    let default_cdn = file
        .default_cdn
        .filter(|d| cdn_providers.iter().any(|c| &c.provider == d))
        .or_else(|| cdn_providers.first().map(|c| c.provider.clone()));
    if let Some(default) = &default_cdn {
        profile.cdn_domain = cdn_providers
            .iter()
            .find(|c| &c.provider == default)
            .and_then(|c| c.domain.clone());
    }
    profile.cdn_provider = default_cdn;
    profile.cdn_providers = cdn_providers;

    cache.invalidate(&profile.id).await;
    store.save(profile.clone()).await.map_err(|e| e.to_string())?;
    // 응답에서 시크릿·인증 관련 값 전부 제거 (keyring 저장 완료 — 프론트엔드로 평문 전달 금지)
    profile.secret_access_key = None;
    profile.akamai_client_secret = None;
    profile.kt_password = None;
    profile.lguplus_password = None;
    profile.access_key_id = None;
    profile.akamai_client_token = None;
    profile.akamai_access_token = None;
    profile.hyosung_api_key = None;
    profile.lguplus_username = None;
    profile.kt_username = None;
    Ok(profile)
}

// ─── 암호화 프로필 Export / Import ───────────────────────────────────────────

/// 저장된 프로필(Keyring 시크릿 포함)을 AES-256-GCM 암호화 파일로 내보낸다.
/// 반환값은 JSON 문자열로, 프론트엔드에서 .nexprofile 파일로 저장한다.
#[tauri::command]
pub async fn export_encrypted_profile(
    profile_id: String,
    passphrase: String,
    store: State<'_, ProfileStore>,
) -> Result<String, String> {
    if passphrase.trim().is_empty() {
        return Err("패스프레이즈는 비워둘 수 없습니다".to_string());
    }

    // Keyring에서 S3 secret 주입
    let mut profile = store
        .get_profile(&profile_id)
        .await
        .map_err(|e| e.to_string())?;
    let creds = store
        .get_credentials(&profile_id)
        .await
        .map_err(|e| e.to_string())?;
    profile.secret_access_key = Some(creds.secret_access_key);

    // CDN secret들도 Keyring에서 주입
    use keyring::Entry;
    const SVC: &str = "cdn-upload-tool";
    for (suffix, field) in [
        ("_akamai",  &mut profile.akamai_client_secret as &mut Option<String>),
        ("_lguplus", &mut profile.lguplus_password),
        ("_hyosung", &mut profile.hyosung_api_secret),
        ("_kt",      &mut profile.kt_password),
        // 인증 관련이지만 secret은 아닌 값들(Access Key ID, 토큰, 계정명)도
        // profiles.json이 아닌 keyring에만 있으므로 내보내기 시 여기서 주입해야 함
        ("_access_key_id",       &mut profile.access_key_id),
        ("_akamai_client_token", &mut profile.akamai_client_token),
        ("_akamai_access_token", &mut profile.akamai_access_token),
        ("_hyosung_api_key",     &mut profile.hyosung_api_key),
        ("_lguplus_username",    &mut profile.lguplus_username),
        ("_kt_username",         &mut profile.kt_username),
    ] {
        let key = format!("{}{}", profile_id, suffix);
        if let Ok(entry) = Entry::new(SVC, &key) {
            if let Ok(secret) = entry.get_password() {
                *field = Some(secret);
            }
        }
    }

    let json = serde_json::to_string(&profile).map_err(|e| e.to_string())?;
    crypto::encrypt(json.as_bytes(), &passphrase).map_err(|e| e.to_string())
}

/// 암호화된 .nexprofile 파일 내용을 복호화하여 ProfileStore에 저장한다.
#[tauri::command]
pub async fn import_encrypted_profile(
    encrypted_data: String,
    passphrase: String,
    store: State<'_, ProfileStore>,
    cache: State<'_, AdapterCache>,
) -> Result<ProfileConfig, String> {
    if passphrase.trim().is_empty() {
        return Err("패스프레이즈는 비워둘 수 없습니다".to_string());
    }

    let decrypted = crypto::decrypt(&encrypted_data, &passphrase).map_err(|e| e.to_string())?;
    let mut profile: ProfileConfig =
        serde_json::from_slice(&decrypted).map_err(|e| format!("프로필 파싱 실패: {}", e))?;

    // 기존 프로필이 있다면 캐시 무효화
    cache.invalidate(&profile.id).await;
    store.save(profile.clone()).await.map_err(|e| e.to_string())?;
    // 응답에서 시크릿·인증 관련 값 전부 제거 (keyring 저장 완료 — 프론트엔드는 이 값을 쓰지 않으며,
    // 복호화된 평문이 IPC를 거쳐 렌더러 프로세스 메모리에 남을 이유가 없다)
    profile.secret_access_key = None;
    profile.akamai_client_secret = None;
    profile.kt_password = None;
    profile.lguplus_password = None;
    profile.access_key_id = None;
    profile.akamai_client_token = None;
    profile.akamai_access_token = None;
    profile.hyosung_api_key = None;
    profile.lguplus_username = None;
    profile.kt_username = None;
    Ok(profile)
}
