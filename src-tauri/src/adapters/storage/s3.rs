use anyhow::{Context, Result};
use aws_config::{BehaviorVersion, Region};
use aws_credential_types::Credentials;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client as AwsS3Client;
use aws_smithy_runtime_api::client::result::SdkError;
use aws_smithy_types::error::metadata::ProvideErrorMetadata;
use reqwest::Client;
use std::path::Path;
use std::time::Duration;
use tokio::fs;
use tokio::io::AsyncReadExt;
use tokio::task::JoinSet;
use url::Url;

use crate::adapters::storage::base::{
    ListResult, ObjectMeta, Progress, RemoteFile, S3ObjectDetail, StorageAdapter, UploadResult,
};
use crate::commands::s3::FileItem;
use crate::utils::config::AwsCredentials;
use crate::utils::sigv4::Signer;

// ?�?�?� Constants ?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�

/// ???�기 ?�상?�면 멀?�파???�로?�로 ?�환
pub const MULTIPART_THRESHOLD: u64 = 10 * 1024 * 1024; // 10 MB
/// ?�트???�기 (S3 최소 5 MB, 마�?�??�트 ?�외)
pub const PART_SIZE: usize = 10 * 1024 * 1024; // 10 MB
/// ?�시 ?�트 ?�로????
const MAX_CONCURRENT_PARTS: usize = 4;

// ?�?�?� S3Adapter ?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�

#[derive(Clone)]
pub struct S3Adapter {
    client:   Client,
    sdk_client: AwsS3Client,
    endpoint: String, // "https://s3.{region}.amazonaws.com" or custom
    bucket:   String,
    region:   String,
    creds:    AwsCredentials,
}

impl S3Adapter {
    pub async fn new(
        region:   &str,
        bucket:   &str,
        creds:    &AwsCredentials,
        endpoint: Option<&str>,
    ) -> Result<Self> {
        let normalized_region = region.trim().to_owned();
        let normalized_bucket = bucket.trim().trim_matches('/').to_owned();
        let normalized_access_key = normalize_access_key_id(&creds.access_key_id);
        let normalized_secret_key = normalize_secret_access_key(&creds.secret_access_key);
        if normalized_access_key.is_empty() {
            return Err(anyhow::anyhow!("Access Key ID is required"));
        }
        if normalized_secret_key.is_empty() {
            return Err(anyhow::anyhow!("Secret Access Key is required"));
        }

        let client = Client::builder()
            .use_native_tls()
            .build()
            .context("HTTP ?�라?�언???�성 ?�패")?;

        let custom_endpoint = endpoint
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|s| s.trim_end_matches('/').to_owned());
        let ep = custom_endpoint
            .clone()
            .unwrap_or_else(|| format!("https://s3.{}.amazonaws.com", normalized_region));

        let sdk_config = aws_config::defaults(BehaviorVersion::latest())
            .region(Region::new(normalized_region.clone()))
            .credentials_provider(Credentials::new(
                normalized_access_key.clone(),
                normalized_secret_key.clone(),
                None,
                None,
                "nexuspurge-static-form-credentials",
            ))
            .load()
            .await;
        let mut s3_config_builder =
            aws_sdk_s3::config::Builder::from(&sdk_config).force_path_style(false);
        if let Some(endpoint_url) = custom_endpoint {
            s3_config_builder = s3_config_builder.endpoint_url(endpoint_url);
        }
        let sdk_client = AwsS3Client::from_conf(s3_config_builder.build());

        Ok(Self {
            client,
            sdk_client,
            endpoint: ep,
            bucket: normalized_bucket,
            region: normalized_region,
            creds: AwsCredentials {
                access_key_id: normalized_access_key,
                secret_access_key: normalized_secret_key,
            },
        })
    }

    fn signer_for(&self, service: &'static str) -> Signer<'_> {
        Signer {
            access_key_id:     &self.creds.access_key_id,
            secret_access_key: &self.creds.secret_access_key,
            region:            &self.region,
            service,
        }
    }

    fn sdk_failure<E, R>(
        &self,
        operation: &str,
        key: Option<&str>,
        err: &SdkError<E, R>,
    ) -> anyhow::Error
    where
        E: ProvideErrorMetadata,
    {
        let code = sdk_error_code(err);
        let user_message = if code == "SignatureDoesNotMatch" {
            "Secret Access Key 불일�??�는 ?�명 ?�성 ?�류?�니?? 같�? Access Key/Secret?�로 AWS CLI PutObject�?먼�? ?�인?�세??"
        } else {
            operation
        };
        tracing::error!(
            "S3 {} failed: access_key_id={}, region={}, bucket={}, key={}, api={}, aws_error_code={}, error={}",
            operation,
            mask_access_key_id(&self.creds.access_key_id),
            self.region,
            self.bucket,
            key.unwrap_or("-"),
            operation,
            code,
            err
        );
        anyhow::anyhow!(
            "S3 {} ?�패: {} (access_key_id={}, region={}, bucket={}, key={}, api={}, aws_error_code={})",
            operation,
            user_message,
            mask_access_key_id(&self.creds.access_key_id),
            self.region,
            self.bucket,
            key.unwrap_or("-"),
            operation,
            code
        )
    }

    // ?�?� Public Operations ?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�

    pub async fn verify_access(&self, base_prefix: &str) -> Result<Vec<String>> {
        self.test_connection(base_prefix).await
    }

    pub async fn test_connection(&self, base_prefix: &str) -> Result<Vec<String>> {
        let prefix = normalize_base_prefix(base_prefix);
        let test_key = if prefix.is_empty() {
            ".nexuspurge-connection-test.txt".to_owned()
        } else {
            format!("{}/.nexuspurge-connection-test.txt", prefix)
        };
        let mut warnings = Vec::new();

        self.test_sts_get_caller_identity().await?;

        if let Err(err) = self.test_head_bucket().await {
            let head_err = err.to_string();
            match self.test_get_bucket_location().await {
                Ok(()) => {}
                Err(location_err) => warnings.push(format!(
                    "HeadBucket/GetBucketLocation 경고: {} / {}",
                    head_err, location_err
                )),
            }
        }

        if let Err(err) = self.test_list_objects_v2(&prefix).await {
            let message = err.to_string();
            if message.contains("AccessDenied") || message.contains("HTTP 403") {
                warnings.push(format!("목록 조회 권한 ?�음: {}", message));
            } else {
                warnings.push(format!("ListObjectsV2 경고: {}", message));
            }
        }

        self.test_put_object(&test_key).await?;

        if let Err(err) = self.test_delete_object(&test_key).await {
            warnings.push(format!("DeleteObject 경고: {}", err));
        }

        Ok(warnings)
    }

    async fn test_sts_get_caller_identity(&self) -> Result<()> {
        let body = b"Action=GetCallerIdentity&Version=2011-06-15".to_vec();
        let url = Url::parse(&self.sts_url()).context("STS URL ?�성 ?�패")?;
        let headers = self.signer_for("sts").sign_headers(
            "POST",
            &url,
            &[("content-type", "application/x-www-form-urlencoded")],
            &body,
        );
        let mut req = self
            .client
            .post(url.as_str())
            .header("content-type", "application/x-www-form-urlencoded")
            .body(body);
        for (k, v) in &headers {
            req = req.header(k.as_str(), v.as_str());
        }
        let resp = req
            .send()
            .await
            .context("STS GetCallerIdentity ?�패: HTTP ?�청 ?�패")?;
        self.ensure_success("STS GetCallerIdentity", None, resp).await
    }

    async fn test_head_bucket(&self) -> Result<()> {
        self.sdk_client
            .head_bucket()
            .bucket(&self.bucket)
            .send()
            .await
            .map_err(|err| self.sdk_failure("HeadBucket", None, &err))?;
        return Ok(());
    }

    async fn test_get_bucket_location(&self) -> Result<()> {
        self.sdk_client
            .get_bucket_location()
            .bucket(&self.bucket)
            .send()
            .await
            .map_err(|err| self.sdk_failure("GetBucketLocation", None, &err))?;
        Ok(())
    }

    async fn test_list_objects_v2(&self, prefix: &str) -> Result<()> {
        self.sdk_client
            .list_objects_v2()
            .bucket(&self.bucket)
            .prefix(prefix)
            .max_keys(1)
            .send()
            .await
            .map_err(|err| self.sdk_failure("ListObjectsV2", Some(prefix), &err))?;
        return Ok(());
    }

    async fn test_put_object(&self, key: &str) -> Result<()> {
        self.sdk_client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(ByteStream::from_static(b"NexusPurge connection test\n"))
            .content_type("text/plain; charset=utf-8")
            .send()
            .await
            .map_err(|err| self.sdk_failure("PutObject", Some(key), &err))?;
        return Ok(());
    }

    async fn test_delete_object(&self, key: &str) -> Result<()> {
        self.sdk_client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|err| self.sdk_failure("DeleteObject", Some(key), &err))?;
        return Ok(());
    }

    async fn ensure_success(
        &self,
        operation: &str,
        key: Option<&str>,
        resp: reqwest::Response,
    ) -> Result<()> {
        let status = resp.status();
        if status.is_success() {
            return Ok(());
        }
        let body = resp.text().await.unwrap_or_default();
        let code = aws_error_code(&body).unwrap_or_else(|| format!("HTTP {}", status));
        Err(anyhow::anyhow!(
            "{} ?�패: {} (bucket={}, key={}, region={}){}",
            operation,
            code,
            self.bucket,
            key.unwrap_or("-"),
            self.region,
            if body.trim().is_empty() {
                String::new()
            } else {
                format!(": {}", compact_error_body(&body))
            }
        ))
    }

    fn sts_url(&self) -> String {
        if self.endpoint.contains("localhost") || self.endpoint.contains("127.0.0.1") {
            return self.endpoint.clone();
        }
        format!("https://sts.{}.amazonaws.com", self.region)
    }

    /// ?�일 ?�이지 목록 조회 (?��???
    async fn list_objects_page(
        &self,
        prefix: &str,
        continuation_token: Option<&str>,
    ) -> Result<ListResult> {
        let mut request = self
            .sdk_client
            .list_objects_v2()
            .bucket(&self.bucket)
            .prefix(prefix)
            .delimiter("/")
            .max_keys(1000);
        if let Some(token) = continuation_token {
            request = request.continuation_token(token);
        }
        let response = request
            .send()
            .await
            .map_err(|err| self.sdk_failure("ListObjectsV2", Some(prefix), &err))?;

        let mut files = Vec::new();
        for common_prefix in response.common_prefixes() {
            if let Some(prefix) = common_prefix.prefix() {
                let name = prefix
                    .trim_end_matches('/')
                    .rsplit('/')
                    .next()
                    .unwrap_or(prefix)
                    .to_owned();
                files.push(FileItem {
                    name,
                    path: prefix.to_owned(),
                    size: 0,
                    last_modified: String::new(),
                    is_directory: true,
                    etag: None,
                    content_type: None,
                });
            }
        }
        for object in response.contents() {
            if let Some(key) = object.key() {
                let name = key.rsplit('/').next().unwrap_or(key).to_owned();
                if !name.is_empty() {
                    files.push(FileItem {
                        name,
                        path: key.to_owned(),
                        size: object.size().unwrap_or(0).max(0) as u64,
                        last_modified: object
                            .last_modified()
                            .map(|value| value.to_string())
                            .unwrap_or_default(),
                        is_directory: false,
                        etag: object.e_tag().map(|value| value.trim_matches('"').to_owned()),
                        content_type: None,
                    });
                }
            }
        }

        return Ok(ListResult {
            files,
            next_token: response.next_continuation_token().map(ToOwned::to_owned),
            is_truncated: response.is_truncated().unwrap_or(false),
        });
    }

    /// C-3: ?�체 ?�이지�??�회??1000�?초과 ?�브?�트�?모두 반환
    pub async fn list_objects_all(&self, prefix: &str) -> Result<ListResult> {
        let mut files = Vec::new();
        let mut token: Option<String> = None;

        loop {
            let page = self.list_objects_page(prefix, token.as_deref()).await?;
            files.extend(page.files);
            if !page.is_truncated || page.next_token.is_none() {
                break;
            }
            token = page.next_token;
        }

        Ok(ListResult { files, next_token: None, is_truncated: false })
    }

    /// ?�브?�트 목록 (기존 FileItem ?�?? ?�위 ?�환?? ???��??�으�??�체 ?�이지 조회
    pub async fn list_objects_raw(&self, prefix: &str) -> Result<ListResult> {
        self.list_objects_all(prefix).await
    }

    /// ETag만 반환 (sync 플랜 비교용)
    #[allow(dead_code)]
    pub async fn head_object_etag(&self, key: &str) -> Result<Option<String>> {
        Ok(self.head_object_meta(key).await?.and_then(|meta| meta.etag))
    }

    pub async fn head_object_meta(&self, key: &str) -> Result<Option<ObjectMeta>> {
        match self
            .sdk_client
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
        {
            Ok(resp) => Ok(Some(ObjectMeta {
                key: key.to_owned(),
                size: resp.content_length().unwrap_or(0).max(0) as u64,
                etag: resp.e_tag().map(|value| value.trim_matches('"').to_owned()),
                last_modified: resp
                    .last_modified()
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                content_type: resp.content_type().map(|value| value.to_owned()),
            })),
            Err(err) => {
                if err
                    .as_service_error()
                    .map(|e| e.is_not_found())
                    .unwrap_or(false)
                {
                    return Ok(None);
                }
                Err(self.sdk_failure("HeadObject", Some(key), &err))
            }
        }
    }

    /// 속성(우클릭) 다이얼로그의 "S3 상세 헤더" 표시용 — HeadObject 응답의 전 필드를 그대로 반환.
    /// 크롬 개발자모드 Network 탭에서 보는 응답 헤더 수준의 상세 정보를 제공한다.
    pub async fn head_object_full(&self, key: &str) -> Result<Option<S3ObjectDetail>> {
        match self
            .sdk_client
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
        {
            Ok(resp) => Ok(Some(S3ObjectDetail {
                key: key.to_owned(),
                etag: resp.e_tag().map(|v| v.trim_matches('"').to_owned()),
                content_length: resp.content_length(),
                content_type: resp.content_type().map(ToOwned::to_owned),
                content_encoding: resp.content_encoding().map(ToOwned::to_owned),
                content_disposition: resp.content_disposition().map(ToOwned::to_owned),
                content_language: resp.content_language().map(ToOwned::to_owned),
                cache_control: resp.cache_control().map(ToOwned::to_owned),
                last_modified: resp.last_modified().map(|v| v.to_string()),
                storage_class: resp.storage_class().map(|v| v.as_str().to_owned()),
                server_side_encryption: resp.server_side_encryption().map(|v| v.as_str().to_owned()),
                sse_kms_key_id: resp.ssekms_key_id().map(ToOwned::to_owned),
                version_id: resp.version_id().map(ToOwned::to_owned),
                replication_status: resp.replication_status().map(|v| v.as_str().to_owned()),
                accept_ranges: resp.accept_ranges().map(ToOwned::to_owned),
                checksum_crc32: resp.checksum_crc32().map(ToOwned::to_owned),
                checksum_sha256: resp.checksum_sha256().map(ToOwned::to_owned),
                metadata: resp.metadata().cloned().unwrap_or_default(),
            })),
            Err(err) => {
                if err
                    .as_service_error()
                    .map(|e| e.is_not_found())
                    .unwrap_or(false)
                {
                    return Ok(None);
                }
                Err(self.sdk_failure("HeadObject", Some(key), &err))
            }
        }
    }

    /// List every object key under the prefix (no delimiter) — used for folder deletion/download.
    pub async fn list_keys_recursive(&self, prefix: &str) -> Result<Vec<String>> {
        let mut keys = Vec::new();
        let mut token: Option<String> = None;
        loop {
            let mut request = self
                .sdk_client
                .list_objects_v2()
                .bucket(&self.bucket)
                .prefix(prefix)
                .max_keys(1000);
            if let Some(t) = &token {
                request = request.continuation_token(t);
            }
            let response = request
                .send()
                .await
                .map_err(|err| self.sdk_failure("ListObjectsV2", Some(prefix), &err))?;
            for object in response.contents() {
                if let Some(key) = object.key() {
                    keys.push(key.to_owned());
                }
            }
            token = response.next_continuation_token().map(ToOwned::to_owned);
            if !response.is_truncated().unwrap_or(false) || token.is_none() {
                break;
            }
        }
        Ok(keys)
    }

    pub async fn delete_objects(&self, keys: &[String]) -> Result<Vec<String>> {
        if keys.is_empty() {
            return Ok(vec![]);
        }
        // A key ending with "/" is a folder: expand to every object under the prefix,
        // including the "dir/" placeholder itself when it exists as a real object.
        let mut expanded: Vec<String> = Vec::with_capacity(keys.len());
        let mut seen = std::collections::HashSet::new();
        for key in keys {
            if key.ends_with('/') {
                for child in self.list_keys_recursive(key).await? {
                    if seen.insert(child.clone()) {
                        expanded.push(child);
                    }
                }
                if seen.insert(key.clone()) {
                    expanded.push(key.clone());
                }
            } else if seen.insert(key.clone()) {
                expanded.push(key.clone());
            }
        }

        let mut deleted = Vec::with_capacity(expanded.len());
        for key in &expanded {
            self.sdk_client
                .delete_object()
                .bucket(&self.bucket)
                .key(key)
                .send()
                .await
                .map_err(|err| self.sdk_failure("DeleteObject", Some(key), &err))?;
            deleted.push(key.clone());
        }
        return Ok(deleted);
    }

    pub async fn put_object(&self, key: &str, data: Vec<u8>, content_type: &str) -> Result<()> {
        self.sdk_client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(ByteStream::from(data.clone()))
            .content_type(content_type)
            .send()
            .await
            .map_err(|err| self.sdk_failure("PutObject", Some(key), &err))?;
        return Ok(());
    }

    /// ?�일 ?�로?? 10 MB ?�상?� ?�동?�로 멀?�파???�로??
    /// on_progress(transferred, total) 콜백?�로 진행�??�달
    pub async fn upload_with_progress(
        &self,
        local_path: &str,
        remote_key: &str,
        on_progress: impl Fn(u64, u64) -> bool,
    ) -> Result<UploadResult> {
        self.upload_with_options(local_path, remote_key, None, None, on_progress)
            .await
    }

    pub async fn upload_with_options(
        &self,
        local_path: &str,
        remote_key: &str,
        content_type_override: Option<&str>,
        cache_control: Option<&str>,
        on_progress: impl Fn(u64, u64) -> bool,
    ) -> Result<UploadResult> {
        let metadata = fs::metadata(local_path)
            .await
            .context("?�일 메�??�이???�기 ?�패")?;
        let total = metadata.len();
        let content_type = content_type_override
            .filter(|value| !value.trim().is_empty())
            .map(|value| value.trim().to_owned())
            .unwrap_or_else(|| {
                mime_guess::from_path(local_path)
                    .first_or_octet_stream()
                    .to_string()
            });
        let cache_control = cache_control.filter(|value| !value.trim().is_empty());

        if total >= MULTIPART_THRESHOLD {
            self.upload_multipart(
                local_path,
                remote_key,
                &content_type,
                cache_control,
                total,
                on_progress,
            )
                .await
        } else {
            self.upload_single(
                local_path,
                remote_key,
                &content_type,
                cache_control,
                total,
                on_progress,
            )
                .await
        }
    }

    /// ?�트리밍 ?�운로드
    pub async fn download_with_progress(
        &self,
        remote_key: &str,
        local_path: &str,
        on_progress: impl Fn(u64, u64),
    ) -> Result<()> {
        self.download_with_cancel(remote_key, local_path, || false, on_progress)
            .await
    }

    pub async fn download_with_cancel(
        &self,
        remote_key: &str,
        local_path: &str,
        is_cancelled: impl Fn() -> bool,
        on_progress: impl Fn(u64, u64),
    ) -> Result<()> {
        use tokio::io::AsyncWriteExt;

        let resp = self
            .sdk_client
            .get_object()
            .bucket(&self.bucket)
            .key(remote_key)
            .send()
            .await
            .map_err(|err| self.sdk_failure("GetObject", Some(remote_key), &err))?;

        let total = resp.content_length().unwrap_or(0).max(0) as u64;

        if let Some(parent) = Path::new(local_path).parent() {
            fs::create_dir_all(parent)
                .await
                .context("디렉터리 생성 실패")?;
        }

        let mut file = fs::File::create(local_path)
            .await
            .context("파일 생성 실패")?;
        let mut body = resp.body;
        let mut received: u64 = 0;

        while let Some(chunk) = body
            .try_next()
            .await
            .context("다운로드 스트림 오류")?
        {
            if is_cancelled() {
                return Err(anyhow::anyhow!("Operation cancelled"));
            }
            file.write_all(&chunk).await.context("파일 쓰기 실패")?;
            received += chunk.len() as u64;
            on_progress(received, total);
        }

        file.flush().await.context("파일 flush 실패")?;
        Ok(())
    }

    /// S3 오브젝트 이름 변경 (CopyObject 후 DeleteObject)
    pub async fn rename_object(&self, src_key: &str, dst_key: &str) -> Result<()> {
        let copy_source = format!("{}/{}", self.bucket, encode_key(src_key));
        self.sdk_client
            .copy_object()
            .bucket(&self.bucket)
            .copy_source(copy_source)
            .key(dst_key)
            .send()
            .await
            .map_err(|err| self.sdk_failure("CopyObject", Some(src_key), &err))?;

        self.delete_objects(&[src_key.to_owned()])
            .await
            .context("원본 오브젝트 삭제 실패")?;
        Ok(())
    }

    pub async fn presign_get(&self, key: &str, expires_in_seconds: u64) -> Result<String> {
        let config = aws_sdk_s3::presigning::PresigningConfig::expires_in(
            std::time::Duration::from_secs(expires_in_seconds),
        )
        .context("Presign 만료 시간 설정 실패")?;

        let presigned = self
            .sdk_client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .presigned(config)
            .await
            .map_err(|err| self.sdk_failure("GetObject(Presign)", Some(key), &err))?;

        Ok(presigned.uri().to_string())
    }

    // ?�?� Multipart Upload Internals ?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�

    async fn upload_single(
        &self,
        local_path: &str,
        remote_key: &str,
        content_type: &str,
        cache_control: Option<&str>,
        total: u64,
        on_progress: impl Fn(u64, u64) -> bool,
    ) -> Result<UploadResult> {
        if !on_progress(0, total) {
            return Err(anyhow::anyhow!("Operation cancelled"));
        }

        let data = fs::read(local_path).await.context("?�일 ?�기 ?�패")?;
        if !on_progress(0, total) {
            return Err(anyhow::anyhow!("Operation cancelled"));
        }

                let mut request = self
            .sdk_client
            .put_object()
            .bucket(&self.bucket)
            .key(remote_key)
            .body(ByteStream::from(data))
            .content_type(content_type);
        if let Some(value) = cache_control {
            request = request.cache_control(value);
        }
        let response = request
            .send()
            .await
            .map_err(|err| self.sdk_failure("PutObject", Some(remote_key), &err))?;
        let etag = response.e_tag().map(|value| value.trim_matches('"').to_owned());

        if !on_progress(total, total) {
            return Err(anyhow::anyhow!("Operation cancelled"));
        }
        Ok(UploadResult {
            key: remote_key.to_owned(),
            etag,
            size: total,
            is_multipart: false,
        })
    }

    /// ?�라?�딩 ?�도??방식: 최�? 4�??�트�??�시???�로??
    /// 최�? 메모�??�용??= MAX_CONCURRENT_PARTS × PART_SIZE = 40 MB
    async fn upload_multipart(
        &self,
        local_path: &str,
        remote_key: &str,
        content_type: &str,
        cache_control: Option<&str>,
        total: u64,
        on_progress: impl Fn(u64, u64) -> bool,
    ) -> Result<UploadResult> {
        if !on_progress(0, total) {
            return Err(anyhow::anyhow!("Operation cancelled"));
        }

        let upload_id = self
            .initiate_multipart_upload(remote_key, content_type, cache_control)
            .await?;

        let mut file = fs::File::open(local_path)
            .await
            .context("?�일 ?�기 ?�패")?;

        let mut part_num: u32 = 1;
        let mut all_etags: Vec<(u32, String)> = Vec::new();
        let mut transferred: u64 = 0;

        loop {
            if !on_progress(transferred, total) {
                let _ = self.abort_multipart_upload(remote_key, &upload_id).await;
                return Err(anyhow::anyhow!("Operation cancelled"));
            }
            // ?�트 배치 ?�기 (최�? MAX_CONCURRENT_PARTS �?
            let mut batch: Vec<(u32, Vec<u8>)> = Vec::new();
            while batch.len() < MAX_CONCURRENT_PARTS {
                let mut chunk = vec![0u8; PART_SIZE];
                let mut filled = 0;

                // 부�??�기 처리: PART_SIZE ?�는 EOF 까�? 채�?
                while filled < PART_SIZE {
                    let n = file
                        .read(&mut chunk[filled..])
                        .await
                        .context("?�일 ?�기 ?�패")?;
                    if n == 0 {
                        break; // EOF
                    }
                    filled += n;
                }

                if filled == 0 {
                    break; // 배치 ??EOF
                }
                chunk.truncate(filled);
                batch.push((part_num, chunk));
                part_num += 1;
            }

            if batch.is_empty() {
                break; // ?�일 ??
            }

            // 배치 병렬 ?�로??
            let mut tasks: JoinSet<Result<(u32, String, u64)>> = JoinSet::new();
            for (num, data) in batch {
                let adapter = self.clone();
                let key = remote_key.to_owned();
                let uid = upload_id.clone();
                let size = data.len() as u64;

                tasks.spawn(async move {
                    let etag = adapter.upload_part_retry(&key, &uid, num, data).await?;
                    Ok((num, etag, size))
                });
            }

            while let Some(result) = tasks.join_next().await {
                match result {
                    Ok(Ok((num, etag, bytes))) => {
                        transferred += bytes;
                        if !on_progress(transferred, total) {
                            let _ = self.abort_multipart_upload(remote_key, &upload_id).await;
                            return Err(anyhow::anyhow!("Operation cancelled"));
                        }
                        all_etags.push((num, etag));
                    }
                    Ok(Err(e)) => {
                        let _ = self.abort_multipart_upload(remote_key, &upload_id).await;
                        return Err(e.context("?�트 ?�로???�패"));
                    }
                    Err(join_err) => {
                        let _ = self.abort_multipart_upload(remote_key, &upload_id).await;
                        return Err(anyhow::anyhow!("?�트 ?�로???�스???�닉: {}", join_err));
                    }
                }
            }
        }

        // ?�트 번호 ?�으�??�렬 ???�료
        all_etags.sort_by_key(|(n, _)| *n);
        let final_etag = match self
            .complete_multipart_upload(remote_key, &upload_id, &all_etags)
            .await
        {
            Ok(etag) => etag,
            Err(e) => {
                let _ = self.abort_multipart_upload(remote_key, &upload_id).await;
                return Err(e);
            }
        };

        Ok(UploadResult {
            key: remote_key.to_owned(),
            etag: Some(final_etag),
            size: total,
            is_multipart: true,
        })
    }

    async fn initiate_multipart_upload(
        &self,
        key: &str,
        content_type: &str,
        cache_control: Option<&str>,
    ) -> Result<String> {
        let mut request = self
            .sdk_client
            .create_multipart_upload()
            .bucket(&self.bucket)
            .key(key)
            .content_type(content_type);
        if let Some(value) = cache_control {
            request = request.cache_control(value);
        }
        let response = request
            .send()
            .await
            .map_err(|err| self.sdk_failure("CreateMultipartUpload", Some(key), &err))?;
        response
            .upload_id()
            .map(ToOwned::to_owned)
            .context("UploadId 없음")
    }

    async fn upload_part(
        &self,
        key: &str,
        upload_id: &str,
        part_number: u32,
        data: Vec<u8>,
    ) -> Result<String> {
        let response = self
            .sdk_client
            .upload_part()
            .bucket(&self.bucket)
            .key(key)
            .upload_id(upload_id)
            .part_number(part_number as i32)
            .body(ByteStream::from(data))
            .send()
            .await
            .map_err(|err| self.sdk_failure("UploadPart", Some(key), &err))?;
        response
            .e_tag()
            .map(|value| value.trim_matches('"').to_owned())
            .context("UploadPart ETag 없음")
    }

    async fn upload_part_retry(
        &self,
        key: &str,
        upload_id: &str,
        part_number: u32,
        data: Vec<u8>,
    ) -> Result<String> {
        let mut last_err = None;
        for attempt in 0..3 {
            match self
                .upload_part(key, upload_id, part_number, data.clone())
                .await
            {
                Ok(etag) => return Ok(etag),
                Err(err) if attempt < 2 => {
                    last_err = Some(err);
                    tokio::time::sleep(retry_delay(attempt)).await;
                }
                Err(err) => return Err(err),
            }
        }
        Err(last_err.expect("retry error"))
    }

    async fn complete_multipart_upload(
        &self,
        key: &str,
        upload_id: &str,
        parts: &[(u32, String)],
    ) -> Result<String> {
        let completed_parts: Vec<aws_sdk_s3::types::CompletedPart> = parts
            .iter()
            .map(|(n, etag)| {
                aws_sdk_s3::types::CompletedPart::builder()
                    .part_number(*n as i32)
                    .e_tag(etag)
                    .build()
            })
            .collect();
        let multipart = aws_sdk_s3::types::CompletedMultipartUpload::builder()
            .set_parts(Some(completed_parts))
            .build();

        let response = self
            .sdk_client
            .complete_multipart_upload()
            .bucket(&self.bucket)
            .key(key)
            .upload_id(upload_id)
            .multipart_upload(multipart)
            .send()
            .await
            .map_err(|err| self.sdk_failure("CompleteMultipartUpload", Some(key), &err))?;

        response
            .e_tag()
            .map(|value| value.trim_matches('"').to_owned())
            .context("CompleteMultipartUpload ETag 없음")
    }

    async fn abort_multipart_upload(&self, key: &str, upload_id: &str) -> Result<()> {
        self.sdk_client
            .abort_multipart_upload()
            .bucket(&self.bucket)
            .key(key)
            .upload_id(upload_id)
            .send()
            .await
            .map_err(|err| self.sdk_failure("AbortMultipartUpload", Some(key), &err))?;
        Ok(())
    }
}

// ?�?�?� StorageAdapter Trait Impl ?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�

impl StorageAdapter for S3Adapter {
    async fn list_objects(&self, prefix: &str) -> Result<Vec<RemoteFile>> {
        let result = self.list_objects_raw(prefix).await?;
        Ok(result
            .files
            .into_iter()
            .map(|f| RemoteFile {
                key:           f.path,
                size:          f.size,
                etag:          f.etag,
                last_modified: f.last_modified,
                content_type:  f.content_type,
            })
            .collect())
    }

    async fn upload_file(
        &self,
        local: &Path,
        key: &str,
        tx: tokio::sync::mpsc::UnboundedSender<Progress>,
    ) -> Result<UploadResult> {
        let local_str = local.to_str().context("?�효?��? ?��? ?�일 경로")?;
        // tx.send() ???�바?�드 채널�??��? 블로?�되지 ?�음
        self.upload_with_progress(local_str, key, move |transferred, total| {
            let _ = tx.send(Progress { transferred, total });
            true
        })
        .await
    }

    async fn download_file(
        &self,
        key: &str,
        local: &Path,
        tx: tokio::sync::mpsc::UnboundedSender<Progress>,
    ) -> Result<()> {
        let local_str = local.to_str().context("?�효?��? ?��? ?�일 경로")?;
        self.download_with_progress(key, local_str, move |transferred, total| {
            let _ = tx.send(Progress { transferred, total });
        })
        .await
    }

    async fn delete_object(&self, key: &str) -> Result<()> {
        self.delete_objects(&[key.to_owned()]).await?;
        Ok(())
    }

    async fn rename_object(&self, old_key: &str, new_key: &str) -> Result<()> {
        self.rename_object(old_key, new_key).await
    }

    async fn head_object(&self, key: &str) -> Result<ObjectMeta> {
        self.head_object_meta(key)
            .await?
            .ok_or_else(|| anyhow::anyhow!("오브젝트 없음: {}", key))
    }
}

// ?�?�?� XML Parsing ?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�

#[allow(dead_code)]
fn parse_list_response(xml: &str) -> Result<ListResult> {
    let mut files: Vec<FileItem> = vec![];

    // ?�더 (CommonPrefixes)
    let mut search = xml;
    while let Some(start) = search.find("<CommonPrefixes>") {
        let rest = &search[start + "<CommonPrefixes>".len()..];
        if let Some(prefix) = xml_extract(rest, "Prefix") {
            let name = prefix
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or(&prefix)
                .to_owned();
            files.push(FileItem {
                name,
                path: prefix,
                size: 0,
                last_modified: String::new(),
                is_directory: true,
                etag: None,
                content_type: None,
            });
        }
        if let Some(end) = rest.find("</CommonPrefixes>") {
            search = &rest[end + "</CommonPrefixes>".len()..];
        } else {
            break;
        }
    }

    // ?�일 (Contents)
    let mut search = xml;
    while let Some(start) = search.find("<Contents>") {
        let rest = &search[start + "<Contents>".len()..];
        let end = rest.find("</Contents>").unwrap_or(rest.len());
        let block = &rest[..end];

        if let Some(key) = xml_extract(block, "Key") {
            let name = key.rsplit('/').next().unwrap_or(&key).to_owned();
            if !name.is_empty() {
                files.push(FileItem {
                    name,
                    path: key,
                    size: xml_extract(block, "Size")
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0),
                    last_modified: xml_extract(block, "LastModified").unwrap_or_default(),
                    is_directory: false,
                    etag: xml_extract(block, "ETag").map(|e| e.trim_matches('"').to_owned()),
                    content_type: None,
                });
            }
        }

        search = &rest[end..];
    }

    let is_truncated = xml.contains("<IsTruncated>true</IsTruncated>");
    let next_token = xml_extract(xml, "NextContinuationToken");

    Ok(ListResult { files, next_token, is_truncated })
}

fn xml_extract(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(xml_unescape(&xml[start..end]))
}

#[allow(dead_code)]
fn xml_tag_values(xml: &str, tag: &str) -> Vec<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let mut values = Vec::new();
    let mut search = xml;

    while let Some(start) = search.find(&open) {
        let rest = &search[start + open.len()..];
        if let Some(end) = rest.find(&close) {
            values.push(xml_unescape(&rest[..end]));
            search = &rest[end + close.len()..];
        } else {
            break;
        }
    }

    values
}

/// H-5: XML entity ?�코????&amp; &lt; &gt; &quot; &apos;
fn xml_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
     .replace("&lt;", "<")
     .replace("&gt;", ">")
     .replace("&quot;", "\"")
     .replace("&apos;", "'")
}

#[allow(dead_code)]
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn encode_key(key: &str) -> String {
    key.split('/')
        .map(|seg| {
            percent_encoding::utf8_percent_encode(seg, percent_encoding::NON_ALPHANUMERIC)
                .to_string()
                .replace("%7E", "~")
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn normalize_base_prefix(prefix: &str) -> String {
    prefix.trim().trim_matches('/').to_owned()
}

fn normalize_access_key_id(value: &str) -> String {
    value.trim().to_owned()
}

fn normalize_secret_access_key(value: &str) -> String {
    value.trim().to_owned()
}

fn mask_access_key_id(value: &str) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() <= 8 {
        return "*".repeat(chars.len().max(1));
    }
    let first = chars.iter().take(4).collect::<String>();
    let last = chars
        .iter()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{}****{}", first, last)
}

fn sdk_error_code<E, R>(err: &SdkError<E, R>) -> String
where
    E: ProvideErrorMetadata,
{
    err.as_service_error()
        .and_then(|service_error| service_error.code())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| "Unknown".to_owned())
}

fn aws_error_code(body: &str) -> Option<String> {
    xml_extract(body, "Code").or_else(|| {
        let trimmed = body.trim();
        if trimmed.is_empty() {
            None
        } else if trimmed.len() <= 80 && !trimmed.contains('<') {
            Some(trimmed.to_owned())
        } else {
            None
        }
    })
}

fn compact_error_body(body: &str) -> String {
    body.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn retry_delay(attempt: usize) -> Duration {
    Duration::from_millis(250 * 2_u64.pow(attempt as u32))
}

#[allow(dead_code)]
fn base64_md5(data: &[u8]) -> String {
    let hash = md5::compute(data);
    base64_encode(hash.as_ref())
}

#[allow(dead_code)]
fn base64_encode(input: &[u8]) -> String {
    const CHARS: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i + 2 < input.len() {
        let b = ((input[i] as u32) << 16)
            | ((input[i + 1] as u32) << 8)
            | (input[i + 2] as u32);
        out.push(CHARS[(b >> 18) as usize] as char);
        out.push(CHARS[((b >> 12) & 0x3f) as usize] as char);
        out.push(CHARS[((b >> 6) & 0x3f) as usize] as char);
        out.push(CHARS[(b & 0x3f) as usize] as char);
        i += 3;
    }
    match input.len() - i {
        1 => {
            let b = (input[i] as u32) << 16;
            out.push(CHARS[(b >> 18) as usize] as char);
            out.push(CHARS[((b >> 12) & 0x3f) as usize] as char);
            out.push_str("==");
        }
        2 => {
            let b = ((input[i] as u32) << 16) | ((input[i + 1] as u32) << 8);
            out.push(CHARS[(b >> 18) as usize] as char);
            out.push(CHARS[((b >> 12) & 0x3f) as usize] as char);
            out.push(CHARS[((b >> 6) & 0x3f) as usize] as char);
            out.push('=');
        }
        _ => {}
    }
    out
}
