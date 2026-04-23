use serde::{Deserialize, Serialize};
use tauri::State;
use crate::adapters::storage::s3::S3Adapter;
use crate::utils::config::{ProfileStore, ProfileConfig};

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

// ─── Profile Commands ─────────────────────────────────────────────────────────

#[tauri::command]
pub async fn load_profiles(store: State<'_, ProfileStore>) -> Result<Vec<ProfileConfig>, String> {
    store.load_all().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_profile(profile: ProfileConfig, store: State<'_, ProfileStore>) -> Result<(), String> {
    store.save(profile).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_profile(id: String, store: State<'_, ProfileStore>) -> Result<(), String> {
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

    let result = adapter.list_objects(&prefix).await.map_err(|e| e.to_string())?;

    Ok(S3ListResponse {
        files: result.files,
        next_continuation_token: result.next_token,
        is_truncated: result.is_truncated,
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
    profile_id: String,
    key: String,
    content: Vec<u8>,
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
    profile_id: String,
    key: String,
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
