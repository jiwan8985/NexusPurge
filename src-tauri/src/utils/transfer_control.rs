use std::collections::HashSet;
use tokio::sync::RwLock;

#[derive(Default)]
pub struct TransferControl {
    cancelled: RwLock<HashSet<String>>,
}

impl TransferControl {
    pub async fn cancel(&self, id: &str) {
        self.cancelled.write().await.insert(id.to_owned());
    }

    pub async fn clear(&self, id: &str) {
        self.cancelled.write().await.remove(id);
    }

    pub async fn is_cancelled(&self, id: &str) -> bool {
        self.cancelled.read().await.contains(id)
    }
}
