use crate::adapters::cdn;
use crate::utils::config::CdnCredentials;
use crate::utils::config::ProfileStore;
use reqwest::header::{CACHE_CONTROL, ETAG, LAST_MODIFIED, RANGE};
use serde::Serialize;
use tauri::State;

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

#[derive(Debug, Serialize)]
pub struct CdnConnectionTestResult {
    pub success: bool,
    pub provider: String,
    pub domain: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CdnPurgeStatusResult {
    pub success: bool,
    pub provider: String,
    pub status: Option<String>,
    pub message: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CdnUrlCheck {
    pub url: String,
    pub ok: bool,
    #[serde(rename = "statusCode")]
    pub status_code: Option<u16>,
    pub etag: Option<String>,
    #[serde(rename = "lastModified")]
    pub last_modified: Option<String>,
    #[serde(rename = "cacheControl")]
    pub cache_control: Option<String>,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn purge_cloudfront(
    profile_id: String,
    distribution_id: String,
    paths: Vec<String>,
    store: State<'_, ProfileStore>,
) -> Result<CdnPurgeResult, String> {
    let creds = store
        .get_credentials(&profile_id)
        .await
        .map_err(|e| e.to_string())?;

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
pub async fn test_cdn_connection(
    profile_id: String,
    provider: String,
    distribution_id: String,
    store: State<'_, ProfileStore>,
) -> Result<CdnConnectionTestResult, String> {
    let result = async {
        let cdn_creds = store
            .get_cdn_credentials(&profile_id, &provider)
            .await
            .map_err(|e| e.to_string())?;

        match cdn_creds {
            CdnCredentials::CloudFront(creds) => {
                if distribution_id.trim().is_empty() {
                    return Err("CloudFront Distribution ID is required".to_string());
                }
                let adapter =
                    cdn::cloudfront::CloudFrontAdapter::new(creds).map_err(|e| e.to_string())?;
                let domain = adapter
                    .get_distribution_domain(&distribution_id)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(Some(domain))
            }
            CdnCredentials::Akamai {
                client_token,
                client_secret,
                access_token,
                host,
                cdn_domain,
            } => {
                if cdn_domain.trim().is_empty() {
                    return Err("Akamai CDN domain is required".to_string());
                }
                let adapter = cdn::akamai::AkamaiAdapter::new(
                    client_token,
                    client_secret,
                    access_token,
                    host,
                )
                .map_err(|e| e.to_string())?;
                adapter
                    .test_fast_purge_access()
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(Some(cdn_domain))
            }
            CdnCredentials::Lguplus { .. } => Err(
                "LG U+ CDN purge API is not implemented yet. API specification is required."
                    .to_string(),
            ),
            CdnCredentials::Hyosung { .. } => Err(
                "Hyosung CDN purge API is not implemented yet. API specification is required."
                    .to_string(),
            ),
        }
    }
    .await;

    match result {
        Ok(domain) => Ok(CdnConnectionTestResult {
            success: true,
            provider,
            domain,
            error: None,
        }),
        Err(error) => Ok(CdnConnectionTestResult {
            success: false,
            provider,
            domain: None,
            error: Some(error),
        }),
    }
}

#[tauri::command]
pub async fn get_purge_status(
    profile_id: String,
    provider: String,
    distribution_id: String,
    invalidation_id: String,
    store: State<'_, ProfileStore>,
) -> Result<CdnPurgeStatusResult, String> {
    let result = async {
        let cdn_creds = store
            .get_cdn_credentials(&profile_id, &provider)
            .await
            .map_err(|e| e.to_string())?;

        match cdn_creds {
            CdnCredentials::CloudFront(creds) => {
                if distribution_id.trim().is_empty() {
                    return Err("CloudFront Distribution ID is required".to_string());
                }
                if invalidation_id.trim().is_empty() {
                    return Err("CloudFront Invalidation ID is required".to_string());
                }
                let adapter =
                    cdn::cloudfront::CloudFrontAdapter::new(creds).map_err(|e| e.to_string())?;
                let status = adapter
                    .get_invalidation_status(&distribution_id, &invalidation_id)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok((Some(status), None))
            }
            CdnCredentials::Akamai { .. } => Ok((
                Some("Accepted".to_string()),
                Some(
                    "Akamai Fast Purge???붿껌 ?깃났 ??蹂꾨룄 Invalidation ID ?놁씠 泥섎━?⑸땲??"
                        .to_string(),
                ),
            )),
            CdnCredentials::Lguplus { .. } => Ok((
                Some("NotImplemented".to_string()),
                Some(
                    "LG U+ CDN purge API is not implemented yet. API specification is required."
                        .to_string(),
                ),
            )),
            CdnCredentials::Hyosung { .. } => Ok((
                Some("NotImplemented".to_string()),
                Some(
                    "Hyosung CDN purge API is not implemented yet. API specification is required."
                        .to_string(),
                ),
            )),
        }
    }
    .await;

    match result {
        Ok((status, message)) => Ok(CdnPurgeStatusResult {
            success: true,
            provider,
            status,
            message,
            error: None,
        }),
        Err(error) => Ok(CdnPurgeStatusResult {
            success: false,
            provider,
            status: None,
            message: None,
            error: Some(error),
        }),
    }
}

#[tauri::command]
pub async fn verify_cdn_urls(
    profile_id: String,
    paths: Vec<String>,
    store: State<'_, ProfileStore>,
) -> Result<Vec<CdnUrlCheck>, String> {
    let profile = store
        .get_profile(&profile_id)
        .await
        .map_err(|e| e.to_string())?;
    let cdn_domain = profile
        .cdn_domain
        .filter(|domain| !domain.trim().is_empty())
        .ok_or_else(|| "CDN domain is required".to_string())?;

    let client = reqwest::Client::builder()
        .use_native_tls()
        .build()
        .map_err(|e| e.to_string())?;

    let mut checks = Vec::with_capacity(paths.len());
    for path in paths {
        let url = build_cdn_url(&cdn_domain, &path);
        checks.push(check_cdn_url(&client, url).await);
    }

    Ok(checks)
}

fn build_cdn_url(cdn_domain: &str, path: &str) -> String {
    cdn::build_cdn_url(cdn_domain, path)
}

async fn check_cdn_url(client: &reqwest::Client, url: String) -> CdnUrlCheck {
    let response = match client.head(&url).send().await {
        Ok(resp) if resp.status().as_u16() != 405 => Ok(resp),
        _ => client.get(&url).header(RANGE, "bytes=0-0").send().await,
    };

    match response {
        Ok(resp) => {
            let status = resp.status();
            let headers = resp.headers();
            CdnUrlCheck {
                url,
                ok: status.is_success(),
                status_code: Some(status.as_u16()),
                etag: header_to_string(headers.get(ETAG)),
                last_modified: header_to_string(headers.get(LAST_MODIFIED)),
                cache_control: header_to_string(headers.get(CACHE_CONTROL)),
                error: if status.is_success() {
                    None
                } else {
                    Some(status.to_string())
                },
            }
        }
        Err(err) => CdnUrlCheck {
            url,
            ok: false,
            status_code: None,
            etag: None,
            last_modified: None,
            cache_control: None,
            error: Some(err.to_string()),
        },
    }
}

fn header_to_string(value: Option<&reqwest::header::HeaderValue>) -> Option<String> {
    value.and_then(|v| v.to_str().ok()).map(ToOwned::to_owned)
}

/// H-6: 怨듦툒?먮퀎 CDN Purge ??CdnCredentials 湲곕컲?쇰줈 Akamai 吏??
#[tauri::command]
pub async fn purge_cdn(
    profile_id: String,
    provider: String,
    distribution_id: String,
    paths: Vec<String>,
    store: State<'_, ProfileStore>,
) -> Result<CdnPurgeResult, String> {
    let cdn_creds = store
        .get_cdn_credentials(&profile_id, &provider)
        .await
        .map_err(|e| e.to_string())?;

    let result = cdn::purge_with_credentials(&distribution_id, &paths, cdn_creds).await;

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
