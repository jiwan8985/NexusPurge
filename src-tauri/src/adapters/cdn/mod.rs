pub mod akamai;
pub mod base;
pub mod cloudfront;
pub mod hyosung;
pub mod kt;
pub mod lguplus;
pub mod mock;

use crate::utils::config::CdnCredentials;
use anyhow::Result;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
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
            username,
            password,
            service_name,
            volume_name,
            endpoint,
            cdn_domain,
        } => {
            let adapter = lguplus::LguplusCdnAdapter::new(
                username, password, service_name, volume_name, endpoint, cdn_domain,
            )?;
            let mut last_err = None;
            for attempt in 0..3 {
                match adapter.purge_paths(paths).await {
                    Ok(()) => return Ok(None),
                    Err(err) if attempt < 2 => {
                        last_err = Some(err);
                        tokio::time::sleep(retry_delay(attempt)).await;
                    }
                    Err(err) => return Err(err),
                }
            }
            Err(last_err.unwrap_or_else(|| anyhow::anyhow!("LG U+ purge retry failed")))
        }
        CdnCredentials::Hyosung {
            api_key,
            api_secret,
            endpoint,
            cdn_domain,
        } => {
            if distribution_id.trim().is_empty() {
                return Err(anyhow::anyhow!(
                    "효성 ITX CDN Service ID가 필요합니다 (프로필의 Distribution ID 필드에 입력)"
                ));
            }
            if cdn_domain.trim().is_empty() {
                return Err(anyhow::anyhow!(
                    "효성 ITX CDN Domain이 필요합니다"
                ));
            }
            let adapter = hyosung::HyosungCdnAdapter::new(
                api_key,
                api_secret,
                endpoint,
                distribution_id.to_owned(),
                cdn_domain,
            )?;
            let mut last_err = None;
            for attempt in 0..3 {
                match adapter.purge_paths(paths).await {
                    Ok(()) => return Ok(None),
                    Err(err) if attempt < 2 => {
                        last_err = Some(err);
                        tokio::time::sleep(retry_delay(attempt)).await;
                    }
                    Err(err) => return Err(err),
                }
            }
            Err(last_err.unwrap_or_else(|| anyhow::anyhow!("Hyosung purge retry failed")))
        }
        CdnCredentials::Kt {
            username,
            password,
            service_name,
            volume_name,
            endpoint,
            cdn_domain,
        } => {
            let adapter = kt::KtCdnAdapter::new(
                username, password, service_name, volume_name, endpoint, cdn_domain,
            )?;
            let mut last_err = None;
            for attempt in 0..3 {
                match adapter.purge_paths(paths).await {
                    Ok(()) => return Ok(None),
                    Err(err) if attempt < 2 => {
                        last_err = Some(err);
                        tokio::time::sleep(retry_delay(attempt)).await;
                    }
                    Err(err) => return Err(err),
                }
            }
            Err(last_err.unwrap_or_else(|| anyhow::anyhow!("KT purge retry failed")))
        }
    }
}

pub fn build_cdn_url(cdn_domain: &str, object_path: &str) -> String {
    let domain = cdn_domain
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');

    // 경로를 슬래시 단위로 분리하여 각 세그먼트만 percent-encode
    // (슬래시 자체는 그대로 유지, 한글/공백/특수문자만 인코딩)
    const PATH_SAFE: &percent_encoding::AsciiSet = &NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'_')
        .remove(b'.')
        .remove(b'~');

    let raw = object_path.trim_start_matches('/');
    let encoded = raw
        .split('/')
        .map(|seg| utf8_percent_encode(seg, PATH_SAFE).to_string())
        .collect::<Vec<_>>()
        .join("/");

    format!("https://{}/{}", domain, encoded)
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
    async fn hyosung_requires_service_id() {
        // distribution_id(serviceId) 없이 호출하면 명확한 오류 반환
        let result = purge_with_credentials(
            "",
            &["assets/app.js".to_string()],
            CdnCredentials::Hyosung {
                api_key:    "key".to_string(),
                api_secret: "secret".to_string(),
                endpoint:   "https://api.xtrmcdn.co.kr:28091".to_string(),
                cdn_domain: "cdn.example.com".to_string(),
            },
        )
        .await;

        let err = result.expect_err("serviceId 없이 호출 시 오류여야 함");
        assert!(
            err.to_string().contains("Service ID"),
            "오류 메시지에 Service ID 언급 필요: {}",
            err
        );
    }

    #[tokio::test]
    async fn hyosung_requires_cdn_domain() {
        let result = purge_with_credentials(
            "TID_18656",
            &["assets/app.js".to_string()],
            CdnCredentials::Hyosung {
                api_key:    "key".to_string(),
                api_secret: "secret".to_string(),
                endpoint:   "https://api.xtrmcdn.co.kr:28091".to_string(),
                cdn_domain: "".to_string(),
            },
        )
        .await;

        let err = result.expect_err("cdn_domain 없이 호출 시 오류여야 함");
        assert!(
            err.to_string().contains("Domain"),
            "오류 메시지에 Domain 언급 필요: {}",
            err
        );
    }
}
