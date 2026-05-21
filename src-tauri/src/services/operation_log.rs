#![allow(dead_code)]

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
    #[serde(rename = "startedAt")]
    pub started_at: String,
    #[serde(rename = "finishedAt")]
    pub finished_at: Option<String>,
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

    // TODO: Add CSV export after report columns are confirmed.
}
