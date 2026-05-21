use anyhow::Result;

pub struct LguplusCdnAdapter {
    #[allow(dead_code)]
    api_key: String,
    #[allow(dead_code)]
    api_secret: String,
    #[allow(dead_code)]
    endpoint: String,
}

impl LguplusCdnAdapter {
    pub fn new(api_key: String, api_secret: String, endpoint: String) -> Self {
        Self {
            api_key,
            api_secret,
            endpoint,
        }
    }

    pub async fn purge_urls(&self, _urls: &[String]) -> Result<()> {
        Err(anyhow::anyhow!(
            "LG U+ CDN purge API is not implemented yet. API specification is required."
        ))
    }
}
