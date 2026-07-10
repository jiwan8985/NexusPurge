/// LG U+ CDN Purge Adapter (CDN v3 — https://v3-api-docs.lgucdn.com/)
///
/// Auth:  POST {endpoint}/v3/auth/tokens
///        Body: {"username":"...", "password":"...", "expiresIn":"1h"}
///        Response: {"token": "..."}
///
/// Purge: POST {endpoint}/v3/management/service/{serviceName}/volume/{volumeName}/purge
///        POST {endpoint}/v3/management/service/{serviceName}/domain/{domain}/purge
///        Authorization: Bearer {token}
///        Body: {"filelist": ["/path1", "/path2"]}  (응답: {"transid": <number>})
///        Volume Name이 있으면 volume 기반, 없으면 Edge Domain 기반으로 Purge
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
    cdn_domain:     String, // FQDN — volume_name 미지정 시 domain 기반 purge에 사용
    service_type:   String, // "cloudcdn" | "volume" — cloudcdn만 Purge by Service 가능
}

/// `/v3/management/service/{serviceName}/purge` URL 구성 (전체 서비스 즉시 플러시, body 없음)
pub(crate) fn service_purge_url(endpoint: &str, service_name: &str) -> String {
    format!("{}/v3/management/service/{}/purge", endpoint.trim_end_matches('/'), service_name)
}

impl LguplusCdnAdapter {
    pub fn new(
        username:     String,
        password:     String,
        service_name: String,
        volume_name:  String,
        endpoint:     String,
        cdn_domain:   String,
        service_type: String,
    ) -> Result<Self> {
        let client = Client::builder()
            .use_native_tls()
            .build()
            .context("HTTP 클라이언트 생성 실패")?;
        let endpoint = endpoint.trim().trim_end_matches('/').to_owned();
        Ok(Self { client, username, password, service_name, volume_name, endpoint, cdn_domain, service_type })
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
            .or_else(|| json["accessToken"].as_str())
            .or_else(|| json["access_token"].as_str())
            .or_else(|| json["data"]["token"].as_str())
            .or_else(|| json["data"]["accessToken"].as_str())
            .ok_or_else(|| anyhow::anyhow!("LG U+ CDN 인증 응답에 token 필드 없음: {}", text))?
            .to_owned();

        Ok(token)
    }

    /// CDN 경로 목록을 Purge (v3 management API)
    pub async fn purge_paths(&self, paths: &[String]) -> Result<Option<String>> {
        if paths.is_empty() {
            return Ok(None);
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

        if self.service_name.trim().is_empty() {
            return Err(anyhow::anyhow!("LG U+ CDN Purge에는 Service Name이 필요합니다"));
        }

        // Volume Name이 있으면 volume 기반, 없으면 CDN 도메인(FQDN) 기반 Purge
        let url = if !self.volume_name.trim().is_empty() {
            format!(
                "{}/v3/management/service/{}/volume/{}/purge",
                self.endpoint, self.service_name, self.volume_name,
            )
        } else {
            let domain = self
                .cdn_domain
                .trim()
                .trim_start_matches("https://")
                .trim_start_matches("http://")
                .trim_end_matches('/');
            if domain.is_empty() {
                return Err(anyhow::anyhow!(
                    "LG U+ CDN Purge에는 Volume Name 또는 Edge Domain이 필요합니다"
                ));
            }
            format!(
                "{}/v3/management/service/{}/domain/{}/purge",
                self.endpoint, self.service_name, domain,
            )
        };

        // 공식 스펙(v3-api-docs Postman 컬렉션): {"filelist": ["/path", ...]}
        // 기본 invalidate 방식, delete 방식은 "purge_type":"HARD" 추가
        let body = serde_json::json!({ "filelist": normalized }).to_string();

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
            let tid = json["transactionId"]
                .as_str()
                .or_else(|| json["transaction_id"].as_str())
                .or_else(|| json["data"]["transactionId"].as_str())
                .or_else(|| json["transid"].as_str())
                .map(ToOwned::to_owned)
                .or_else(|| json["transid"].as_u64().map(|v| v.to_string()));
            if let Some(tid) = tid.as_deref() {
                tracing::info!(
                    "LG U+ CDN Purge 요청 수락: transactionId={}, {} 경로 (서비스: {}, 볼륨: {})",
                    tid, paths.len(), self.service_name, self.volume_name,
                );
                return Ok(Some(tid.to_owned()));
            }
        }

        tracing::info!(
            "LG U+ CDN Purge 완료: {} 경로 (서비스: {}, 볼륨: {})",
            paths.len(), self.service_name, self.volume_name,
        );
        Ok(None)
    }

    /// 서비스 전체 즉시 플러시 — Delivery-cloudcdn 타입 서비스에서만 지원
    /// (LG U+ CDN 3.0 OpenAPI v3 문서 "Purge by Service": 그 외 타입은 Purge by Volume 사용)
    /// body 없음 — 응답은 filelist 기반 응답과 동일한 형태(transactionId 포함)로 가정한다.
    pub async fn purge_service(&self) -> Result<Option<String>> {
        if self.service_type != "cloudcdn" {
            return Err(anyhow::anyhow!(
                "LG U+ CDN 서비스 전체 Purge는 서비스 타입이 cloudcdn인 경우에만 지원됩니다 \
                 (현재: {}). 프로필에서 서비스 타입을 확인하세요.",
                self.service_type
            ));
        }
        if self.service_name.trim().is_empty() {
            return Err(anyhow::anyhow!("LG U+ CDN Purge에는 Service Name이 필요합니다"));
        }

        let token = self.acquire_token().await?;
        let url = service_purge_url(&self.endpoint, &self.service_name);

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .context("LG U+ CDN 서비스 전체 Purge 요청 실패")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "LG U+ CDN 서비스 전체 Purge 실패 (HTTP {}): {}",
                status,
                text
            ));
        }

        let text = resp.text().await.unwrap_or_default();
        if let Ok(json) = serde_json::from_str::<Value>(&text) {
            let tid = json["transactionId"]
                .as_str()
                .or_else(|| json["transid"].as_str())
                .map(ToOwned::to_owned)
                .or_else(|| json["transid"].as_u64().map(|v| v.to_string()));
            if let Some(tid) = tid {
                tracing::info!("LG U+ CDN 서비스 전체 Purge 요청 수락: transactionId={}", tid);
                return Ok(Some(tid));
            }
        }

        tracing::info!("LG U+ CDN 서비스 전체 Purge 완료 (서비스: {})", self.service_name);
        Ok(None)
    }

    /// 트랜잭션 상태 조회 (v3 management/transaction/{transactionId})
    pub async fn get_transaction_status(&self, transaction_id: &str) -> Result<String> {
        let token = self.acquire_token().await?;
        let url = format!(
            "{}/v3/management/transaction/{}",
            self.endpoint, transaction_id
        );

        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .context("LG U+ CDN 트랜잭션 상태 요청 실패")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "LG U+ CDN 트랜잭션 상태 조회 실패 (HTTP {}): {}",
                status,
                text
            ));
        }

        let text = resp.text().await.context("LG U+ CDN 트랜잭션 상태 응답 읽기 실패")?;
        let json: Value =
            serde_json::from_str(&text).context("LG U+ CDN 트랜잭션 상태 응답 JSON 파싱 실패")?;

        let status = json["status"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("LG U+ CDN 트랜잭션 상태 응답에 status 필드 없음: {}", text))?
            .to_owned();

        Ok(status)
    }

    /// 연결 테스트 — 토큰 발급만 확인
    pub async fn test_connection(&self) -> Result<()> {
        self.acquire_token().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_purge_url_has_no_trailing_slash() {
        let url = service_purge_url("https://api.lgucdn.com/", "my-service");
        assert_eq!(url, "https://api.lgucdn.com/v3/management/service/my-service/purge");
    }
}
