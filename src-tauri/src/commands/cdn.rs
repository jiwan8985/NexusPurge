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
            CdnCredentials::Lguplus {
                username,
                password,
                service_name,
                volume_name,
                endpoint,
                cdn_domain,
            } => {
                let adapter = cdn::lguplus::LguplusCdnAdapter::new(
                    username, password, service_name, volume_name, endpoint, cdn_domain.clone(),
                )
                .map_err(|e| e.to_string())?;
                adapter.test_connection().await.map_err(|e| e.to_string())?;
                Ok(Some(cdn_domain))
            }
            CdnCredentials::Hyosung {
                api_key,
                api_secret,
                endpoint,
                cdn_domain,
            } => {
                let adapter = cdn::hyosung::HyosungCdnAdapter::new(
                    api_key,
                    api_secret,
                    endpoint,
                    distribution_id.clone(),
                    cdn_domain.clone(),
                )
                .map_err(|e| e.to_string())?;
                adapter.test_connection().await.map_err(|e| e.to_string())?;
                Ok(Some(cdn_domain))
            }
            CdnCredentials::Kt {
                username,
                password,
                service_name,
                volume_name,
                endpoint,
                cdn_domain,
            } => {
                let adapter = cdn::kt::KtCdnAdapter::new(
                    username, password, service_name, volume_name, endpoint, cdn_domain.clone(),
                )
                .map_err(|e| e.to_string())?;
                adapter.test_connection().await.map_err(|e| e.to_string())?;
                Ok(Some(cdn_domain))
            }
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
                    "Akamai Fast Purge 요청 성공 후 별도 Invalidation ID 없이 처리됩니다."
                        .to_string(),
                ),
            )),
            CdnCredentials::Lguplus {
                username,
                password,
                service_name,
                volume_name,
                endpoint,
                cdn_domain,
            } => {
                if invalidation_id.trim().is_empty() {
                    return Err("LG U+ CDN Invalidation ID(Transaction ID)가 필요합니다".to_string());
                }
                let adapter = cdn::lguplus::LguplusCdnAdapter::new(
                    username, password, service_name, volume_name, endpoint, cdn_domain,
                )
                .map_err(|e| e.to_string())?;
                let status = adapter
                    .get_transaction_status(&invalidation_id)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok((Some(status), None))
            }
            CdnCredentials::Hyosung { .. } => Ok((
                Some("Accepted".to_string()),
                Some("효성 ITX CDN purge 상태 조회 미지원 — 요청 후 즉시 처리됩니다.".to_string()),
            )),
            CdnCredentials::Kt {
                username,
                password,
                service_name,
                volume_name,
                endpoint,
                cdn_domain,
            } => {
                if invalidation_id.trim().is_empty() {
                    return Err("KT CDN Invalidation ID(Transaction ID)가 필요합니다".to_string());
                }
                let adapter = cdn::kt::KtCdnAdapter::new(
                    username, password, service_name, volume_name, endpoint, cdn_domain,
                )
                .map_err(|e| e.to_string())?;
                let status = adapter
                    .get_transaction_status(&invalidation_id)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok((Some(status), None))
            }
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

    let cdn_base_path = profile.cdn_base_path.clone();

    let client = reqwest::Client::builder()
        .use_native_tls()
        .build()
        .map_err(|e| e.to_string())?;

    let mut checks = Vec::with_capacity(paths.len());
    for path in paths {
        let url = build_cdn_url(&cdn_domain, &path, cdn_base_path.as_deref());
        checks.push(check_cdn_url(&client, url).await);
    }

    Ok(checks)
}

fn build_cdn_url(cdn_domain: &str, path: &str, cdn_base_path: Option<&str>) -> String {
    // S3 키에서 CDN base path를 제거 (예: "contents/file.txt" + base "contents/" → "file.txt")
    let effective_path = if let Some(base) = cdn_base_path.filter(|b| !b.trim().is_empty()) {
        let base_stripped = base.trim_start_matches('/').trim_end_matches('/');
        let prefix = format!("{}/", base_stripped);
        let key_stripped = path.trim_start_matches('/');
        if key_stripped.starts_with(&prefix) {
            &key_stripped[prefix.len()..]
        } else {
            key_stripped
        }
    } else {
        path.trim_start_matches('/')
    };
    cdn::build_cdn_url(cdn_domain, effective_path)
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
            // 403: CDN이 응답했으므로 CDN 자체는 동작 중.
            // 접근 제한(IP 제한, 서명 URL 필요 등)은 purge 성공 여부와 무관.
            let is_access_restricted = status.as_u16() == 403;
            CdnUrlCheck {
                url,
                ok: status.is_success() || is_access_restricted,
                status_code: Some(status.as_u16()),
                etag: header_to_string(headers.get(ETAG)),
                last_modified: header_to_string(headers.get(LAST_MODIFIED)),
                cache_control: header_to_string(headers.get(CACHE_CONTROL)),
                error: if status.is_success() || is_access_restricted {
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

/// H-6: 공급자별 CDN Purge 및 CdnCredentials 기반으로 Akamai 지원
#[tauri::command]
pub async fn purge_cdn(
    profile_id: String,
    provider: String,
    distribution_id: String,
    paths: Vec<String>,
    store: State<'_, ProfileStore>,
) -> Result<CdnPurgeResult, String> {
    let profile = store
        .get_profile(&profile_id)
        .await
        .map_err(|e| e.to_string())?;

    let cdn_creds = store
        .get_cdn_credentials(&profile_id, &provider)
        .await
        .map_err(|e| e.to_string())?;

    // cdn_base_path 제거하여 실제 CDN 경로 구성 (예: "contents/file.txt" + base "contents/" -> "file.txt")
    let normalized_paths = if let Some(base) = profile.cdn_base_path.as_deref().filter(|b| !b.trim().is_empty()) {
        let base_stripped = base.trim_start_matches('/').trim_end_matches('/');
        let prefix = format!("{}/", base_stripped);
        paths
            .iter()
            .map(|p| {
                let key_stripped = p.trim_start_matches('/');
                if key_stripped.starts_with(&prefix) {
                    key_stripped[prefix.len()..].to_owned()
                } else {
                    key_stripped.to_owned()
                }
            })
            .collect()
    } else {
        paths.clone()
    };

    let result = cdn::purge_with_credentials(&distribution_id, &normalized_paths, cdn_creds).await;

    match result {
        Ok(id) => Ok(CdnPurgeResult {
            success: true,
            provider,
            invalidation_id: id,
            paths, // 프론트엔드 매칭을 위해 원본 S3 키 경로 유지
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
