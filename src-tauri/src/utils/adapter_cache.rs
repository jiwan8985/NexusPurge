use std::collections::HashMap;
use tokio::sync::RwLock;
use crate::adapters::storage::s3::S3Adapter;

/// 프로필 ID별 S3Adapter 캐시 — reqwest::Client 재사용으로 연결 오버헤드 제거
pub struct AdapterCache {
    inner: RwLock<HashMap<String, S3Adapter>>,
}

impl AdapterCache {
    pub fn new() -> Self {
        Self { inner: RwLock::new(HashMap::new()) }
    }

    /// 캐시 히트 시 기존 어댑터 반환, 미스 시 factory로 생성 후 캐싱
    pub async fn get_or_create(
        &self,
        profile_id: &str,
        factory: impl FnOnce() -> anyhow::Result<S3Adapter>,
    ) -> anyhow::Result<S3Adapter> {
        {
            let map = self.inner.read().await;
            if let Some(adapter) = map.get(profile_id) {
                return Ok(adapter.clone());
            }
        }
        let mut map = self.inner.write().await;
        if let Some(adapter) = map.get(profile_id) {
            return Ok(adapter.clone());
        }
        let adapter = factory()?;
        map.insert(profile_id.to_owned(), adapter.clone());
        Ok(adapter)
    }

    /// 프로필 변경 또는 삭제 시 캐시 무효화
    pub async fn invalidate(&self, profile_id: &str) {
        self.inner.write().await.remove(profile_id);
    }
}
