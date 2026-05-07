use anyhow::Result;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
#[allow(dead_code)]
pub struct MockCdnAdapter {
    requests: Arc<Mutex<Vec<Vec<String>>>>,
}

impl MockCdnAdapter {
    #[allow(dead_code)]
    pub fn purge_urls(&self, urls: &[String]) -> Result<()> {
        self.requests
            .lock()
            .map_err(|_| anyhow::anyhow!("mock lock poisoned"))?
            .push(urls.to_vec());
        Ok(())
    }

    #[allow(dead_code)]
    pub fn requests(&self) -> Vec<Vec<String>> {
        self.requests.lock().map(|items| items.clone()).unwrap_or_default()
    }
}

#[allow(dead_code)]
pub fn build_mock_urls(domain: &str, paths: &[String]) -> Vec<String> {
    let domain = domain
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');
    paths
        .iter()
        .map(|path| format!("https://{}/{}", domain, path.trim_start_matches('/')))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_purge_requests_without_network() {
        let adapter = MockCdnAdapter::default();
        let urls = build_mock_urls("https://cdn.example.com/", &["/a.txt".into(), "b.txt".into()]);

        adapter.purge_urls(&urls).unwrap();

        assert_eq!(adapter.requests(), vec![urls]);
    }
}
