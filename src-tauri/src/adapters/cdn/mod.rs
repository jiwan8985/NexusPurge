pub mod base;
pub mod cloudfront;

use anyhow::Result;
use crate::utils::config::AwsCredentials;

/// provider 문자열 기반 Purge (sync.rs 내부 사용 — credentials 없이 호출되는 경우)
#[allow(dead_code)]
pub async fn purge(_provider: &str, _distribution_id: &str, _paths: &[String]) -> Result<()> {
    // sync.rs에서 호출 시 credentials를 State에서 받아오도록 이후 리팩토링 예정
    Ok(())
}

/// credentials 포함 Purge (cdn.rs command에서 사용)
pub async fn purge_with_credentials(
    provider: &str,
    distribution_id: &str,
    paths: &[String],
    creds: AwsCredentials,
) -> Result<Option<String>> {
    match provider {
        "cloudfront" => {
            let adapter = cloudfront::CloudFrontAdapter::new(creds)?;
            let id = adapter.create_invalidation(distribution_id, paths).await?;
            Ok(Some(id))
        }
        "akamai" => Err(anyhow::anyhow!("Akamai CDN 어댑터는 아직 구현되지 않았습니다")),
        "lgu"    => Err(anyhow::anyhow!("LG U+ CDN 어댑터는 아직 구현되지 않았습니다")),
        "hyosung" => Err(anyhow::anyhow!("효성 CDN 어댑터는 아직 구현되지 않았습니다")),
        other    => Err(anyhow::anyhow!("알 수 없는 CDN 제공자: {}", other)),
    }
}
