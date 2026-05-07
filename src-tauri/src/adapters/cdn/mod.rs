pub mod akamai;
pub mod base;
pub mod cloudfront;
pub mod mock;

use anyhow::Result;
use std::time::Duration;
use crate::utils::config::CdnCredentials;

/// H-6: CdnCredentials 기반 Purge — provider 구분은 enum variant로 처리
/// distribution_id: CloudFront 전용 (Akamai는 URL 기반이므로 무시)
pub async fn purge_with_credentials(
    distribution_id: &str,
    paths: &[String],
    creds: CdnCredentials,
) -> Result<Option<String>> {
    match creds {
        CdnCredentials::CloudFront(aws_creds) => {
            if distribution_id.trim().is_empty() {
                return Err(anyhow::anyhow!("CloudFront Distribution ID가 필요합니다"));
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
            Err(last_err.unwrap_or_else(|| anyhow::anyhow!("CloudFront Purge retry 실패")))
        }
        CdnCredentials::Akamai {
            client_token,
            client_secret,
            access_token,
            host,
            cdn_domain,
        } => {
            if cdn_domain.trim().is_empty() {
                return Err(anyhow::anyhow!("Akamai CDN 도메인이 필요합니다"));
            }
            let adapter = akamai::AkamaiAdapter::new(
                client_token,
                client_secret,
                access_token,
                host,
            )?;
            // S3 key → CDN URL 변환 (cdn_domain + "/" + key)
            let urls: Vec<String> = paths
                .iter()
                .map(|p| {
                    let domain = cdn_domain.trim_end_matches('/');
                    let key = p.trim_start_matches('/');
                    format!("https://{}/{}", domain, key)
                })
                .collect();
            let mut last_err = None;
            for attempt in 0..3 {
                match adapter.purge_urls(&urls).await {
                    Ok(()) => return Ok(None),
                    Err(err) if attempt < 2 => {
                        last_err = Some(err);
                        tokio::time::sleep(retry_delay(attempt)).await;
                    }
                    Err(err) => return Err(err),
                }
            }
            Err(last_err.unwrap_or_else(|| anyhow::anyhow!("Akamai Purge retry 실패")))
        }
    }
}

fn retry_delay(attempt: usize) -> Duration {
    Duration::from_millis(250 * 2_u64.pow(attempt as u32))
}
