pub mod akamai;
pub mod base;
pub mod cloudfront;

use anyhow::Result;
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
            let adapter = cloudfront::CloudFrontAdapter::new(aws_creds)?;
            let id = adapter.create_invalidation(distribution_id, paths).await?;
            Ok(Some(id))
        }
        CdnCredentials::Akamai {
            client_token,
            client_secret,
            access_token,
            host,
            cdn_domain,
        } => {
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
            adapter.purge_urls(&urls).await?;
            Ok(None)
        }
    }
}
