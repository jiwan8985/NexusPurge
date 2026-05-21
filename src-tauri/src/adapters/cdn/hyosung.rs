use anyhow::Result;

pub struct HyosungCdnAdapter {
    #[allow(dead_code)]
    api_key: String,
    #[allow(dead_code)]
    api_secret: String,
    #[allow(dead_code)]
    endpoint: String,
}

impl HyosungCdnAdapter {
    pub fn new(api_key: String, api_secret: String, endpoint: String) -> Self {
        Self {
            api_key,
            api_secret,
            endpoint,
        }
    }

    pub async fn purge_urls(&self, _urls: &[String]) -> Result<()> {
        Err(anyhow::anyhow!(
            "Hyosung CDN purge API is not implemented yet. API specification is required."
        ))
    }
}
