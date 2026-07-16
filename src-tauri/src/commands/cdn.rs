use crate::adapters::cdn;
use crate::utils::config::CdnCredentials;
use crate::utils::config::ProfileStore;
use serde::Serialize;
use std::error::Error;
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
    /// 실제 호출된 CDN Purge API 엔드포인트 (감사/디버깅용 — 보안팀 로그 전달 시 참고)
    #[serde(default, rename = "requestEndpoint")]
    pub request_endpoint: Option<String>,
    /// 요청 소요 시간 (ms)
    #[serde(default, rename = "durationMs")]
    pub duration_ms: Option<u64>,
    /// 이 Purge 요청 과정에서 실제로 발생한 개별 HTTP 호출(인증 → purge 등) 순서 —
    /// cdn-*.log에 provider 블록 하위로 상태코드·소요시간과 함께 기록된다.
    #[serde(default, rename = "requestSteps")]
    pub request_steps: Vec<cdn::RequestStep>,
}

/// 대량 Purge → 폴더 Purge 전환 임계값: 개별 경로가 이 수 이상이면
/// 와일드카드를 네이티브 지원하는 벤더(CloudFront/Akamai)는 공통 폴더 와일드카드 1건으로 전환
const FOLDER_PURGE_CONVERSION_THRESHOLD: usize = 100;

/// 경로 목록의 가장 깊은 공통 상위 폴더를 찾아 "{prefix}/*" 와일드카드로 축약.
/// 공통 폴더가 없으면 전체("/*")를 반환한다.
fn collapse_to_folder_wildcard(paths: &[String]) -> String {
    let mut common: Option<Vec<&str>> = None;
    for p in paths {
        let trimmed = p.trim_start_matches('/');
        // 파일명(마지막 세그먼트)을 제외한 폴더 세그먼트만 비교
        let segments: Vec<&str> = match trimmed.rsplit_once('/') {
            Some((dir, _file)) => dir.split('/').collect(),
            None => Vec::new(), // 루트 바로 아래 파일 — 공통 폴더 없음
        };
        common = Some(match common {
            None => segments,
            Some(prev) => prev
                .iter()
                .zip(segments.iter())
                .take_while(|(a, b)| a == b)
                .map(|(a, _)| *a)
                .collect(),
        });
        if common.as_ref().is_some_and(|c| c.is_empty()) {
            break;
        }
    }

    match common {
        Some(segs) if !segs.is_empty() => format!("{}/*", segs.join("/")),
        _ => "/*".to_string(),
    }
}

/// Purge 요청이 실제로 호출하는 API 엔드포인트를 설명 문자열로 반환 (로그·감사용, 실제 호출은 아님)
fn describe_cdn_endpoint(creds: &CdnCredentials, distribution_id: &str) -> String {
    match creds {
        CdnCredentials::CloudFront(_) => format!(
            "POST https://cloudfront.amazonaws.com/2020-05-31/distribution/{}/invalidation",
            distribution_id
        ),
        CdnCredentials::Akamai { host, .. } => format!(
            "POST https://{}/ccu/v3/invalidate/url/production (폴더/전체 Purge는 .../invalidate/cpcode/production)",
            host
        ),
        CdnCredentials::Lguplus { endpoint, service_name, .. } => format!(
            "POST {}/v3/management/service/{}/volume/{{volumeName}}/purge (Volume Name 미설정 시 .../domain/{{domain}}/purge)",
            endpoint, service_name
        ),
        CdnCredentials::Kt { endpoint, service_name, .. } => format!(
            "POST {}/v3/management/service/{}/volume/{{volumeName}}/purge (Volume Name 미설정 시 .../domain/{{domain}}/purge)",
            endpoint, service_name
        ),
        CdnCredentials::Hyosung { endpoint, .. } => format!(
            "POST {}/api/v1/purge/{}",
            endpoint, distribution_id
        ),
    }
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
            request_endpoint: None,
            duration_ms: None,
            request_steps: Vec::new(),
        }),
        Err(e) => Ok(CdnPurgeResult {
            success: false,
            provider: "cloudfront".into(),
            invalidation_id: None,
            paths,
            purged_at: None,
            error: Some(e.to_string()),
            request_endpoint: None,
            duration_ms: None,
            request_steps: Vec::new(),
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
                cp_code: _,
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
                service_type,
            } => {
                let adapter = cdn::lguplus::LguplusCdnAdapter::new(
                    username, password, service_name, volume_name, endpoint, cdn_domain.clone(), service_type,
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
                service_type,
            } => {
                let adapter = cdn::kt::KtCdnAdapter::new(
                    username, password, service_name, volume_name, endpoint, cdn_domain.clone(), service_type,
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
                service_type,
            } => {
                if invalidation_id.trim().is_empty() {
                    return Err("LG U+ CDN Invalidation ID(Transaction ID)가 필요합니다".to_string());
                }
                let adapter = cdn::lguplus::LguplusCdnAdapter::new(
                    username, password, service_name, volume_name, endpoint, cdn_domain, service_type,
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
                service_type,
            } => {
                if invalidation_id.trim().is_empty() {
                    return Err("KT CDN Invalidation ID(Transaction ID)가 필요합니다".to_string());
                }
                let adapter = cdn::kt::KtCdnAdapter::new(
                    username, password, service_name, volume_name, endpoint, cdn_domain, service_type,
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


/// H-6: 공급자별 CDN Purge 및 CdnCredentials 기반으로 Akamai 지원
#[tauri::command]
pub async fn purge_cdn(
    profile_id: String,
    provider: String,
    distribution_id: String,
    paths: Vec<String>,
    store: State<'_, ProfileStore>,
    cache: State<'_, crate::utils::adapter_cache::AdapterCache>,
) -> Result<CdnPurgeResult, String> {
    let profile = store
        .get_profile(&profile_id)
        .await
        .map_err(|e| e.to_string())?;

    let cdn_creds = store
        .get_cdn_credentials(&profile_id, &provider)
        .await
        .map_err(|e| e.to_string())?;

    // ── 벤더별 폴더/대량 Purge 조건 처리 ─────────────────────────────────────
    //
    // | 벤더            | 폴더(와일드카드) Purge                  | 대량(개별 경로 THRESHOLD 이상)   |
    // |----------------|----------------------------------------|--------------------------------|
    // | CloudFront     | 와일드카드 네이티브 지원                 | 공통 폴더 와일드카드로 전환       |
    // | Akamai         | CP Code 무효화로 처리 (mod.rs)          | CP Code 설정 시 와일드카드 전환   |
    // | LG U+/KT       | 전체("/*")+cloudcdn → Purge by Service, | 전환 불가 (filelist API만 지원)  |
    // |                | 그 외 → S3 목록으로 개별 파일 확장       |                                |
    // | 효성            | S3 목록으로 개별 파일 확장 (와일드카드 시 502) | 전환 불가                   |

    // LG U+/KT: 버킷 루트 전체 Purge("/*" 단일 항목) + 서비스 타입 cloudcdn이면
    // Purge by Service(전체 즉시 플러시)로 처리해 대량 S3 목록 조회를 피한다.
    let is_full_root_wildcard =
        paths.len() == 1 && paths[0].trim_start_matches('/') == "*";

    if (provider == "lguplus" || provider == "kt") && is_full_root_wildcard {
        let is_cloudcdn = matches!(
            &cdn_creds,
            CdnCredentials::Lguplus { service_type, .. } if service_type == "cloudcdn"
        ) || matches!(
            &cdn_creds,
            CdnCredentials::Kt { service_type, .. } if service_type == "cloudcdn"
        );

        if is_cloudcdn {
            let request_endpoint = match &cdn_creds {
                CdnCredentials::Lguplus { endpoint, service_name, .. }
                | CdnCredentials::Kt { endpoint, service_name, .. } => Some(format!(
                    "POST {}/v3/management/service/{}/purge (Purge by Service — 전체 즉시 플러시)",
                    endpoint, service_name
                )),
                _ => None,
            };
            let started = std::time::Instant::now();
            let (result, request_steps) = match &cdn_creds {
                CdnCredentials::Lguplus {
                    username, password, service_name, volume_name, endpoint, cdn_domain, service_type,
                } => {
                    let adapter = cdn::lguplus::LguplusCdnAdapter::new(
                        username.clone(), password.clone(), service_name.clone(),
                        volume_name.clone(), endpoint.clone(), cdn_domain.clone(), service_type.clone(),
                    )
                    .map_err(|e| e.to_string())?;
                    cdn::capture_request_steps(adapter.purge_service()).await
                }
                CdnCredentials::Kt {
                    username, password, service_name, volume_name, endpoint, cdn_domain, service_type,
                } => {
                    let adapter = cdn::kt::KtCdnAdapter::new(
                        username.clone(), password.clone(), service_name.clone(),
                        volume_name.clone(), endpoint.clone(), cdn_domain.clone(), service_type.clone(),
                    )
                    .map_err(|e| e.to_string())?;
                    cdn::capture_request_steps(adapter.purge_service()).await
                }
                _ => unreachable!("provider 검사로 Lguplus/Kt만 도달"),
            };
            let duration_ms = Some(started.elapsed().as_millis() as u64);
            return Ok(match result {
                Ok(id) => CdnPurgeResult {
                    success: true,
                    provider,
                    invalidation_id: id,
                    paths,
                    purged_at: Some(chrono::Utc::now().to_rfc3339()),
                    error: None,
                    request_endpoint,
                    duration_ms,
                    request_steps,
                },
                Err(e) => CdnPurgeResult {
                    success: false,
                    provider,
                    invalidation_id: None,
                    paths,
                    purged_at: None,
                    error: Some(e.to_string()),
                    request_endpoint,
                    duration_ms,
                    request_steps,
                },
            });
        }
        // service_type이 volume이면 아래 개별 파일 확장 로직으로 계속 진행
    }

    // 효성·LG U+·KT는 와일드카드 미지원 (효성: 노드 purge 데몬이 "*" URL에 502 반환,
    // LG U+/KT: 공식 v3 문서상 filelist는 리터럴 경로만 허용)
    // → 폴더/전체 Purge("prefix/*")를 S3 목록 조회로 개별 파일 경로로 확장
    let needs_wildcard_expansion = matches!(provider.as_str(), "hyosung" | "lguplus" | "kt")
        && paths.iter().any(|p| p.ends_with('*'));

    let effective_paths: Vec<String> = if needs_wildcard_expansion {
        let (creds, region, bucket, endpoint) = store
            .get_connection_info(&profile_id)
            .await
            .map_err(|e| e.to_string())?;
        let adapter = cache
            .get_or_create(&profile_id, || async {
                crate::adapters::storage::s3::S3Adapter::new(
                    &region, &bucket, &creds, endpoint.as_deref(),
                )
                .await
            })
            .await
            .map_err(|e| e.to_string())?;

        let mut expanded = Vec::new();
        for p in &paths {
            if let Some(prefix) = p.strip_suffix('*') {
                let prefix = prefix.trim_start_matches('/');
                match adapter.list_keys_recursive(prefix).await {
                    Ok(keys) => {
                        expanded.extend(keys.into_iter().filter(|k| !k.ends_with('/')));
                    }
                    Err(e) => {
                        return Ok(CdnPurgeResult {
                            success: false,
                            provider,
                            invalidation_id: None,
                            paths,
                            purged_at: None,
                            error: Some(format!(
                                "폴더 Purge 확장 실패 (S3 목록 조회 오류): {}",
                                e
                            )),
                            request_endpoint: None,
                            duration_ms: None,
                            request_steps: Vec::new(),
                        });
                    }
                }
            } else {
                expanded.push(p.clone());
            }
        }
        expanded.sort();
        expanded.dedup();

        if expanded.is_empty() {
            // 빈 폴더 — 무효화할 파일 없음, 성공 처리
            return Ok(CdnPurgeResult {
                success: true,
                provider,
                invalidation_id: None,
                paths,
                purged_at: Some(chrono::Utc::now().to_rfc3339()),
                error: None,
                request_endpoint: None,
                duration_ms: None,
                request_steps: Vec::new(),
            });
        }
        tracing::info!(
            "[{}] 폴더 Purge 확장: {}개 와일드카드 → {}개 파일 경로",
            provider,
            paths.len(),
            expanded.len()
        );
        expanded
    } else {
        paths.clone()
    };

    // 대량 전환: 와일드카드를 네이티브 지원하는 벤더(CloudFront, CP Code 설정된 Akamai)는
    // 개별 경로가 임계값 이상이면 공통 상위 폴더 와일드카드 1건으로 전환
    // (CloudFront 무효화 경로 수 절감 + Akamai URL Purge 요청 크기 제한 회피)
    let effective_paths: Vec<String> = {
        let supports_wildcard = provider == "cloudfront"
            || (provider == "akamai"
                && matches!(
                    &cdn_creds,
                    CdnCredentials::Akamai { cp_code: Some(c), .. } if !c.trim().is_empty()
                ));
        let exact_count = effective_paths.iter().filter(|p| !p.ends_with('*')).count();

        if supports_wildcard && exact_count >= FOLDER_PURGE_CONVERSION_THRESHOLD {
            let converted = collapse_to_folder_wildcard(&effective_paths);
            tracing::info!(
                "[{}] 대량 Purge 전환: 개별 경로 {}개 → 폴더 Purge \"{}\" ({}건 임계값 초과)",
                provider,
                exact_count,
                converted,
                FOLDER_PURGE_CONVERSION_THRESHOLD
            );
            vec![converted]
        } else {
            effective_paths
        }
    };

    // cdn_base_path 제거하여 실제 CDN 경로 구성 (예: "contents/file.txt" + base "contents/" -> "file.txt")
    let normalized_paths = if let Some(base) = profile.cdn_base_path.as_deref().filter(|b| !b.trim().is_empty()) {
        let base_stripped = base.trim_start_matches('/').trim_end_matches('/');
        let prefix = format!("{}/", base_stripped);
        effective_paths
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
        effective_paths
    };

    let request_endpoint = describe_cdn_endpoint(&cdn_creds, &distribution_id);
    let started = std::time::Instant::now();
    let (result, request_steps) = cdn::capture_request_steps(cdn::purge_with_credentials(
        &distribution_id,
        &normalized_paths,
        cdn_creds,
    ))
    .await;
    let duration_ms = Some(started.elapsed().as_millis() as u64);

    match result {
        Ok(id) => Ok(CdnPurgeResult {
            success: true,
            provider,
            invalidation_id: id,
            paths, // 프론트엔드 매칭을 위해 원본 S3 키 경로 유지
            purged_at: Some(chrono::Utc::now().to_rfc3339()),
            error: None,
            request_endpoint: Some(request_endpoint),
            duration_ms,
            request_steps,
        }),
        Err(e) => Ok(CdnPurgeResult {
            success: false,
            provider,
            invalidation_id: None,
            paths,
            purged_at: None,
            error: Some(e.to_string()),
            request_endpoint: Some(request_endpoint),
            duration_ms,
            request_steps,
        }),
    }
}

// ─── URL 실시간 조회 (속성 다이얼로그 — 크롬 개발자모드 Network 탭과 유사한 상세 정보) ──────

#[derive(Debug, Serialize)]
pub struct UrlInspection {
    pub url: String,
    #[serde(rename = "statusCode")]
    pub status_code: Option<u16>,
    /// 응답 헤더 원본 순서 그대로 (key, value) — DevTools Response Headers와 동일한 형태
    pub headers: Vec<(String, String)>,
    #[serde(rename = "durationMs")]
    pub duration_ms: u64,
    pub error: Option<String>,
    /// 오류 분류: "dns" | "timeout" | "connect" | "tls" | "other" (성공 시 None)
    #[serde(rename = "errorKind")]
    pub error_kind: Option<String>,
}

/// URL에서 DNS 사전 조회용 (호스트, 포트) 추출. 유효한 절대 URL이 아니면 None.
fn inspect_target(url: &str) -> Option<(String, u16)> {
    let parsed = reqwest::Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_string();
    let port = parsed.port_or_known_default()?;
    Some((host, port))
}

fn dns_failure_message(host: &str) -> String {
    format!(
        "DNS 조회 실패: {} — 도메인이 DNS에 등록되어 있지 않습니다(NXDOMAIN). \
         CDN 서비스 도메인의 DNS/CNAME 등록 후 다시 시도하세요.",
        host
    )
}

/// reqwest 전송 오류를 (errorKind, 한국어 메시지)로 분류.
/// reqwest::Error는 테스트에서 직접 생성할 수 없어 판별 플래그 + 오류 원문으로 분리했다.
fn classify_send_error(
    is_timeout: bool,
    is_connect: bool,
    detail: &str,
    url: &str,
) -> (&'static str, String) {
    let lower = detail.to_lowercase();
    if is_timeout {
        (
            "timeout",
            format!(
                "응답 시간 초과(10초): {} — 서버가 응답하지 않습니다. 방화벽/사설망 여부를 확인하세요.",
                url
            ),
        )
    } else if lower.contains("certificate") || lower.contains("tls") || lower.contains("ssl") {
        ("tls", format!("TLS 인증서 오류: {} ({})", url, detail))
    } else if is_connect {
        (
            "connect",
            format!(
                "연결 실패: {} — 호스트에 연결할 수 없습니다(포트 차단 또는 서버 다운). ({})",
                url, detail
            ),
        )
    } else {
        ("other", format!("요청 실패: {} ({})", url, detail))
    }
}

/// 주어진 URL에 실제 HTTP 요청(HEAD, 미지원 시 GET Range)을 보내 상태코드·응답 헤더·소요시간을 그대로 반환.
/// S3 객체 속성 다이얼로그에서 "실시간 확인" 버튼으로 호출 — 자동 실행되지 않음(온디맨드).
///
/// 테스트/스테이징 CDN 엣지 도메인은 인증서 체인이 사설 CA이거나 SNI가 맞지 않는 경우가 흔해
/// (효성 API 서버와 동일한 문제 — hyosung.rs 참고) TLS 검증을 curl -k 와 동일하게 우회한다.
/// 이 커맨드는 진단/디버깅 전용이며 실제 Purge 요청 경로와는 무관하다.
#[tauri::command]
pub async fn inspect_url(url: String) -> Result<UrlInspection, String> {
    let started = std::time::Instant::now();

    // DNS 사전 조회 — 미등록 도메인(NXDOMAIN)이면 10초 타임아웃을 기다리지 않고 즉시 원인 반환
    let Some((host, port)) = inspect_target(&url) else {
        return Ok(UrlInspection {
            url: url.clone(),
            status_code: None,
            headers: vec![],
            duration_ms: 0,
            error: Some(format!("URL 형식이 올바르지 않습니다: {}", url)),
            error_kind: Some("other".into()),
        });
    };
    if tokio::net::lookup_host((host.as_str(), port)).await.is_err() {
        return Ok(UrlInspection {
            url,
            status_code: None,
            headers: vec![],
            duration_ms: started.elapsed().as_millis() as u64,
            error: Some(dns_failure_message(&host)),
            error_kind: Some("dns".into()),
        });
    }

    let client = reqwest::Client::builder()
        .use_native_tls()
        .danger_accept_invalid_certs(true)
        // 접근 불가한 사설망 도메인일 경우 무한 대기하지 않고 명확한 오류로 빨리 실패시킴
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let request_started = std::time::Instant::now();
    let response = match client.head(&url).send().await {
        Ok(resp) if resp.status().as_u16() != 405 => Ok(resp),
        _ => {
            client
                .get(&url)
                .header(reqwest::header::RANGE, "bytes=0-0")
                .send()
                .await
        }
    };
    let duration_ms = request_started.elapsed().as_millis() as u64;

    match response {
        Ok(resp) => {
            let status_code = Some(resp.status().as_u16());
            let headers = resp
                .headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("<binary>").to_string()))
                .collect();
            Ok(UrlInspection { url, status_code, headers, duration_ms, error: None, error_kind: None })
        }
        Err(e) => {
            // 오류 원인 체인 전체를 모아 TLS/연결 오류 문구를 판별
            let mut detail_parts = Vec::new();
            let mut current: Option<&dyn Error> = Some(&e);
            while let Some(err) = current {
                detail_parts.push(err.to_string());
                current = err.source();
            }
            let detail = detail_parts.join(": ");
            let (kind, message) = classify_send_error(e.is_timeout(), e.is_connect(), &detail, &url);
            Ok(UrlInspection {
                url,
                status_code: None,
                headers: vec![],
                duration_ms,
                error: Some(message),
                error_kind: Some(kind.into()),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapse_finds_deepest_common_folder() {
        let paths = vec![
            "assets/img/a.png".to_string(),
            "assets/img/b.png".to_string(),
            "assets/img/icons/c.svg".to_string(),
        ];
        assert_eq!(collapse_to_folder_wildcard(&paths), "assets/img/*");
    }

    #[test]
    fn collapse_falls_back_to_root_when_no_common_folder() {
        let paths = vec![
            "assets/a.png".to_string(),
            "contents/b.png".to_string(),
        ];
        assert_eq!(collapse_to_folder_wildcard(&paths), "/*");
    }

    #[test]
    fn collapse_root_level_files_return_root_wildcard() {
        let paths = vec!["a.png".to_string(), "b.png".to_string()];
        assert_eq!(collapse_to_folder_wildcard(&paths), "/*");
    }

    #[test]
    fn collapse_single_folder() {
        let paths = vec![
            "contents/file1.txt".to_string(),
            "/contents/file2.txt".to_string(), // 선행 슬래시 혼재 허용
        ];
        assert_eq!(collapse_to_folder_wildcard(&paths), "contents/*");
    }

    #[test]
    fn inspect_target_parses_host_and_port() {
        assert_eq!(
            inspect_target("http://cdn.example.com/a.txt"),
            Some(("cdn.example.com".to_string(), 80))
        );
        assert_eq!(
            inspect_target("https://cdn.example.com:8443/contents/a.txt"),
            Some(("cdn.example.com".to_string(), 8443))
        );
        // 스킴 없는 상대 경로는 URL이 아님
        assert_eq!(inspect_target("contents/a.txt"), None);
    }

    #[test]
    fn dns_failure_message_mentions_host_and_guidance() {
        let msg = dns_failure_message("sklb-test.dn.nexoncdn.co.kr");
        assert!(msg.contains("sklb-test.dn.nexoncdn.co.kr"));
        assert!(msg.contains("NXDOMAIN"));
        assert!(msg.contains("CNAME"));
    }

    #[test]
    fn classify_send_error_priority() {
        // timeout이 최우선
        let (kind, msg) = classify_send_error(true, false, "operation timed out", "http://a/b");
        assert_eq!(kind, "timeout");
        assert!(msg.contains("http://a/b"));

        // TLS 문구는 connect보다 우선 (TLS 핸드셰이크 실패도 is_connect=true로 옴)
        let (kind, _) = classify_send_error(false, true, "invalid peer certificate", "http://a/b");
        assert_eq!(kind, "tls");

        let (kind, msg) = classify_send_error(false, true, "connection refused", "http://a/b");
        assert_eq!(kind, "connect");
        assert!(msg.contains("connection refused"));

        let (kind, _) = classify_send_error(false, false, "unexpected", "http://a/b");
        assert_eq!(kind, "other");
    }
}
