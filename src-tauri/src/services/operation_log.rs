use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const OPERATION_LOGS_FILENAME: &str = "operation_logs.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationLog {
    pub id: String,
    #[serde(rename = "profileId")]
    pub profile_id: String,
    pub operation: String,
    pub status: String,
    pub bucket: Option<String>,
    pub prefix: Option<String>,
    pub files: Vec<serde_json::Value>,
    #[serde(rename = "purgeResults")]
    pub purge_results: Vec<serde_json::Value>,
    #[serde(default, rename = "metadataFailures")]
    pub metadata_failures: Vec<MetadataFailureLog>,
    #[serde(default, rename = "logShipping", skip_serializing_if = "Option::is_none")]
    pub log_shipping: Option<LogShippingState>,
    #[serde(rename = "startedAt")]
    pub started_at: String,
    #[serde(rename = "finishedAt")]
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataFailureLog {
    pub path: String,
    #[serde(default)]
    pub headers: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub metadata: serde_json::Map<String, serde_json::Value>,
    pub error: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogShippingState {
    #[serde(default, rename = "targetBucket", skip_serializing_if = "Option::is_none")]
    pub target_bucket: Option<String>,
    #[serde(default, rename = "targetPrefix", skip_serializing_if = "Option::is_none")]
    pub target_prefix: Option<String>,
    pub status: String,
    pub attempts: usize,
    #[serde(default, rename = "nextRetryAt", skip_serializing_if = "Option::is_none")]
    pub next_retry_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub struct OperationLogService {
    data_dir: PathBuf,
}

impl OperationLogService {
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    fn path(&self) -> PathBuf {
        self.data_dir.join(OPERATION_LOGS_FILENAME)
    }

    pub async fn list_recent(&self) -> Result<Vec<OperationLog>> {
        let path = self.path();
        if !path.exists() {
            return Ok(vec![]);
        }
        let content = tokio::fs::read_to_string(path)
            .await
            .context("operation log read failed")?;
        Ok(serde_json::from_str(&content).unwrap_or_default())
    }

    pub async fn get(&self, id: &str) -> Result<Option<OperationLog>> {
        Ok(self.list_recent().await?.into_iter().find(|log| log.id == id))
    }

    pub async fn save(&self, log: OperationLog) -> Result<()> {
        tokio::fs::create_dir_all(&self.data_dir)
            .await
            .context("operation log directory creation failed")?;
        let mut logs = self.list_recent().await?;
        logs.retain(|item| item.id != log.id);
        logs.insert(0, log);
        logs.truncate(500);
        tokio::fs::write(
            self.path(),
            serde_json::to_string_pretty(&logs).context("operation log serialization failed")?,
        )
        .await
        .context("operation log write failed")
    }

    pub async fn clear(&self) -> Result<()> {
        let path = self.path();
        if path.exists() {
            tokio::fs::remove_file(path)
                .await
                .context("operation log clear failed")?;
        }
        Ok(())
    }

    // TODO: Add CSV export after report columns are confirmed.
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_log(id: &str, started_at: &str) -> OperationLog {
        OperationLog {
            id: id.to_string(),
            profile_id: "profile-1".to_string(),
            operation: "upload".to_string(),
            status: "success".to_string(),
            bucket: Some("bucket".to_string()),
            prefix: Some("assets/".to_string()),
            files: vec![],
            purge_results: vec![],
            metadata_failures: vec![],
            log_shipping: None,
            started_at: started_at.to_string(),
            finished_at: Some(started_at.to_string()),
        }
    }

    #[tokio::test]
    async fn save_get_list_and_clear_operation_logs() {
        let data_dir = std::env::temp_dir().join(format!(
            "nexuspurge-operation-log-test-{}",
            uuid::Uuid::new_v4()
        ));
        let service = OperationLogService::new(data_dir.clone());

        service
            .save(sample_log("old", "2026-05-22T00:00:00Z"))
            .await
            .unwrap();
        service
            .save(sample_log("new", "2026-05-22T00:01:00Z"))
            .await
            .unwrap();

        let logs = service.list_recent().await.unwrap();
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].id, "new");

        let found = service.get("old").await.unwrap().unwrap();
        assert_eq!(found.profile_id, "profile-1");

        service.clear().await.unwrap();
        assert!(service.list_recent().await.unwrap().is_empty());

        let _ = std::fs::remove_dir_all(data_dir);
    }
}
