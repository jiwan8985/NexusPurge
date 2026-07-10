use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const OPERATION_LOGS_FILENAME: &str = "operation_logs.json";
const LOG_FILES_DIR: &str = "logs";

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

    /// 날짜별 텍스트 로그 파일이 쌓이는 폴더 (예: .../cdn-upload-tool/logs)
    pub fn log_files_dir(&self) -> PathBuf {
        self.data_dir.join(LOG_FILES_DIR)
    }

    /// 고객사 요청: 로그를 타입별로 분리 저장 — 하나의 작업(예: 업로드+Purge)도
    /// 파일 전송 결과는 transfer 로그로, CDN Purge 결과는 cdn 로그로 나뉘어 기록된다.
    async fn append_typed_log(&self, kind: &str, now: chrono::DateTime<chrono::Local>, content: &str) -> Result<()> {
        let dir = self.log_files_dir();
        tokio::fs::create_dir_all(&dir)
            .await
            .context("log files directory creation failed")?;
        let file_path = dir.join(format!("{}-{}.log", kind, now.format("%Y-%m-%d")));

        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .await
            .context("log file open failed")?;
        file.write_all(content.as_bytes())
            .await
            .context("log file write failed")
    }

    /// 사람이 읽을 수 있는 한 줄 요약 + 상세를 타입별(system/transfer/cdn) 로그 파일에 append
    async fn append_log_file(&self, log: &OperationLog) -> Result<()> {
        let now = chrono::Local::now();
        let ts = now.format("%Y-%m-%d %H:%M:%S");

        // ── 파일 작업 로그 (upload/download → transfer, mkdir/rename/delete → system) ──
        if !log.files.is_empty() {
            let kind = if log.operation == "upload" || log.operation == "download" {
                "transfer"
            } else {
                "system"
            };
            let mut lines = Vec::new();
            lines.push(format!(
                "[{}] {} {} | bucket={} prefix={} files={}",
                ts,
                log.operation.to_uppercase(),
                log.status.to_uppercase(),
                log.bucket.as_deref().unwrap_or("-"),
                log.prefix.as_deref().unwrap_or("-"),
                log.files.len(),
            ));
            for file in &log.files {
                let path = file.get("path").and_then(|v| v.as_str()).unwrap_or("-");
                let status = file.get("status").and_then(|v| v.as_str()).unwrap_or("-");
                let started = file.get("startedAt").and_then(|v| v.as_str()).unwrap_or("-");
                let finished = file.get("finishedAt").and_then(|v| v.as_str()).unwrap_or("-");
                let error = file.get("error").and_then(|v| v.as_str());
                // 보안팀 감사/오류 추적용: 파일별 정확한 시작·종료 시각을 함께 기록
                let mut line = format!("  - {} [{}] started={} finished={}", path, status, started, finished);
                if let Some(err) = error {
                    line.push_str(&format!(" error: {}", err));
                }
                lines.push(line);
            }
            lines.push(String::new());
            let mut content = lines.join("\n");
            content.push('\n');
            self.append_typed_log(kind, now, &content).await?;
        }

        // ── CDN Purge 상세 로그 (provider·경로수·요청ID·전체 오류 메시지) ──
        if !log.purge_results.is_empty() {
            let mut lines = Vec::new();
            lines.push(format!(
                "[{}] {} {} | bucket={} prefix={} purge_batches={}",
                ts,
                log.operation.to_uppercase(),
                log.status.to_uppercase(),
                log.bucket.as_deref().unwrap_or("-"),
                log.prefix.as_deref().unwrap_or("-"),
                log.purge_results.len(),
            ));
            const URL_PREVIEW_LIMIT: usize = 50;
            for purge in &log.purge_results {
                let provider = purge.get("provider").and_then(|v| v.as_str()).unwrap_or("-");
                let status = purge.get("status").and_then(|v| v.as_str()).unwrap_or("-");
                let request_id = purge.get("requestId").and_then(|v| v.as_str());
                let request_endpoint = purge.get("requestEndpoint").and_then(|v| v.as_str());
                let duration_ms = purge.get("durationMs").and_then(|v| v.as_u64());
                let started = purge.get("startedAt").and_then(|v| v.as_str()).unwrap_or("-");
                let finished = purge.get("finishedAt").and_then(|v| v.as_str()).unwrap_or("-");
                let error = purge.get("error").and_then(|v| v.as_str());
                let urls = purge.get("urls").and_then(|v| v.as_array());
                let url_count = urls.map(|u| u.len()).unwrap_or(0);

                let mut line = format!(
                    "  * [{}] {} urls={} started={} finished={}",
                    provider, status, url_count, started, finished
                );
                if let Some(id) = request_id {
                    line.push_str(&format!(" requestId={}", id));
                }
                if let Some(ms) = duration_ms {
                    line.push_str(&format!(" duration={}ms", ms));
                }
                lines.push(line);
                // 실제 호출된 CDN API 엔드포인트 (감사/디버깅용)
                if let Some(endpoint) = request_endpoint {
                    lines.push(format!("      endpoint: {}", endpoint));
                }
                // 대상 경로 목록 (최대 URL_PREVIEW_LIMIT개 — 전량은 JSON 로그(operation_logs.json)에서 확인)
                if let Some(urls) = urls {
                    for u in urls.iter().take(URL_PREVIEW_LIMIT).filter_map(|v| v.as_str()) {
                        lines.push(format!("      - {}", u));
                    }
                    if url_count > URL_PREVIEW_LIMIT {
                        lines.push(format!("      ... 외 {}개 (전체 목록은 operation_logs.json 참고)", url_count - URL_PREVIEW_LIMIT));
                    }
                }
                // 오류 메시지는 CDN이 반환한 상세(HTTP 상태·응답 본문 등)를 그대로 전체 기록
                if let Some(err) = error {
                    lines.push(format!("      error: {}", err));
                }
            }
            lines.push(String::new());
            let mut content = lines.join("\n");
            content.push('\n');
            self.append_typed_log("cdn", now, &content).await?;
        }

        Ok(())
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

        // 날짜별 텍스트 로그 파일에도 기록 (실패해도 JSON 저장은 계속)
        if let Err(err) = self.append_log_file(&log).await {
            tracing::warn!("operation log file append failed: {}", err);
        }

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

    /// 고객사 요청 검증: 로그 타입별 파일 분리
    /// upload/download → transfer-*.log, mkdir/rename/delete → system-*.log,
    /// CDN Purge 결과 → cdn-*.log (한 작업의 전송/퍼지 결과가 각각의 파일로 나뉨)
    #[tokio::test]
    async fn append_log_file_splits_by_type() {
        let data_dir = std::env::temp_dir().join(format!(
            "nexuspurge-typed-log-test-{}",
            uuid::Uuid::new_v4()
        ));
        let service = OperationLogService::new(data_dir.clone());
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();

        // 1) 업로드 + CDN Purge가 섞인 작업 → transfer + cdn 두 파일에 분리 기록
        let mut upload = sample_log("upload-1", "2026-07-10T00:00:00Z");
        upload.files = vec![serde_json::json!({
            "path": "assets/app.js",
            "status": "error",
            "startedAt": "2026-07-10T00:00:00Z",
            "finishedAt": "2026-07-10T00:00:03Z",
            "error": "커넥션 끊김",
        })];
        upload.purge_results = vec![serde_json::json!({
            "provider": "cloudfront",
            "status": "failed",
            "startedAt": "2026-07-10T00:00:03Z",
            "finishedAt": "2026-07-10T00:00:08Z",
            "durationMs": 5000,
            "urls": ["assets/app.js"],
            "error": "HTTP 503 from CDN",
        })];
        service.save(upload).await.unwrap();

        // 2) 폴더 생성(mkdir) → system 파일에 기록
        let mut mkdir = sample_log("mkdir-1", "2026-07-10T00:01:00Z");
        mkdir.operation = "mkdir".to_string();
        mkdir.files = vec![serde_json::json!({
            "path": "new-folder/",
            "status": "success",
            "startedAt": "2026-07-10T00:01:00Z",
            "finishedAt": "2026-07-10T00:01:01Z",
        })];
        service.save(mkdir).await.unwrap();

        let logs_dir = service.log_files_dir();
        let transfer = std::fs::read_to_string(logs_dir.join(format!("transfer-{}.log", today))).unwrap();
        let cdn      = std::fs::read_to_string(logs_dir.join(format!("cdn-{}.log", today))).unwrap();
        let system   = std::fs::read_to_string(logs_dir.join(format!("system-{}.log", today))).unwrap();

        // transfer: 파일 전송 결과 + 시작/종료 시각 + 오류 메시지 (감사 추적용)
        assert!(transfer.contains("UPLOAD"));
        assert!(transfer.contains("assets/app.js"));
        assert!(transfer.contains("started=2026-07-10T00:00:00Z"));
        assert!(transfer.contains("finished=2026-07-10T00:00:03Z"));
        assert!(transfer.contains("error: 커넥션 끊김"));
        // transfer 파일에는 CDN Purge 상세가 섞이지 않음
        assert!(!transfer.contains("cloudfront"));

        // cdn: provider·소요시간·대상 경로·전체 오류 (Purge 지연/실패 추적용)
        assert!(cdn.contains("cloudfront"));
        assert!(cdn.contains("duration=5000ms"));
        assert!(cdn.contains("error: HTTP 503 from CDN"));
        assert!(cdn.contains("started=2026-07-10T00:00:03Z"));

        // system: mkdir 등 파일 관리 작업만
        assert!(system.contains("MKDIR"));
        assert!(system.contains("new-folder/"));
        assert!(!system.contains("UPLOAD"));

        let _ = std::fs::remove_dir_all(data_dir);
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
