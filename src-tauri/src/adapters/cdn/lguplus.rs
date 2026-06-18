/// LG U+ CDN Purge Adapter (Solbox CDN v2)
///
/// Auth:  POST {endpoint}/v2/auth/token?expires_in=1h
///        Body: {"username": "...", "password": "..."}
///        Response: {"token": "..."}
///
/// Purge: POST {endpoint}/v2/service/service/{service_name}/purge
///        Authorization: Bearer {token}
///        Body: {"domain": "fqdn", "paths": ["/path1", "/path2"]}
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
    endpoint:     String, // e.g. https://api.lgucdn.com
    cdn_domain:   String, // FQDN for purge URL
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
        let endpoint = endpoint
            .trim()
            .trim_end_matches('/')
            .to_owned();
        Ok(Self { client, username, password, service_name, volume_name, endpoint, cdn_domain })
    }

    /// JWT 토큰 발급 (1시간 유효)
    async fn acquire_token(&self) -> Result<String> {
        let url = format!("{}/v2/auth/token?expires_in=1h", self.endpoint);
        let body = serde_json::json!({
            "username": self.username,
            "password": self.password,
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
        let json: Value = serde_json::from_str(&text)
            .context("LG U+ CDN 인증 응답 JSON 파싱 실패")?;

        let token = json["token"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("LG U+ CDN 인증 응답에 token 필드 없음: {}", text))?
            .to_owned();

        Ok(token)
    }

    /// CDN 경로 목록을 Purge
    pub async fn purge_paths(&self, paths: &[String]) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }

        let token = self.acquire_token().await?;

        // paths는 S3 키 기반. 앞에 / 보장
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

        let url = format!(
            "{}/v2/service/service/{}/purge",
            self.endpoint,
            self.service_name
        );

        let body = serde_json::json!({
            "domain": self.cdn_domain,
            "paths":  normalized,
            "volumeName": self.volume_name,
        })
        .to_string();

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

        tracing::info!(
            "LG U+ CDN Purge 성공: {} 경로 (서비스: {})",
            paths.len(),
            self.service_name
        );
        Ok(())
    }

    /// 연결 테스트 — 토큰 발급만 확인
    pub async fn test_connection(&self) -> Result<()> {
        self.acquire_token().await?;
        Ok(())
    }
}
