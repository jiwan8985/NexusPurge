pub mod akamai;
pub mod base;
pub mod cloudfront;
pub mod hyosung;
pub mod kt;
pub mod lguplus;
pub mod mock;

use crate::utils::config::CdnCredentials;
use anyhow::Result;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// `purge_cdn` 커맨드가 호출하는 동안, 그 호출 과정에서 발생한 개별 HTTP 요청
/// (인증 → purge 등)을 순서대로 담아 cdn-*.log의 provider 블록에 함께 남긴다.
#[derive(Debug, Clone, Serialize)]
pub struct RequestStep {
    pub method: String,
    pub url: String,
    pub status: u16,
    #[serde(rename = "statusText")]
    pub status_text: String,
    #[serde(rename = "elapsedMs")]
    pub elapsed_ms: u64,
    pub summary: String,
}

tokio::task_local! {
    static REQUEST_STEPS: Arc<Mutex<Vec<RequestStep>>>;
}

/// `fut` 실행 중 발생한 `log_cdn_http` 호출들을 순서대로 모아 함께 반환한다.
/// task_local 스코프 밖(연결 테스트 등 단발 호출)에서는 아무 효과 없이 audit 로그에만 남는다.
pub(crate) async fn capture_request_steps<Fut, T>(fut: Fut) -> (T, Vec<RequestStep>)
where
    Fut: std::future::Future<Output = T>,
{
    let steps = Arc::new(Mutex::new(Vec::new()));
    let result = REQUEST_STEPS.scope(steps.clone(), fut).await;
    let collected = std::mem::take(&mut *steps.lock().unwrap());
    (result, collected)
}

/// 응답 본문에 섞인 개행을 공백으로 치환하고 길이를 제한한다 —
/// CloudFront XML 등 pretty-print된 응답이 로그 한 줄을 깨뜨리는 것을 방지.
fn sanitize_body_preview(body: &str, limit: usize) -> (String, bool) {
    let cleaned: String = body
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    let trimmed: String = cleaned.chars().take(limit).collect();
    let truncated = cleaned.chars().count() > limit;
    (trimmed, truncated)
}

/// CDN API 요청/응답 상세를 audit 로그(logs/audit.YYYY-MM-DD.log)에 남긴다.
/// 성공/실패 공통 — "왜 Purge가 안 됐는지"를 사후 추적할 수 있도록
/// 메서드·URL·HTTP 상태("200 OK" 등)·소요시간·응답 본문을 기록한다.
/// 응답 본문은 과도한 로그 방지를 위해 1,000자에서 자른다 (오류 전문은 별도로 에러 경로에 포함됨).
/// `capture_request_steps`로 감싼 호출 중이면 cdn-*.log용 요약도 함께 수집한다.
pub(crate) fn log_cdn_http(
    provider: &str,
    method: &str,
    url: &str,
    status: reqwest::StatusCode,
    elapsed_ms: u128,
    body: &str,
) {
    const BODY_LIMIT: usize = 1000;
    let (trimmed, truncated) = sanitize_body_preview(body, BODY_LIMIT);
    tracing::info!(
        "[{}] {} {} → HTTP {} ({}ms) 응답: {}{}",
        provider,
        method,
        url,
        status,
        elapsed_ms,
        if trimmed.trim().is_empty() { "(빈 응답)" } else { trimmed.as_str() },
        if truncated { " …(이하 생략)" } else { "" },
    );

    let _ = REQUEST_STEPS.try_with(|steps| {
        const STEP_SUMMARY_LIMIT: usize = 200;
        let (step_trimmed, step_truncated) = sanitize_body_preview(body, STEP_SUMMARY_LIMIT);
        let summary = if step_trimmed.trim().is_empty() {
            "(빈 응답)".to_string()
        } else if step_truncated {
            format!("{}…", step_trimmed)
        } else {
            step_trimmed
        };
        if let Ok(mut guard) = steps.lock() {
            guard.push(RequestStep {
                method: method.to_string(),
                url: url.to_string(),
                status: status.as_u16(),
                status_text: status.canonical_reason().unwrap_or("").to_string(),
                elapsed_ms: elapsed_ms as u64,
                summary,
            });
        }
    });
}

pub async fn purge_with_credentials(
    distribution_id: &str,
    paths: &[String],
    creds: CdnCredentials,
) -> Result<Option<String>> {
    match creds {
        CdnCredentials::CloudFront(aws_creds) => {
            if distribution_id.trim().is_empty() {
                return Err(anyhow::anyhow!("CloudFront Distribution ID is required"));
            }
            let adapter = cloudfront::CloudFrontAdapter::new(aws_creds)?;
            let mut last_err = None;
            for attempt in 0..3 {
                match adapter.create_invalidation(distribution_id, paths).await {
                    Ok(id) => return Ok(Some(id)),
                    Err(err) if attempt < 2 => {
                        last_err = Some(err);
                        tokio::time::sleep(retry_delay(attempt)).await;
                    }
                    Err(err) => return Err(err),
                }
            }
            Err(last_err.unwrap_or_else(|| anyhow::anyhow!("CloudFront purge retry failed")))
        }
        CdnCredentials::Akamai {
            client_token,
            client_secret,
            access_token,
            host,
            cdn_domain,
            cp_code,
        } => {
            if cdn_domain.trim().is_empty() {
                return Err(anyhow::anyhow!("Akamai CDN domain is required"));
            }
            let adapter =
                akamai::AkamaiAdapter::new(client_token, client_secret, access_token, host)?;

            // 와일드카드(폴더/전체) 경로는 Akamai URL Purge가 지원하지 않음 → CP Code 무효화로 처리
            let (wildcard, exact): (Vec<String>, Vec<String>) =
                paths.iter().cloned().partition(|p| p.ends_with('*'));

            if !wildcard.is_empty() {
                let code: u64 = cp_code
                    .as_deref()
                    .map(str::trim)
                    .filter(|c| !c.is_empty())
                    .ok_or_else(|| anyhow::anyhow!(
                        "Akamai 폴더/전체 Purge에는 CP Code가 필요합니다 (프로필 관리에서 CP Code 입력)"
                    ))?
                    .parse()
                    .map_err(|_| anyhow::anyhow!("Akamai CP Code는 숫자여야 합니다"))?;

                let mut last_err = None;
                let mut ok = false;
                for attempt in 0..3 {
                    match adapter.purge_cp_codes(&[code]).await {
                        Ok(()) => { ok = true; break; }
                        Err(err) if attempt < 2 => {
                            last_err = Some(err);
                            tokio::time::sleep(retry_delay(attempt)).await;
                        }
                        Err(err) => return Err(err),
                    }
                }
                if !ok {
                    return Err(last_err
                        .unwrap_or_else(|| anyhow::anyhow!("Akamai CP Code purge retry failed")));
                }
            }

            if !exact.is_empty() {
                let urls = build_cdn_urls(&cdn_domain, &exact);
                let mut last_err = None;
                let mut ok = false;
                for attempt in 0..3 {
                    match adapter.purge_urls(&urls).await {
                        Ok(()) => { ok = true; break; }
                        Err(err) if attempt < 2 => {
                            last_err = Some(err);
                            tokio::time::sleep(retry_delay(attempt)).await;
                        }
                        Err(err) => return Err(err),
                    }
                }
                if !ok {
                    return Err(
                        last_err.unwrap_or_else(|| anyhow::anyhow!("Akamai purge retry failed"))
                    );
                }
            }

            Ok(None)
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
            let adapter = lguplus::LguplusCdnAdapter::new(
                username, password, service_name, volume_name, endpoint, cdn_domain, service_type,
            )?;
            let mut last_err = None;
            for attempt in 0..3 {
                match adapter.purge_paths(paths).await {
                    Ok(id) => return Ok(id),
                    Err(err) if attempt < 2 => {
                        last_err = Some(err);
                        tokio::time::sleep(retry_delay(attempt)).await;
                    }
                    Err(err) => return Err(err),
                }
            }
            Err(last_err.unwrap_or_else(|| anyhow::anyhow!("LG U+ purge retry failed")))
        }
        CdnCredentials::Hyosung {
            api_key,
            api_secret,
            endpoint,
            cdn_domain,
        } => {
            if distribution_id.trim().is_empty() {
                return Err(anyhow::anyhow!(
                    "효성 ITX CDN Service ID가 필요합니다 (프로필의 Distribution ID 필드에 입력)"
                ));
            }
            if cdn_domain.trim().is_empty() {
                return Err(anyhow::anyhow!(
                    "효성 ITX CDN Domain이 필요합니다"
                ));
            }
            let adapter = hyosung::HyosungCdnAdapter::new(
                api_key,
                api_secret,
                endpoint,
                distribution_id.to_owned(),
                cdn_domain,
            )?;
            let mut last_err = None;
            for attempt in 0..3 {
                match adapter.purge_paths(paths).await {
                    Ok(id) => return Ok(id),
                    Err(err) if attempt < 2 => {
                        last_err = Some(err);
                        tokio::time::sleep(retry_delay(attempt)).await;
                    }
                    Err(err) => return Err(err),
                }
            }
            Err(last_err.unwrap_or_else(|| anyhow::anyhow!("Hyosung purge retry failed")))
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
            let adapter = kt::KtCdnAdapter::new(
                username, password, service_name, volume_name, endpoint, cdn_domain, service_type,
            )?;
            let mut last_err = None;
            for attempt in 0..3 {
                match adapter.purge_paths(paths).await {
                    Ok(id) => return Ok(id),
                    Err(err) if attempt < 2 => {
                        last_err = Some(err);
                        tokio::time::sleep(retry_delay(attempt)).await;
                    }
                    Err(err) => return Err(err),
                }
            }
            Err(last_err.unwrap_or_else(|| anyhow::anyhow!("KT purge retry failed")))
        }
    }
}

/// 경로를 슬래시 단위로 분리하여 각 세그먼트만 percent-encode
/// (슬래시 자체는 그대로 유지, 한글/공백/특수문자만 인코딩)
pub fn percent_encode_path_segments(raw: &str) -> String {
    const PATH_SAFE: &percent_encoding::AsciiSet = &NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'_')
        .remove(b'.')
        .remove(b'~');

    let had_leading_slash = raw.starts_with('/');
    let trimmed = raw.trim_start_matches('/');
    let encoded = trimmed
        .split('/')
        .map(|seg| utf8_percent_encode(seg, PATH_SAFE).to_string())
        .collect::<Vec<_>>()
        .join("/");

    if had_leading_slash {
        format!("/{}", encoded)
    } else {
        encoded
    }
}

pub fn build_cdn_url(cdn_domain: &str, object_path: &str) -> String {
    let domain = cdn_domain
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');

    let encoded = percent_encode_path_segments(object_path.trim_start_matches('/'));

    format!("https://{}/{}", domain, encoded)
}

pub fn build_cdn_urls(cdn_domain: &str, paths: &[String]) -> Vec<String> {
    paths
        .iter()
        .map(|path| build_cdn_url(cdn_domain, path))
        .collect()
}

fn retry_delay(attempt: usize) -> Duration {
    Duration::from_millis(250 * 2_u64.pow(attempt as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// audit.log에서 CloudFront XML 등 개행 섞인 응답이 한 줄을 깨뜨리던 문제 재현/방지
    #[test]
    fn sanitize_body_preview_replaces_newlines_and_truncates() {
        let (cleaned, truncated) = sanitize_body_preview("line1\nline2\r\nline3", 100);
        assert_eq!(cleaned, "line1 line2  line3");
        assert!(!truncated);

        let (cleaned, truncated) = sanitize_body_preview("abcdefgh", 5);
        assert_eq!(cleaned, "abcde");
        assert!(truncated);
    }

    /// capture_request_steps로 감싼 호출 중 log_cdn_http가 호출되면 순서대로 수집되고,
    /// 스코프 밖(연결 테스트 등 단발 호출)에서는 조용히 무시되어야 함
    #[tokio::test]
    async fn capture_request_steps_collects_calls_in_order_and_ignores_outside_scope() {
        // 스코프 밖: 패닉 없이 그냥 무시됨
        log_cdn_http("kt", "GET", "https://example.com/outside", reqwest::StatusCode::OK, 1, "");

        let (value, steps) = capture_request_steps(async {
            log_cdn_http(
                "kt",
                "POST(인증)",
                "https://api.ktcdn.co.kr/v3/auth/tokens",
                reqwest::StatusCode::OK,
                372,
                "",
            );
            log_cdn_http(
                "kt",
                "POST",
                "https://api.ktcdn.co.kr/v3/management/service/x/purge",
                reqwest::StatusCode::CREATED,
                41,
                "{\"transid\":123}",
            );
            42
        })
        .await;

        assert_eq!(value, 42);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].status, 200);
        assert_eq!(steps[0].elapsed_ms, 372);
        assert_eq!(steps[1].status, 201);
        assert_eq!(steps[1].summary, "{\"transid\":123}");
    }

    #[test]
    fn percent_encode_path_segments_encodes_korean_and_space_but_keeps_slashes() {
        let encoded = percent_encode_path_segments("/contents/한글 파일.txt");
        assert_eq!(encoded, "/contents/%ED%95%9C%EA%B8%80%20%ED%8C%8C%EC%9D%BC.txt");
        // ASCII 안전 문자는 그대로 유지 (이중 인코딩 없음)
        assert_eq!(
            percent_encode_path_segments("assets/app-v1.2_final.js"),
            "assets/app-v1.2_final.js"
        );
    }

    #[test]
    fn build_cdn_url_normalizes_scheme_domain_and_path() {
        assert_eq!(
            build_cdn_url("https://cdn.example.com/", "/assets/app.js"),
            "https://cdn.example.com/assets/app.js"
        );
        assert_eq!(
            build_cdn_url("http://cdn.example.com/base", "assets/app.js"),
            "https://cdn.example.com/base/assets/app.js"
        );
    }

    #[tokio::test]
    async fn hyosung_requires_service_id() {
        // distribution_id(serviceId) 없이 호출하면 명확한 오류 반환
        let result = purge_with_credentials(
            "",
            &["assets/app.js".to_string()],
            CdnCredentials::Hyosung {
                api_key:    "key".to_string(),
                api_secret: "secret".to_string(),
                endpoint:   "https://api.xtrmcdn.co.kr:28091".to_string(),
                cdn_domain: "cdn.example.com".to_string(),
            },
        )
        .await;

        let err = result.expect_err("serviceId 없이 호출 시 오류여야 함");
        assert!(
            err.to_string().contains("Service ID"),
            "오류 메시지에 Service ID 언급 필요: {}",
            err
        );
    }

    #[tokio::test]
    async fn hyosung_requires_cdn_domain() {
        let result = purge_with_credentials(
            "TID_18656",
            &["assets/app.js".to_string()],
            CdnCredentials::Hyosung {
                api_key:    "key".to_string(),
                api_secret: "secret".to_string(),
                endpoint:   "https://api.xtrmcdn.co.kr:28091".to_string(),
                cdn_domain: "".to_string(),
            },
        )
        .await;

        let err = result.expect_err("cdn_domain 없이 호출 시 오류여야 함");
        assert!(
            err.to_string().contains("Domain"),
            "오류 메시지에 Domain 언급 필요: {}",
            err
        );
    }
}
