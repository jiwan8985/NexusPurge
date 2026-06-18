use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::adapters::storage::s3::S3Adapter;
use crate::services::operation_log::OperationLog;
use crate::utils::config::{AwsCredentials, ProfileStore};

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
    #[serde(rename = "targetKey")]
    pub target_key: Option<String>,
    pub attempts: usize,
    pub error: Option<String>,
}

pub struct LogShippingService;

impl LogShippingService {
    pub fn new() -> Self {
        Self
    }

    /// 로그를 고객 S3 버킷에 JSON으로 업로드한다.
    /// `target`이 None이면 즉시 반환 (disabled).
    /// 자격증명은 `store`에서 `profile_id`로 조회한다 — 로그 버킷은 동일 계정 가정.
    pub async fn ship(
        &self,
        log: &OperationLog,
        target: &LogShippingTarget,
        creds: AwsCredentials,
        region: &str,
    ) -> Result<LogShippingResult> {
        let json = serde_json::to_vec_pretty(log).context("로그 JSON 직렬화 실패")?;

        // key: {prefix}/{yyyy-MM-dd}/{log_id}.json
        let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let prefix = target.prefix.trim_end_matches('/');
        let key = if prefix.is_empty() {
            format!("{}/{}.json", date, log.id)
        } else {
            format!("{}/{}/{}.json", prefix, date, log.id)
        };

        let adapter = S3Adapter::new(region, &target.bucket, &creds, None)
            .await
            .context("로그 적재용 S3Adapter 초기화 실패")?;

        // 임시 파일 없이 바이트 직접 업로드 (put_object_bytes)
        adapter
            .put_object(&key, json, "application/json")
            .await
            .with_context(|| format!("로그 S3 업로드 실패: s3://{}/{}", target.bucket, key))?;

        Ok(LogShippingResult {
            success: true,
            target_bucket: Some(target.bucket.clone()),
            target_key: Some(key),
            attempts: 1,
            error: None,
        })
    }
}

/// `ProfileStore`에서 자격증명을 가져와 로그를 S3에 적재한다.
pub async fn ship_log_with_profile(
    log: &OperationLog,
    profile_id: &str,
    target: &LogShippingTarget,
    store: &ProfileStore,
) -> Result<LogShippingResult> {
    let (creds, region, _bucket, _endpoint) =
        store.get_connection_info(profile_id).await?;

    LogShippingService::new().ship(log, target, creds, &region).await
}

fn json_format() -> String {
    "json".to_string()
}
