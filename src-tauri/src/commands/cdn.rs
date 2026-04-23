use serde::Serialize;
use tauri::State;
use crate::adapters::cdn;
use crate::utils::config::ProfileStore;

#[derive(Debug, Serialize)]
pub struct CdnPurgeResult {
    pub success: bool,
    pub provider: String,
    #[serde(rename = "invalidationId")]
    pub invalidation_id: Option<String>,
    pub paths: Vec<String>,
    #[serde(rename = "purgedAt")]
    pub purged_at: Option<String>,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn purge_cloudfront(
    profile_id: String,
    distribution_id: String,
    paths: Vec<String>,
    store: State<'_, ProfileStore>,
) -> Result<CdnPurgeResult, String> {
    let creds = store.get_credentials(&profile_id).await.map_err(|e| e.to_string())?;

    let result = crate::adapters::cdn::cloudfront::CloudFrontAdapter::new(creds)
        .map_err(|e| e.to_string())?
        .create_invalidation(&distribution_id, &paths)
        .await;

    match result {
        Ok(id) => Ok(CdnPurgeResult {
            success: true,
            provider: "cloudfront".into(),
            invalidation_id: Some(id),
            paths,
            purged_at: Some(chrono::Utc::now().to_rfc3339()),
            error: None,
        }),
        Err(e) => Ok(CdnPurgeResult {
            success: false,
            provider: "cloudfront".into(),
            invalidation_id: None,
            paths,
            purged_at: None,
            error: Some(e.to_string()),
        }),
    }
}

#[tauri::command]
pub async fn purge_cdn(
    profile_id: String,
    provider: String,
    distribution_id: String,
    paths: Vec<String>,
    store: State<'_, ProfileStore>,
) -> Result<CdnPurgeResult, String> {
    let creds = store.get_credentials(&profile_id).await.map_err(|e| e.to_string())?;

    let result = cdn::purge_with_credentials(&provider, &distribution_id, &paths, creds).await;

    match result {
        Ok(id) => Ok(CdnPurgeResult {
            success: true,
            provider,
            invalidation_id: id,
            paths,
            purged_at: Some(chrono::Utc::now().to_rfc3339()),
            error: None,
        }),
        Err(e) => Ok(CdnPurgeResult {
            success: false,
            provider,
            invalidation_id: None,
            paths,
            purged_at: None,
            error: Some(e.to_string()),
        }),
    }
}
