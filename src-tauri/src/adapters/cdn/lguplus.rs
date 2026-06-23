/// LG U+ CDN Purge Adapter (Solbox CDN v3)
///
/// Auth:  POST {endpoint}/v3/auth/tokens
///        Body: {"username":"...", "password":"...", "expiresIn":"1h"}
///        Response: {"token": "..."}
///
/// Purge: POST {endpoint}/v3/management/service/{serviceName}/volume/{volumeName}/purge
///        Authorization: Bearer {token}
///        Body: {"paths": ["/path1", "/path2"]}
///
/// Default endpoint: https://api.lgucdn.com
use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::Value;

pub struct LguplusCdnAdapter {
    client:       Client,
    username:     String,
    password:     String,
    service_name: String,
    volume_name:  String,
    endpoint:       String,
    #[allow(dead_code)]
    cdn_domain:     String, // 향후 URL 기반 purge 전환 시 사용
}

impl LguplusCdnAdapter {
    pub fn new(
        username:     String,
        password:     String,
        service_name: String,
        volume_name:  String,
        endpoint:     String,
        cdn_domain:   String,
    ) -> Result<Self> {
        let client = Client::builder()
            .use_native_tls()
            .build()
            .context("HTTP 클라이언트 생성 실패")?;
        let endpoint = endpoint.trim().trim_end_matches('/').to_owned();
        Ok(Self { client, username, password, service_name, volume_name, endpoint, cdn_domain })
    }

    /// JWT 토큰 발급 (v3 auth/tokens)
    async fn acquire_token(&self) -> Result<String> {
        let url = format!("{}/v3/auth/tokens", self.endpoint);
        let body = serde_json::json!({
            "username":  self.username,
            "password":  self.password,
            "expiresIn": "1h",
        })
        .to_string();

        let resp = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .context("LG U+ CDN 인증 요청 실패")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "LG U+ CDN 인증 실패 (HTTP {}): {}",
                status,
                text
            ));
        }

        let text = resp.text().await.context("LG U+ CDN 인증 응답 읽기 실패")?;
        let json: Value =
            serde_json::from_str(&text).context("LG U+ CDN 인증 응답 JSON 파싱 실패")?;

        let token = json["token"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("LG U+ CDN 인증 응답에 token 필드 없음: {}", text))?
            .to_owned();

        Ok(token)
    }

    /// CDN 경로 목록을 Purge (v3 management API)
    pub async fn purge_paths(&self, paths: &[String]) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }

        let token = self.acquire_token().await?;

        // S3 키 → 앞에 / 보장
        let normalized: Vec<String> = paths
            .iter()
            .map(|p| {
                if p.starts_with('/') {
                    p.clone()
                } else {
                    format!("/{}", p)
                }
            })
            .collect();

        // Purge by Volume: /v3/management/service/{serviceName}/volume/{volumeName}/purge
        let url = format!(
            "{}/v3/management/service/{}/volume/{}/purge",
            self.endpoint, self.service_name, self.volume_name,
        );

        let body = serde_json::json!({ "paths": normalized }).to_string();

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .context("LG U+ CDN Purge 요청 실패")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "LG U+ CDN Purge 실패 (HTTP {}): {}",
                status,
                text
            ));
        }

        // 비동기 트랜잭션 응답(202)도 성공으로 처리 — transactionId 로깅
        let text = resp.text().await.unwrap_or_default();
        if let Ok(json) = serde_json::from_str::<Value>(&text) {
            if let Some(tid) = json["transactionId"].as_str() {
                tracing::info!(
                    "LG U+ CDN Purge 요청 수락: transactionId={}, {} 경로 (서비스: {}, 볼륨: {})",
                    tid, paths.len(), self.service_name, self.volume_name,
                );
                return Ok(());
            }
        }

        tracing::info!(
            "LG U+ CDN Purge 완료: {} 경로 (서비스: {}, 볼륨: {})",
            paths.len(), self.service_name, self.volume_name,
        );
        Ok(())
    }

    /// 연결 테스트 — 토큰 발급만 확인
    pub async fn test_connection(&self) -> Result<()> {
        self.acquire_token().await?;
        Ok(())
    }
}
