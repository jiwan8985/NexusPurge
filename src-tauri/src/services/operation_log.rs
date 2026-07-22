use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

const OPERATION_LOGS_FILENAME: &str = "operation_logs.json";
const LOG_FILES_DIR: &str = "logs";
/// system/transfer/cdn-*.log 보관 기간 — 고객사 환경에서 로그가 무한정 쌓이지 않도록 앱 시작 시 정리
const LOG_RETENTION_DAYS: i64 = 30;
const TYPED_LOG_KINDS: [&str; 3] = ["system-", "transfer-", "cdn-"];

/// system-*.log / transfer-*.log의 파일 1건을 한 줄로 렌더링 (경로·상태·시작/종료 시각·오류)
fn format_file_line(file: &serde_json::Value) -> String {
    let path = file.get("path").and_then(|v| v.as_str()).unwrap_or("-");
    let status = file.get("status").and_then(|v| v.as_str()).unwrap_or("-");
    let started = file.get("startedAt").and_then(|v| v.as_str()).unwrap_or("-");
    let finished = file.get("finishedAt").and_then(|v| v.as_str()).unwrap_or("-");
    let error = file.get("error").and_then(|v| v.as_str());
    let mut line = format!("  - {} [{}] started={} finished={}", path, status, started, finished);
    if let Some(err) = error {
        line.push_str(&format!(" error: {}", err));
    }
    line
}

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
            // 대량 작업(파일 수가 임계값 초과)은 파일마다 한 줄씩 쓰면 사실상 못 읽는
            // 로그가 되므로, 상태별 건수 + 시간 범위로 요약하고 실패/기타 상태 건만 개별 나열한다.
            // 전체 파일 목록은 손실 없이 operation_logs.json에 무제한 보관된다.
            const FILE_SUMMARY_THRESHOLD: usize = 30;
            const FILE_LIST_LIMIT: usize = 50;
            if log.files.len() > FILE_SUMMARY_THRESHOLD {
                let mut status_counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
                let mut min_started: Option<&str> = None;
                let mut max_finished: Option<&str> = None;
                for file in &log.files {
                    let status = file.get("status").and_then(|v| v.as_str()).unwrap_or("-");
                    *status_counts.entry(status).or_insert(0) += 1;
                    if let Some(s) = file.get("startedAt").and_then(|v| v.as_str()) {
                        min_started = Some(min_started.map_or(s, |m| if s < m { s } else { m }));
                    }
                    if let Some(f) = file.get("finishedAt").and_then(|v| v.as_str()) {
                        max_finished = Some(max_finished.map_or(f, |m| if f > m { f } else { m }));
                    }
                }
                let summary = status_counts
                    .iter()
                    .map(|(status, count)| format!("{} {}건", status, count))
                    .collect::<Vec<_>>()
                    .join(", ");
                lines.push(format!(
                    "  요약: {} (started={} ~ finished={})",
                    summary,
                    min_started.unwrap_or("-"),
                    max_finished.unwrap_or("-"),
                ));

                let non_success: Vec<&serde_json::Value> = log
                    .files
                    .iter()
                    .filter(|f| f.get("status").and_then(|v| v.as_str()) != Some("success"))
                    .collect();
                for file in non_success.iter().take(FILE_LIST_LIMIT) {
                    lines.push(format_file_line(file));
                }
                if non_success.len() > FILE_LIST_LIMIT {
                    lines.push(format!(
                        "  ... 외 {}건 실패/기타 상태 (전체 목록은 operation_logs.json 참고)",
                        non_success.len() - FILE_LIST_LIMIT
                    ));
                }
            } else {
                for file in &log.files {
                    lines.push(format_file_line(file));
                }
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
                // 이 Purge 요청 중 실제 발생한 HTTP 호출 단계 (인증 → purge 등) —
                // 상태코드·소요시간·응답 요약을 순서대로 기록해 "왜 실패했는지"를 이 파일 하나로 추적 가능하게 한다.
                if let Some(steps) = purge.get("requestSteps").and_then(|v| v.as_array()) {
                    if !steps.is_empty() {
                        lines.push("      steps:".to_string());
                        for (i, step) in steps.iter().enumerate() {
                            let method = step.get("method").and_then(|v| v.as_str()).unwrap_or("-");
                            let step_url = step.get("url").and_then(|v| v.as_str()).unwrap_or("-");
                            let step_status = step.get("status").and_then(|v| v.as_u64()).unwrap_or(0);
                            let status_text = step.get("statusText").and_then(|v| v.as_str()).unwrap_or("");
                            let elapsed = step.get("elapsedMs").and_then(|v| v.as_u64()).unwrap_or(0);
                            let summary = step.get("summary").and_then(|v| v.as_str()).unwrap_or("-");
                            lines.push(format!(
                                "        {}. {} {} → HTTP {} {} ({}ms) 응답: {}",
                                i + 1, method, step_url, step_status, status_text, elapsed, summary
                            ));
                        }
                    }
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

    /// logs/ 폴더의 날짜별 텍스트 로그(system-*.log / transfer-*.log / cdn-*.log)를 정리한다.
    /// - 오늘 날짜가 아닌 `.log` 파일은 `.log.gz`로 압축해 디스크 사용량을 줄인다.
    /// - LOG_RETENTION_DAYS(30일)보다 오래된 파일은 `.log`/`.log.gz` 상관없이 삭제한다.
    /// audit-*.log는 tracing-appender의 max_log_files가 자기 prefix/suffix로 파일 개수를 세어
    /// 자체 로테이션하므로, 확장자를 바꾸면 그 카운팅이 깨질 수 있어 여기서는 건드리지 않는다.
    pub async fn cleanup_old_logs(&self) -> Result<usize> {
        let dir = self.log_files_dir();
        if !dir.exists() {
            return Ok(0);
        }
        let today = chrono::Local::now().date_naive();
        let cutoff = today - chrono::Duration::days(LOG_RETENTION_DAYS);
        let mut removed = 0usize;
        let mut entries = tokio::fs::read_dir(&dir)
            .await
            .context("log directory read failed")?;
        while let Some(entry) = entries
            .next_entry()
            .await
            .context("log directory entry read failed")?
        {
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else { continue };
            if !TYPED_LOG_KINDS.iter().any(|prefix| file_name.starts_with(prefix)) {
                continue;
            }
            let Some((stem, is_gz)) = strip_log_suffix(file_name) else { continue };
            if stem.len() < 10 {
                continue;
            }
            let date_str = &stem[stem.len() - 10..];
            let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") else { continue };

            if date < cutoff {
                if tokio::fs::remove_file(&path).await.is_ok() {
                    removed += 1;
                }
                continue;
            }

            if !is_gz && date < today {
                if let Err(err) = compress_log_file(&path).await {
                    tracing::warn!("로그 파일 압축 실패 ({}): {}", file_name, err);
                }
            }
        }
        Ok(removed)
    }

    // TODO: Add CSV export after report columns are confirmed.
}

/// `.log.gz`를 `.log`보다 먼저 확인해 `.log.gz` 파일도 `.log`로 오인하지 않도록 한다.
/// 반환값: (날짜 추출용 stem, 이미 압축된 파일인지)
fn strip_log_suffix(file_name: &str) -> Option<(&str, bool)> {
    if let Some(stem) = file_name.strip_suffix(".log.gz") {
        Some((stem, true))
    } else {
        file_name.strip_suffix(".log").map(|stem| (stem, false))
    }
}

/// `{name}.log` 파일을 gzip 압축해 `{name}.log.gz`로 만들고 원본을 삭제한다.
/// 디스크 I/O + CPU 압축이라 blocking 스레드에서 실행한다.
async fn compress_log_file(path: &Path) -> Result<()> {
    let mut gz_name = path.as_os_str().to_owned();
    gz_name.push(".gz");
    let gz_path = PathBuf::from(gz_name);
    let src = path.to_path_buf();

    tokio::task::spawn_blocking(move || -> Result<()> {
        let data = std::fs::read(&src).context("압축 대상 로그 읽기 실패")?;
        let file = std::fs::File::create(&gz_path).context("압축 파일 생성 실패")?;
        let mut encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        encoder.write_all(&data).context("gzip 쓰기 실패")?;
        encoder.finish().context("gzip 종료 실패")?;
        std::fs::remove_file(&src).context("원본 로그 삭제 실패")?;
        Ok(())
    })
    .await
    .context("로그 압축 작업 실행 실패")?
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

    /// cdn-*.log에 provider별 HTTP 호출 단계(인증 → purge 등)가 상태코드·소요시간과 함께
    /// 순서대로 기록되는지 검증 — audit.log를 따로 뒤지지 않아도 실패 원인을 이 파일에서 추적 가능해야 함
    #[tokio::test]
    async fn append_log_file_renders_cdn_request_steps() {
        let data_dir = std::env::temp_dir().join(format!(
            "nexuspurge-request-steps-test-{}",
            uuid::Uuid::new_v4()
        ));
        let service = OperationLogService::new(data_dir.clone());
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();

        let mut log = sample_log("purge-1", "2026-07-10T00:00:00Z");
        log.purge_results = vec![serde_json::json!({
            "provider": "kt",
            "status": "success",
            "startedAt": "2026-07-10T00:00:00Z",
            "finishedAt": "2026-07-10T00:00:01Z",
            "urls": ["assets/app.js"],
            "requestSteps": [
                {
                    "method": "POST(인증)",
                    "url": "https://api.ktcdn.co.kr/v3/auth/tokens",
                    "status": 200,
                    "statusText": "OK",
                    "elapsedMs": 372,
                    "summary": "(빈 응답)",
                },
                {
                    "method": "POST",
                    "url": "https://api.ktcdn.co.kr/v3/management/service/x/purge",
                    "status": 201,
                    "statusText": "Created",
                    "elapsedMs": 41,
                    "summary": "{\"transid\":123}",
                },
            ],
        })];
        service.save(log).await.unwrap();

        let logs_dir = service.log_files_dir();
        let cdn = std::fs::read_to_string(logs_dir.join(format!("cdn-{}.log", today))).unwrap();

        assert!(cdn.contains("steps:"));
        assert!(cdn.contains("1. POST(인증) https://api.ktcdn.co.kr/v3/auth/tokens → HTTP 200 OK (372ms)"));
        assert!(cdn.contains("2. POST https://api.ktcdn.co.kr/v3/management/service/x/purge → HTTP 201 Created (41ms) 응답: {\"transid\":123}"));

        let _ = std::fs::remove_dir_all(data_dir);
    }

    /// 대량 작업(파일 수 > 임계값)은 파일마다 한 줄씩 반복하지 않고 상태별 건수로 요약하며,
    /// 실패 건만 개별 나열해야 함 — 1000+ 파일 삭제 시 로그가 사실상 못 읽게 되는 문제 방지
    #[tokio::test]
    async fn append_log_file_summarizes_large_file_batches() {
        let data_dir = std::env::temp_dir().join(format!(
            "nexuspurge-large-batch-test-{}",
            uuid::Uuid::new_v4()
        ));
        let service = OperationLogService::new(data_dir.clone());
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();

        let mut log = sample_log("delete-1", "2026-07-10T00:00:00Z");
        log.operation = "delete".to_string();
        let mut files: Vec<serde_json::Value> = (0..40)
            .map(|i| {
                serde_json::json!({
                    "path": format!("contents/file-{:03}.txt", i),
                    "status": "success",
                    "startedAt": "2026-07-10T00:00:00Z",
                    "finishedAt": "2026-07-10T00:00:01Z",
                })
            })
            .collect();
        files.push(serde_json::json!({
            "path": "contents/file-broken.txt",
            "status": "error",
            "startedAt": "2026-07-10T00:00:00Z",
            "finishedAt": "2026-07-10T00:00:02Z",
            "error": "권한 없음",
        }));
        log.files = files;
        service.save(log).await.unwrap();

        let logs_dir = service.log_files_dir();
        let system = std::fs::read_to_string(logs_dir.join(format!("system-{}.log", today))).unwrap();

        // BTreeMap 키 정렬 순서(알파벳순)대로 출력됨: error < success
        assert!(system.contains("요약: error 1건, success 40건"));
        // 실패 건은 개별 라인으로 계속 노출
        assert!(system.contains("contents/file-broken.txt"));
        assert!(system.contains("error: 권한 없음"));
        // 성공 건은 개별 나열되지 않음 (요약에만 반영)
        assert!(!system.contains("file-000.txt"));

        let _ = std::fs::remove_dir_all(data_dir);
    }

    /// LOG_RETENTION_DAYS(30일)보다 오래된 system/transfer/cdn-*.log는 삭제되고,
    /// 오늘 날짜 파일과 audit-*.log(별도 관리 대상)는 그대로 남아있어야 함
    #[tokio::test]
    async fn cleanup_old_logs_removes_only_stale_typed_logs() {
        let data_dir = std::env::temp_dir().join(format!(
            "nexuspurge-cleanup-test-{}",
            uuid::Uuid::new_v4()
        ));
        let service = OperationLogService::new(data_dir.clone());
        let logs_dir = service.log_files_dir();
        std::fs::create_dir_all(&logs_dir).unwrap();

        let old_date = (chrono::Local::now().date_naive() - chrono::Duration::days(31))
            .format("%Y-%m-%d")
            .to_string();
        let today = chrono::Local::now().date_naive().format("%Y-%m-%d").to_string();

        let old_system = logs_dir.join(format!("system-{}.log", old_date));
        let old_transfer = logs_dir.join(format!("transfer-{}.log", old_date));
        let old_audit = logs_dir.join(format!("audit-{}.log", old_date));
        let today_cdn = logs_dir.join(format!("cdn-{}.log", today));
        for path in [&old_system, &old_transfer, &old_audit, &today_cdn] {
            std::fs::write(path, "log content").unwrap();
        }

        let removed = service.cleanup_old_logs().await.unwrap();

        assert_eq!(removed, 2, "system/transfer의 오래된 파일 2개만 삭제되어야 함");
        assert!(!old_system.exists());
        assert!(!old_transfer.exists());
        // audit-*.log는 tracing-appender의 max_log_files가 관리 — 여기서는 건드리지 않음
        assert!(old_audit.exists());
        // 오늘 날짜 파일은 아직 쓰기 중일 수 있으므로 압축도, 삭제도 하지 않음
        assert!(today_cdn.exists());

        let _ = std::fs::remove_dir_all(data_dir);
    }

    /// 오늘 날짜가 아닌 typed 로그는 `.log.gz`로 압축되어 원본이 사라지고,
    /// 압축 해제 시 원본 내용과 동일해야 함. 30일 초과분은 `.log.gz`도 삭제 대상.
    #[tokio::test]
    async fn cleanup_old_logs_compresses_non_today_logs_and_still_expires_gz() {
        let data_dir = std::env::temp_dir().join(format!(
            "nexuspurge-compress-test-{}",
            uuid::Uuid::new_v4()
        ));
        let service = OperationLogService::new(data_dir.clone());
        let logs_dir = service.log_files_dir();
        std::fs::create_dir_all(&logs_dir).unwrap();

        let yesterday = (chrono::Local::now().date_naive() - chrono::Duration::days(1))
            .format("%Y-%m-%d")
            .to_string();
        let yesterday_system = logs_dir.join(format!("system-{}.log", yesterday));
        let content = "어제 기록된 시스템 로그\n두 번째 줄";
        std::fs::write(&yesterday_system, content).unwrap();

        service.cleanup_old_logs().await.unwrap();

        assert!(!yesterday_system.exists(), "압축 후 원본 .log는 삭제되어야 함");
        let gz_path = logs_dir.join(format!("system-{}.log.gz", yesterday));
        assert!(gz_path.exists(), ".log.gz 파일이 생성되어야 함");

        use std::io::Read;
        let mut decoder = flate2::read::GzDecoder::new(std::fs::File::open(&gz_path).unwrap());
        let mut decompressed = String::new();
        decoder.read_to_string(&mut decompressed).unwrap();
        assert_eq!(decompressed, content, "압축 해제 결과가 원본과 같아야 함");

        // 이미 30일 지난 .log.gz 파일도 다음 정리에서 삭제되어야 함
        let expired_gz = logs_dir.join(format!(
            "cdn-{}.log.gz",
            (chrono::Local::now().date_naive() - chrono::Duration::days(31)).format("%Y-%m-%d")
        ));
        std::fs::write(&expired_gz, "old gz").unwrap();
        let removed = service.cleanup_old_logs().await.unwrap();
        assert_eq!(removed, 1);
        assert!(!expired_gz.exists());

        let _ = std::fs::remove_dir_all(data_dir);
    }
}
