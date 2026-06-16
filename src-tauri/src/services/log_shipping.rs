use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::services::operation_log::OperationLog;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogShippingTarget {
    pub bucket: String,
    pub prefix: String,
    #[serde(default = "json_format")]
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogShippingResult {
    pub success: bool,
    #[serde(rename = "targetBucket")]
    pub target_bucket: Option<String>,
    #[serde(rename = "targetPrefix")]
    pub target_prefix: Option<String>,
    pub attempts: usize,
    pub error: Option<String>,
}

pub struct LogShippingService;

impl LogShippingService {
    pub fn new() -> Self {
        Self
    }

    pub async fn ship_json_log(
        &self,
        _log: OperationLog,
        target: Option<LogShippingTarget>,
    ) -> Result<LogShippingResult> {
        let Some(target) = target else {
            return Ok(LogShippingResult {
                success: false,
                target_bucket: None,
                target_prefix: None,
                attempts: 0,
                error: Some("Log shipping target is not configured.".to_string()),
            });
        };

        Err(anyhow!(
            "Customer S3 log shipping is a stub until bucket policy, prefix rules, and retry requirements are confirmed. Target: s3://{}/{}",
            target.bucket,
            target.prefix
        ))
    }
}

fn json_format() -> String {
    "json".to_string()
}
