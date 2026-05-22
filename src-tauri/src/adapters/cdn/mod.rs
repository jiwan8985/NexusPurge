pub mod akamai;
pub mod base;
pub mod cloudfront;
pub mod hyosung;
pub mod kt;
pub mod lguplus;
pub mod mock;

use crate::utils::config::CdnCredentials;
use anyhow::Result;
use std::time::Duration;

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
        } => {
            if cdn_domain.trim().is_empty() {
                return Err(anyhow::anyhow!("Akamai CDN domain is required"));
            }
            let adapter =
                akamai::AkamaiAdapter::new(client_token, client_secret, access_token, host)?;
            let urls = build_cdn_urls(&cdn_domain, paths);
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
            Err(last_err.unwrap_or_else(|| anyhow::anyhow!("Akamai purge retry failed")))
        }
        CdnCredentials::Lguplus {
            api_key,
            api_secret,
            endpoint,
            cdn_domain,
        } => {
            let adapter = lguplus::LguplusCdnAdapter::new(api_key, api_secret, endpoint);
            let urls = build_cdn_urls(&cdn_domain, paths);
            adapter.purge_urls(&urls).await?;
            Ok(None)
        }
        CdnCredentials::Hyosung {
            api_key,
            api_secret,
            endpoint,
            cdn_domain,
        } => {
            let adapter = hyosung::HyosungCdnAdapter::new(api_key, api_secret, endpoint);
            let urls = build_cdn_urls(&cdn_domain, paths);
            adapter.purge_urls(&urls).await?;
            Ok(None)
        }
        CdnCredentials::Kt {
            api_key,
            api_secret,
            endpoint,
            cdn_domain,
        } => {
            let adapter = kt::KtCdnAdapter::new(api_key, api_secret, endpoint);
            let urls = build_cdn_urls(&cdn_domain, paths);
            adapter.purge_urls(&urls).await?;
            Ok(None)
        }
    }
}

pub fn build_cdn_url(cdn_domain: &str, object_path: &str) -> String {
    let domain = cdn_domain
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');
    let path = object_path.trim_start_matches('/');
    format!("https://{}/{}", domain, path)
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
    async fn kt_provider_is_explicitly_not_implemented_until_api_spec_is_available() {
        let result = purge_with_credentials(
            "",
            &["assets/app.js".to_string()],
            CdnCredentials::Kt {
                api_key: "key".to_string(),
                api_secret: "secret".to_string(),
                endpoint: "https://api.example.com".to_string(),
                cdn_domain: "cdn.example.com".to_string(),
            },
        )
        .await;

        let err = result.expect_err("KT provider should fail until the real API is implemented");
        assert!(
            err.to_string()
                .contains("KT CDN purge API is not implemented yet")
        );
    }

    #[tokio::test]
    async fn lguplus_provider_is_explicitly_not_implemented_until_api_spec_is_available() {
        let result = purge_with_credentials(
            "",
            &["assets/app.js".to_string()],
            CdnCredentials::Lguplus {
                api_key: "key".to_string(),
                api_secret: "secret".to_string(),
                endpoint: "https://api.example.com".to_string(),
                cdn_domain: "cdn.example.com".to_string(),
            },
        )
        .await;

        let err =
            result.expect_err("LG U+ provider should fail until the real API is implemented");
        assert!(
            err.to_string()
                .contains("LG U+ CDN purge API is not implemented yet")
        );
    }

    #[tokio::test]
    async fn hyosung_provider_is_explicitly_not_implemented_until_api_spec_is_available() {
        let result = purge_with_credentials(
            "",
            &["assets/app.js".to_string()],
            CdnCredentials::Hyosung {
                api_key: "key".to_string(),
                api_secret: "secret".to_string(),
                endpoint: "https://api.example.com".to_string(),
                cdn_domain: "cdn.example.com".to_string(),
            },
        )
        .await;

        let err =
            result.expect_err("Hyosung provider should fail until the real API is implemented");
        assert!(
            err.to_string()
                .contains("Hyosung CDN purge API is not implemented yet")
        );
    }
}
