use anyhow::{Context, Result};
use reqwest::Client;
use url::Url;
use uuid::Uuid;

use crate::utils::config::AwsCredentials;
use crate::utils::retry::is_retryable_status;
use crate::utils::sigv4::Signer;

pub struct CloudFrontAdapter {
    client: Client,
    creds:  AwsCredentials,
}

impl CloudFrontAdapter {
    pub fn new(creds: AwsCredentials) -> Result<Self> {
        let client = Client::builder()
            .use_native_tls()
            .build()
            .context("HTTP 클라이언트 생성 실패")?;
        Ok(Self { client, creds })
    }

    pub async fn create_invalidation(
        &self,
        distribution_id: &str,
        paths: &[String],
    ) -> Result<String> {
        let caller_ref = Uuid::new_v4().to_string();

        let items: String = paths
            .iter()
            .map(|p| {
                let normalized =
                    if p.starts_with('/') { p.clone() } else { format!("/{}", p) };
                format!("      <Path>{}</Path>", normalized)
            })
            .collect::<Vec<_>>()
            .join("\n");

        let body = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<InvalidationBatch xmlns=\"http://cloudfront.amazonaws.com/doc/2020-05-31/\">\n\
  <CallerReference>{caller_ref}</CallerReference>\n\
  <Paths>\n\
    <Quantity>{count}</Quantity>\n\
    <Items>\n{items}\n    </Items>\n\
  </Paths>\n\
</InvalidationBatch>",
            caller_ref = caller_ref,
            count      = paths.len(),
            items      = items,
        )
        .into_bytes();

        let raw_url = format!(
            "https://cloudfront.amazonaws.com/2020-05-31/distribution/{}/invalidation",
            distribution_id
        );
        let url = Url::parse(&raw_url).context("URL 파싱 실패")?;

        let signer = Signer {
            access_key_id:     &self.creds.access_key_id,
            secret_access_key: &self.creds.secret_access_key,
            region:            "us-east-1",
            service:           "cloudfront",
        };

        // M-5: 지수 백오프 재시도 (최대 3회) — CloudFront rate limit(429) 대응
        let mut delay_ms = 500u64;
        for attempt in 0u32..3 {
            // SigV4 서명은 타임스탬프 포함이므로 재시도마다 재생성
            let headers = signer.sign_headers(
                "POST",
                &url,
                &[("content-type", "application/xml")],
                &body,
            );
            let mut req = self
                .client
                .post(raw_url.clone())
                .header("content-type", "application/xml")
                .body(body.clone());
            for (k, v) in &headers {
                req = req.header(k.as_str(), v.as_str());
            }

            match req.send().await {
                Err(e) if attempt < 2 => {
                    tracing::warn!("CloudFront Invalidation 네트워크 오류 재시도 {}/3: {}", attempt + 1, e);
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                    delay_ms *= 2;
                    continue;
                }
                Err(e) => return Err(e).context("CloudFront 요청 실패"),
                Ok(resp) => {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    if status.is_success() {
                        let id = xml_extract(&text, "Id").unwrap_or(caller_ref);
                        tracing::info!("CloudFront Invalidation 생성: {} (dist={})", id, distribution_id);
                        return Ok(id);
                    }
                    let code = status.as_u16();
                    if attempt < 2 && is_retryable_status(code) {
                        tracing::warn!("CloudFront Invalidation 재시도 {}/3: HTTP {}", attempt + 1, code);
                        tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                        delay_ms *= 2;
                        continue;
                    }
                    return Err(anyhow::anyhow!("CloudFront Invalidation 실패 ({}): {}", status, text));
                }
            }
        }
        unreachable!()
    }

    /// CloudFront 배포의 도메인명 조회 (예: d111111abcdef8.cloudfront.net)
    #[allow(dead_code)]
    pub async fn get_distribution_domain(&self, distribution_id: &str) -> Result<String> {
        let raw_url = format!(
            "https://cloudfront.amazonaws.com/2020-05-31/distribution/{}",
            distribution_id
        );
        let url = Url::parse(&raw_url).context("URL 파싱 실패")?;

        let signer = Signer {
            access_key_id:     &self.creds.access_key_id,
            secret_access_key: &self.creds.secret_access_key,
            region:            "us-east-1",
            service:           "cloudfront",
        };
        let headers = signer.sign_headers("GET", &url, &[], b"");

        let mut req = self.client.get(raw_url);
        for (k, v) in &headers {
            req = req.header(k.as_str(), v.as_str());
        }

        let resp = req
            .send()
            .await
            .context("CloudFront GetDistribution 요청 실패")?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(anyhow::anyhow!(
                "GetDistribution 실패 ({}): {}",
                status,
                text
            ));
        }

        // <DomainName> 은 Distribution 블록과 Origins 양쪽에 있으므로
        // Distribution 최상위 블록에서 첫 번째 값을 추출
        let dist_block = text
            .find("<Distribution ")
            .or_else(|| text.find("<Distribution>"))
            .map(|start| &text[start..])
            .unwrap_or(&text);

        xml_extract(dist_block, "DomainName").context("배포 도메인명 파싱 실패")
    }
}

fn xml_extract(xml: &str, tag: &str) -> Option<String> {
    let open  = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = xml.find(&open)? + open.len();
    let end   = xml[start..].find(&close)? + start;
    Some(xml[start..end].to_owned())
}
