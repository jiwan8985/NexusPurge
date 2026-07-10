/// 효성 ITX CDN Purge Adapter
/// (docs/효성 CDN_PURGE_API_GUIDE_ver.2026.pdf 기준)
///
/// Auth:  정적 헤더 방식 (JWT 없음)
///        X-ITX-Security-Principal: {api_key}
///        X-ITX-Security-Secret:    {api_secret}
///
/// Purge: POST {endpoint}/api/v1/purge/{serviceId}   (기본 endpoint: https://api.xtrmcdn.co.kr:28091)
///        Content-Type: application/json
///        Body: {"filelist": ["http://cdn.domain.com/path1", ...]}  ← 스킴 포함 전체 URL
///        서버가 6500 byte 기준으로 내부 분할 처리 (다건은 POST 권장)
///
/// Response (meta + data 구조):
///   meta.status == "ok" && HTTP 200 → 성공, meta.transactionId 추적용
///   data 필드는 이스케이프된 JSON 문자열 → 2차 파싱 필요 ({successCount, failedCount, results})
///   부분 실패는 HTTP 500 + meta.message "Partial execution failure! failed=N"
///
/// 주의:
///   - GET 경로는 trailing slash 필수, POST는 trailing slash 없음 (누락 시 405)
///   - Purge URL 스킴은 기본 http (가이드 예시 기준), Edge Domain에 https:// 명시 시 https
///   - 와일드카드("prefix/*") 미지원 — 노드 purge 데몬이 502 반환 (purge_cdn에서 개별 파일로 확장)
///   - API 서버 인증서가 신뢰 체인에 없을 수 있어 TLS 검증을 우회함 (가이드 8장 참고)
use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;

pub struct HyosungCdnAdapter {
    client:     Client,
    api_key:    String,
    api_secret: String,
    endpoint:   String,
    service_id: String,
    cdn_domain: String,
}

/// POST 응답 외부 봉투
#[derive(Debug, Deserialize)]
struct PurgeEnvelope {
    meta: PurgeMeta,
    data: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct PurgeMeta {
    #[serde(rename = "statusCode")]
    #[allow(dead_code)]
    status_code: u16,
    status:      String,
    message:     Option<String>,
    #[serde(rename = "transactionId")]
    transaction_id: Option<String>,
}

/// data 문자열 2차 파싱 후 내부 구조
#[derive(Debug, Deserialize)]
struct PurgeSet {
    #[serde(rename = "successCount", default)]
    success_count: u32,
    #[serde(rename = "failedCount", default)]
    failed_count:  u32,
}

impl HyosungCdnAdapter {
    pub fn new(
        api_key:    String,
        api_secret: String,
        endpoint:   String,
        service_id: String,
        cdn_domain: String,
    ) -> Result<Self> {
        // 가이드 8장: API 서버(포트 28091) 인증서가 신뢰 체인에 없는 환경이 있어
        // curl -k 에 해당하는 TLS 검증 우회를 적용한다.
        let client = Client::builder()
            .use_native_tls()
            .danger_accept_invalid_certs(true)
            .build()
            .context("HTTP 클라이언트 생성 실패")?;
        let endpoint = endpoint.trim().trim_end_matches('/').to_owned();
        Ok(Self { client, api_key, api_secret, endpoint, service_id, cdn_domain })
    }

    /// 경로 목록을 완전한 CDN URL로 변환.
    /// 가이드 예시 기준 기본 스킴은 http. Edge Domain에 https://를 명시하면 https로 생성.
    /// (양쪽 스킴 동시 전송은 노드 명령에서 URL이 파이프 결합·중복되어 502를 유발하므로 단일 스킴만)
    fn build_urls(&self, paths: &[String]) -> Vec<String> {
        let raw = self.cdn_domain.trim().trim_end_matches('/');
        let (scheme, domain): (&str, &str) = if let Some(rest) = raw.strip_prefix("https://") {
            ("https", rest)
        } else if let Some(rest) = raw.strip_prefix("http://") {
            ("http", rest)
        } else {
            ("http", raw)
        };

        // 가이드 8장: 한글/특수문자 파일명은 URL 인코딩 형태 전달 권장
        // → 미인코딩 전달 시 노드 purge 데몬이 URL을 잘못 파싱해 실패한다
        paths
            .iter()
            .map(|p| {
                let encoded = crate::adapters::cdn::percent_encode_path_segments(
                    p.trim_start_matches('/'),
                );
                format!("{}://{}/{}", scheme, domain, encoded)
            })
            .collect()
    }

    /// POST /api/v1/purge/{serviceId} — 다건 일괄 Purge
    pub async fn purge_urls(&self, urls: &[String]) -> Result<Option<String>> {
        if urls.is_empty() {
            return Ok(None);
        }

        // POST 경로는 trailing slash 없음 (GET과 구별)
        let url = format!("{}/api/v1/purge/{}", self.endpoint, self.service_id);

        let body = serde_json::json!({ "filelist": urls }).to_string();

        let resp = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("X-ITX-Security-Principal", &self.api_key)
            .header("X-ITX-Security-Secret", &self.api_secret)
            .body(body)
            .send()
            .await
            .context("효성 ITX CDN Purge 요청 실패")?;

        let http_status = resp.status();
        let text = resp.text().await.context("효성 ITX CDN Purge 응답 읽기 실패")?;

        let envelope: PurgeEnvelope = serde_json::from_str(&text)
            .with_context(|| format!("효성 ITX CDN Purge 응답 파싱 실패: {}", text))?;

        let meta = &envelope.meta;

        // HTTP 에러 또는 meta.status != "ok"
        if !http_status.is_success() || meta.status != "ok" {
            // 원인 판별용: 시도한 URL(최대 3개)과 노드별 상세 결과(data)를 함께 보여준다
            let sample_urls = urls
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            let detail = envelope
                .data
                .as_ref()
                .map(|d| match d {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .unwrap_or_default();
            return Err(anyhow::anyhow!(
                "효성 ITX CDN Purge 실패 (HTTP {}, status={}): {} | 대상 URL: {}{} | 상세: {}",
                http_status,
                meta.status,
                meta.message.as_deref().unwrap_or(""),
                sample_urls,
                if urls.len() > 3 { format!(" 외 {}개", urls.len() - 3) } else { String::new() },
                if detail.is_empty() { "-".to_string() } else { detail },
            ));
        }

        // data 필드는 이스케이프된 JSON 문자열 → 2차 파싱
        if let Some(data_val) = &envelope.data {
            let data_str = match data_val {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };

            if let Ok(purge_set) = serde_json::from_str::<PurgeSet>(&data_str) {
                if purge_set.failed_count > 0 {
                    return Err(anyhow::anyhow!(
                        "효성 ITX CDN 부분 Purge 실패: 성공 {}, 실패 {} (transactionId: {})",
                        purge_set.success_count,
                        purge_set.failed_count,
                        meta.transaction_id.as_deref().unwrap_or("-"),
                    ));
                }

                tracing::info!(
                    "효성 ITX CDN Purge 완료: 성공 {} (transactionId: {})",
                    purge_set.success_count,
                    meta.transaction_id.as_deref().unwrap_or("-"),
                );
                return Ok(meta.transaction_id.clone());
            }
        }

        tracing::info!(
            "효성 ITX CDN Purge 완료: {} URLs (transactionId: {})",
            urls.len(),
            meta.transaction_id.as_deref().unwrap_or("-"),
        );
        Ok(meta.transaction_id.clone())
    }

    /// paths (S3 키) → CDN URL 변환 후 Purge
    pub async fn purge_paths(&self, paths: &[String]) -> Result<Option<String>> {
        let urls = self.build_urls(paths);
        self.purge_urls(&urls).await
    }

    /// 연결 테스트 — serviceId 존재 여부 확인 (GET trailing slash 필수)
    pub async fn test_connection(&self) -> Result<()> {
        // GET /api/v1/purge/{serviceId}/?target={dummy} 로 서비스 존재 확인
        let dummy_url = format!(
            "https://{}/connection-test",
            self.cdn_domain
                .trim()
                .trim_start_matches("https://")
                .trim_start_matches("http://")
                .trim_end_matches('/')
        );
        let url = format!(
            "{}/api/v1/purge/{}/",
            self.endpoint, self.service_id
        );

        let resp = self
            .client
            .get(&url)
            .query(&[("target", &dummy_url)])
            .header("X-ITX-Security-Principal", &self.api_key)
            .header("X-ITX-Security-Secret", &self.api_secret)
            .send()
            .await
            .context("효성 ITX CDN 연결 테스트 실패")?;

        let http_status = resp.status();
        let text = resp.text().await.unwrap_or_default();

        // 오류 코드 표: 401 = 인증 실패, 500 = 서비스 조회 실패(잘못된 serviceId 등),
        // 400 = target 검증 오류(서비스는 찾음), 200 = 성공 → 400/200은 연결 자체 성공
        if http_status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(anyhow::anyhow!(
                "효성 ITX CDN 인증 실패 (401): Principal/Secret을 확인하세요."
            ));
        }
        if http_status.is_server_error() {
            let message = serde_json::from_str::<PurgeEnvelope>(&text)
                .ok()
                .and_then(|env| env.meta.message)
                .unwrap_or_else(|| text.clone());
            return Err(anyhow::anyhow!(
                "효성 ITX CDN 연결 테스트 실패 (HTTP {}): {} — Service ID(Distribution ID 필드)를 확인하세요.",
                http_status,
                message
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_urls_normalizes_domain_and_path() {
        let adapter = HyosungCdnAdapter {
            client:     Client::new(),
            api_key:    "key".into(),
            api_secret: "secret".into(),
            endpoint:   "https://api.xtrmcdn.co.kr:28091".into(),
            service_id: "TID_18656".into(),
            cdn_domain: "https://cdn.example.com/".into(),
        };

        let urls = adapter.build_urls(&[
            "assets/app.js".to_string(),
            "/assets/style.css".to_string(),
        ]);

        assert_eq!(urls[0], "https://cdn.example.com/assets/app.js");
        assert_eq!(urls[1], "https://cdn.example.com/assets/style.css");
    }

    #[test]
    fn build_urls_percent_encodes_korean_and_space() {
        let adapter = HyosungCdnAdapter {
            client:     Client::new(),
            api_key:    "key".into(),
            api_secret: "secret".into(),
            endpoint:   "https://api.xtrmcdn.co.kr:28091".into(),
            service_id: "TID_18656".into(),
            cdn_domain: "https://cdn.example.com".into(),
        };

        let urls = adapter.build_urls(&["contents/한글 파일.txt".to_string()]);

        assert_eq!(
            urls[0],
            "https://cdn.example.com/contents/%ED%95%9C%EA%B8%80%20%ED%8C%8C%EC%9D%BC.txt"
        );
    }

    #[test]
    fn build_urls_without_scheme_defaults_to_http() {
        let adapter = HyosungCdnAdapter {
            client:     Client::new(),
            api_key:    "key".into(),
            api_secret: "secret".into(),
            endpoint:   "https://api.xtrmcdn.co.kr:28091".into(),
            service_id: "TID_18656".into(),
            cdn_domain: "cdn.example.com".into(),
        };

        let urls = adapter.build_urls(&["contents/test.png".to_string()]);

        assert_eq!(urls, vec!["http://cdn.example.com/contents/test.png".to_string()]);
    }
}
