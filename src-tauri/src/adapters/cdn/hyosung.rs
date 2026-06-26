/// 효성 ITX CDN Purge Adapter
///
/// Auth:  정적 헤더 방식 (JWT 없음)
///        X-ITX-Security-Principal: {api_key}
///        X-ITX-Security-Secret:    {api_secret}
///
/// Purge: POST {endpoint}/api/v1/purge/{serviceId}
///        Content-Type: application/json
///        Body: {"filelist": ["https://cdn.domain.com/path1", ...]}
///
/// Response (meta + data 구조):
///   meta.status == "ok"  → 성공
///   meta.statusCode 200  → 성공
///   data 필드는 이스케이프된 JSON 문자열 → 2차 파싱 필요
///   data.failedCount > 0 → 부분 실패로 오류 반환
///
/// 주의: GET 경로는 trailing slash 필수, POST는 trailing slash 없음.
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
        let client = Client::builder()
            .use_native_tls()
            .build()
            .context("HTTP 클라이언트 생성 실패")?;
        let endpoint = endpoint.trim().trim_end_matches('/').to_owned();
        Ok(Self { client, api_key, api_secret, endpoint, service_id, cdn_domain })
    }

    /// 경로 목록을 완전한 CDN URL로 변환
    fn build_urls(&self, paths: &[String]) -> Vec<String> {
        let domain = self
            .cdn_domain
            .trim()
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_end_matches('/');

        paths
            .iter()
            .map(|p| {
                let path = p.trim_start_matches('/');
                format!("https://{}/{}", domain, path)
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
            return Err(anyhow::anyhow!(
                "효성 ITX CDN Purge 실패 (HTTP {}, status={}): {}",
                http_status,
                meta.status,
                meta.message.as_deref().unwrap_or(""),
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

        // 401: 인증 실패 / 400: 서비스 찾음 (target 오류) / 200: 성공 — 모두 연결 자체는 성공
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(anyhow::anyhow!(
                "효성 ITX CDN 인증 실패: Principal/Secret을 확인하세요."
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
}
