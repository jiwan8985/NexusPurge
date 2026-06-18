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
    ListResult, ObjectMeta, Progress, RemoteFile, StorageAdapter, UploadResult,
};
use crate::commands::s3::FileItem;
use crate::utils::config::AwsCredentials;
use crate::utils::retry::is_retryable_status;
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

    fn signer(&self) -> Signer<'_> {
        self.signer_for("s3")
    }

    fn signer_for(&self, service: &'static str) -> Signer<'_> {
        Signer {
            access_key_id:     &self.creds.access_key_id,
            secret_access_key: &self.creds.secret_access_key,
            region:            &self.region,
            service,
        }
    }

    fn bucket_url(&self) -> String {
        format!("{}/{}", self.endpoint, self.bucket)
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

    // ?�?� Signed HTTP Helpers ?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�?�

    async fn signed_get(&self, url: &Url) -> Result<reqwest::Response> {
        let headers = self.signer().sign_headers("GET", url, &[], b"");
        let mut last_err = None;
        for attempt in 0..3 {
            let mut req = self.client.get(url.as_str());
            for (k, v) in &headers {
                req = req.header(k.as_str(), v.as_str());
            }
            match req.send().await {
                Ok(resp) if should_retry_status(resp.status()) && attempt < 2 => {
                    tokio::time::sleep(retry_delay(attempt)).await;
                }
                Ok(resp) => return Ok(resp),
                Err(err) if attempt < 2 => {
                    last_err = Some(err);
                    tokio::time::sleep(retry_delay(attempt)).await;
                }
                Err(err) => return Err(err).context("HTTP GET ?�패"),
            }
        }
        Err(last_err.expect("retry error")).context("HTTP GET ?�패")
    }

    async fn signed_head(&self, url: &Url) -> Result<reqwest::Response> {
        let headers = self.signer().sign_headers("HEAD", url, &[], b"");
        let mut last_err = None;
        for attempt in 0..3 {
            let mut req = self.client.head(url.as_str());
            for (k, v) in &headers {
                req = req.header(k.as_str(), v.as_str());
            }
            match req.send().await {
                Ok(resp) if should_retry_status(resp.status()) && attempt < 2 => {
                    tokio::time::sleep(retry_delay(attempt)).await;
                }
                Ok(resp) => return Ok(resp),
                Err(err) if attempt < 2 => {
                    last_err = Some(err);
                    tokio::time::sleep(retry_delay(attempt)).await;
                }
                Err(err) => return Err(err).context("HTTP HEAD ?�패"),
            }
        }
        Err(last_err.expect("retry error")).context("HTTP HEAD ?�패")
    }

    #[allow(dead_code)]
    async fn signed_put(
        &self,
        url: &Url,
        body: Vec<u8>,
        content_type: &str,
        cache_control: Option<&str>,
    ) -> Result<reqwest::Response> {
        let mut sign_headers = vec![("content-type", content_type)];
        if let Some(value) = cache_control {
            sign_headers.push(("cache-control", value));
        }
        let headers = self.signer().sign_headers("PUT", url, &sign_headers, &body);
        let mut last_err = None;
        for attempt in 0..3 {
            let mut req = self
                .client
                .put(url.as_str())
                .header("content-type", content_type)
                .body(body.clone());
            if let Some(value) = cache_control {
                req = req.header("cache-control", value);
            }
            for (k, v) in &headers {
                req = req.header(k.as_str(), v.as_str());
            }
            match req.send().await {
                Ok(resp) if should_retry_status(resp.status()) && attempt < 2 => {
                    tokio::time::sleep(retry_delay(attempt)).await;
                }
                Ok(resp) => return Ok(resp),
                Err(err) if attempt < 2 => {
                    last_err = Some(err);
                    tokio::time::sleep(retry_delay(attempt)).await;
                }
                Err(err) => return Err(err).context("HTTP PUT ?�패"),
            }
        }
        Err(last_err.expect("retry error")).context("HTTP PUT ?�패")
    }

    async fn signed_post(
        &self,
        url: &Url,
        body: Vec<u8>,
        content_type: &str,
        cache_control: Option<&str>,
    ) -> Result<reqwest::Response> {
        let mut sign_headers = vec![("content-type", content_type)];
        if let Some(value) = cache_control {
            sign_headers.push(("cache-control", value));
        }
        let headers = self.signer().sign_headers("POST", url, &sign_headers, &body);
        let mut last_err = None;
        for attempt in 0..3 {
            let mut req = self
                .client
                .post(url.as_str())
                .header("content-type", content_type)
                .body(body.clone());
            if let Some(value) = cache_control {
                req = req.header("cache-control", value);
            }
            for (k, v) in &headers {
                req = req.header(k.as_str(), v.as_str());
            }
            match req.send().await {
                Ok(resp) if should_retry_status(resp.status()) && attempt < 2 => {
                    tokio::time::sleep(retry_delay(attempt)).await;
                }
                Ok(resp) => return Ok(resp),
                Err(err) if attempt < 2 => {
                    last_err = Some(err);
                    tokio::time::sleep(retry_delay(attempt)).await;
                }
                Err(err) => return Err(err).context("HTTP POST ?�패"),
            }
        }
        Err(last_err.expect("retry error")).context("HTTP POST ?�패")
    }

    async fn signed_delete(&self, url: &Url) -> Result<reqwest::Response> {
        let headers = self.signer().sign_headers("DELETE", url, &[], b"");
        let mut last_err = None;
        for attempt in 0..3 {
            let mut req = self.client.delete(url.as_str());
            for (k, v) in &headers {
                req = req.header(k.as_str(), v.as_str());
            }
            match req.send().await {
                Ok(resp) if should_retry_status(resp.status()) && attempt < 2 => {
                    tokio::time::sleep(retry_delay(attempt)).await;
                }
                Ok(resp) => return Ok(resp),
                Err(err) if attempt < 2 => {
                    last_err = Some(err);
                    tokio::time::sleep(retry_delay(attempt)).await;
                }
                Err(err) => return Err(err).context("HTTP DELETE ?�패"),
            }
        }
        Err(last_err.expect("retry error")).context("HTTP DELETE ?�패")
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
        let url = Url::parse(&format!("{}/?location", self.bucket_url()))
            .context("GetBucketLocation URL ?�성 ?�패")?;
        let resp = self.signed_get(&url).await.context(format!(
            "S3 GetBucketLocation ?�패: bucket={}, region={}",
            self.bucket, self.region
        ))?;
        self.ensure_success("S3 GetBucketLocation", None, resp).await
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

    #[allow(dead_code)]
    async fn verify_list_access(&self) -> Result<()> {
        let url =
            Url::parse(&format!("{}/?list-type=2&max-keys=1", self.bucket_url()))
                .context("URL ?�싱 ?�패")?;
        let resp = self.signed_get(&url).await?;
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("버킷 ?�근 ?�패: HTTP {}", resp.status()));
        }
        Ok(())
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

    /// ETag�?반환 (sync ?�랜 비교??
    #[allow(dead_code)]
    pub async fn head_object_etag(&self, key: &str) -> Result<Option<String>> {
        let url = Url::parse(&format!("{}/{}", self.bucket_url(), encode_key(key)))
            .context("URL ?�싱 ?�패")?;
        let resp = self.signed_head(&url).await?;

        if resp.status().as_u16() == 404 {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("HeadObject ?�패: HTTP {}", resp.status()));
        }

        let etag = resp
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(|e| e.trim_matches('"').to_owned());

        Ok(etag)
    }

    pub async fn head_object_meta(&self, key: &str) -> Result<Option<ObjectMeta>> {
        let url = Url::parse(&format!("{}/{}", self.bucket_url(), encode_key(key)))
            .context("URL ?�싱 ?�패")?;
        let resp = self.signed_head(&url).await?;

        if resp.status().as_u16() == 404 {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("HeadObject ?�패: HTTP {}", resp.status()));
        }

        let headers = resp.headers();
        let etag = headers
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(|e| e.trim_matches('"').to_owned());
        let size = headers
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let last_modified = headers
            .get("last-modified")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();
        let content_type = headers
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_owned());

        Ok(Some(ObjectMeta {
            key: key.to_owned(),
            size,
            etag,
            last_modified,
            content_type,
        }))
    }

    pub async fn delete_objects(&self, keys: &[String]) -> Result<Vec<String>> {
        if keys.is_empty() {
            return Ok(vec![]);
        }
        let mut deleted = Vec::with_capacity(keys.len());
        for key in keys {
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
        use futures::StreamExt;
        use tokio::io::AsyncWriteExt;

        let url = Url::parse(&format!("{}/{}", self.bucket_url(), encode_key(remote_key)))
            .context("URL ?�싱 ?�패")?;
        let resp = self.signed_get(&url).await?;

        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("GetObject ?�패: HTTP {}", resp.status()));
        }

        let total = resp
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
            .unwrap_or(0u64);

        if let Some(parent) = Path::new(local_path).parent() {
            fs::create_dir_all(parent)
                .await
                .context("?�렉?�리 ?�성 ?�패")?;
        }

        let mut file = fs::File::create(local_path)
            .await
            .context("?�일 ?�성 ?�패")?;
        let mut stream = resp.bytes_stream();
        let mut received: u64 = 0;

        while let Some(chunk) = stream.next().await {
            if is_cancelled() {
                return Err(anyhow::anyhow!("Operation cancelled"));
            }
            let chunk = chunk.context("?�운로드 ?�트�??�류")?;
            file.write_all(&chunk).await.context("?�일 ?�기 ?�패")?;
            received += chunk.len() as u64;
            on_progress(received, total);
        }

        file.flush().await.context("?�일 flush ?�패")?;
        Ok(())
    }

    /// S3 ?�브?�트 ?�름 변�?(CopyObject ??DeleteObject)
    pub async fn rename_object(&self, src_key: &str, dst_key: &str) -> Result<()> {
        let copy_source = format!("/{}/{}", self.bucket, encode_key(src_key));
        let url = Url::parse(&format!("{}/{}", self.bucket_url(), encode_key(dst_key)))
            .context("URL ?�싱 ?�패")?;

        let headers = self.signer().sign_headers(
            "PUT",
            &url,
            &[
                ("x-amz-copy-source", &copy_source),
                ("content-length", "0"),
            ],
            b"",
        );

        let mut req = self
            .client
            .put(url.as_str())
            .header("x-amz-copy-source", &copy_source)
            .header("content-length", "0");
        for (k, v) in &headers {
            req = req.header(k.as_str(), v.as_str());
        }

        let resp = req.send().await.context("CopyObject HTTP PUT ?�패")?;
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("CopyObject ?�패: {}", body));
        }

        self.delete_objects(&[src_key.to_owned()])
            .await
            .context("?�본 ?�브?�트 ??�� ?�패")?;
        Ok(())
    }

    pub async fn presign_get(&self, key: &str, expires_in_seconds: u64) -> Result<String> {
        let now = chrono::Utc::now();
        let datetime = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date = now.format("%Y%m%d").to_string();
        let host = self
            .bucket_url()
            .trim_start_matches("https://")
            .split('/')
            .next()
            .unwrap_or("")
            .to_owned();

        let credential_scope = format!("{}/{}/s3/aws4_request", date, self.region);
        let credential = format!("{}/{}", self.creds.access_key_id, credential_scope);

        let raw = format!(
            "{}/{}?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Credential={}&X-Amz-Date={}&X-Amz-Expires={}&X-Amz-SignedHeaders=host",
            self.bucket_url(),
            encode_key(key),
            percent_encoding::utf8_percent_encode(&credential, percent_encoding::NON_ALPHANUMERIC),
            datetime,
            expires_in_seconds
        );

        let url = Url::parse(&raw).context("Presign URL ?�싱 ?�패")?;
        let canonical_query = {
            let mut pairs: Vec<(String, String)> = url
                .query_pairs()
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect();
            pairs.sort_by_key(|(k, _)| k.clone());
            pairs
                .iter()
                .map(|(k, v)| {
                    format!(
                        "{}={}",
                        percent_encoding::utf8_percent_encode(
                            k,
                            percent_encoding::NON_ALPHANUMERIC
                        ),
                        percent_encoding::utf8_percent_encode(
                            v,
                            percent_encoding::NON_ALPHANUMERIC
                        )
                    )
                })
                .collect::<Vec<_>>()
                .join("&")
        };

        let canonical_request = format!(
            "GET\n/{}\n{}\nhost:{}\n\nhost\nUNSIGNED-PAYLOAD",
            encode_key(key),
            canonical_query,
            host
        );

        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{}\n{}\n{}",
            datetime,
            credential_scope,
            crate::utils::sigv4::sha256_hex(canonical_request.as_bytes())
        );

        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        type HmacSha256 = Hmac<Sha256>;

        let k_date = {
            let mut m = HmacSha256::new_from_slice(
                format!("AWS4{}", self.creds.secret_access_key).as_bytes(),
            )
            .unwrap();
            m.update(date.as_bytes());
            m.finalize().into_bytes().to_vec()
        };
        let k_region = {
            let mut m = HmacSha256::new_from_slice(&k_date).unwrap();
            m.update(self.region.as_bytes());
            m.finalize().into_bytes().to_vec()
        };
        let k_service = {
            let mut m = HmacSha256::new_from_slice(&k_region).unwrap();
            m.update(b"s3");
            m.finalize().into_bytes().to_vec()
        };
        let k_signing = {
            let mut m = HmacSha256::new_from_slice(&k_service).unwrap();
            m.update(b"aws4_request");
            m.finalize().into_bytes().to_vec()
        };
        let signature = {
            let mut m = HmacSha256::new_from_slice(&k_signing).unwrap();
            m.update(string_to_sign.as_bytes());
            hex::encode(m.finalize().into_bytes())
        };

        Ok(format!("{}&X-Amz-Signature={}", raw, signature))
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
        let raw = format!("{}/{}?uploads", self.bucket_url(), encode_key(key));
        let url = Url::parse(&raw).context("URL ?�싱 ?�패")?;
        let resp = self
            .signed_post(&url, vec![], content_type, cache_control)
            .await?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("InitiateMultipartUpload ?�패: {}", body));
        }

        let text = resp.text().await.context("?�답 ?�기 ?�패")?;
        xml_extract(&text, "UploadId").context("UploadId ?�싱 ?�패")
    }

    async fn upload_part(
        &self,
        key: &str,
        upload_id: &str,
        part_number: u32,
        data: Vec<u8>,
    ) -> Result<String> {
        let raw = format!(
            "{}/{}?partNumber={}&uploadId={}",
            self.bucket_url(),
            encode_key(key),
            part_number,
            upload_id
        );
        let url = Url::parse(&raw).context("URL ?�싱 ?�패")?;

        // M-5: 지??백오???�시??(최�? 3??
        let mut delay_ms = 500u64;
        for attempt in 0u32..3 {
            let headers = self.signer().sign_headers("PUT", &url, &[], &data);
            let mut req = self.client.put(url.as_str()).body(data.clone());
            for (k, v) in &headers {
                req = req.header(k.as_str(), v.as_str());
            }
            match req.send().await {
                Err(e) if attempt < 2 => {
                    tracing::warn!("?�트 ?�로???�트?�크 ?�류 ?�시??{}/3 (part {}): {}", attempt + 1, part_number, e);
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                    delay_ms *= 2;
                    continue;
                }
                Err(e) => return Err(anyhow::anyhow!("HTTP PUT(part) ?�패: {}", e)),
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        return resp
                            .headers()
                            .get("etag")
                            .and_then(|v| v.to_str().ok())
                            .map(|e| e.trim_matches('"').to_owned())
                            .context("UploadPart ETag ?�더 ?�음");
                    }
                    let code = status.as_u16();
                    if attempt < 2 && is_retryable_status(code) {
                        tracing::warn!("?�트 ?�로???�시??{}/3 (part {}): HTTP {}", attempt + 1, part_number, code);
                        tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                        delay_ms *= 2;
                        continue;
                    }
                    let body = resp.text().await.unwrap_or_default();
                    return Err(anyhow::anyhow!("UploadPart ?�패 (part {}): {}", part_number, body));
                }
            }
        }
        unreachable!()
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
        let part_xml: String = parts
            .iter()
            .map(|(n, etag)| {
                format!(
                    "<Part><PartNumber>{}</PartNumber><ETag>{}</ETag></Part>",
                    n, etag
                )
            })
            .collect();

        let body = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
             <CompleteMultipartUpload>{}</CompleteMultipartUpload>",
            part_xml
        )
        .into_bytes();

        let raw = format!(
            "{}/{}?uploadId={}",
            self.bucket_url(),
            encode_key(key),
            upload_id
        );
        let url = Url::parse(&raw).context("URL ?�싱 ?�패")?;
        let resp = self
            .signed_post(&url, body, "application/xml", None)
            .await?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("CompleteMultipartUpload ?�패: {}", body));
        }

        let text = resp.text().await.context("?�답 ?�기 ?�패")?;
        xml_extract(&text, "ETag")
            .map(|e| e.trim_matches('"').to_owned())
            .context("CompleteMultipartUpload ETag ?�싱 ?�패")
    }

    async fn abort_multipart_upload(&self, key: &str, upload_id: &str) -> Result<()> {
        let raw = format!(
            "{}/{}?uploadId={}",
            self.bucket_url(),
            encode_key(key),
            upload_id
        );
        let url = Url::parse(&raw).context("URL ?�싱 ?�패")?;
        let resp = self.signed_delete(&url).await?;

        // 404???��? ?�료 ?�는 존재?��? ?�음 ??무시
        if !resp.status().is_success() && resp.status().as_u16() != 404 {
            return Err(anyhow::anyhow!(
                "AbortMultipartUpload ?�패: {}",
                resp.status()
            ));
        }
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
        let url = Url::parse(&format!("{}/{}", self.bucket_url(), encode_key(key)))
            .context("URL ?�싱 ?�패")?;
        let resp = self.signed_head(&url).await?;

        if resp.status().as_u16() == 404 {
            return Err(anyhow::anyhow!("?�브?�트 ?�음: {}", key));
        }
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("HeadObject ?�패: HTTP {}", resp.status()));
        }

        let headers = resp.headers();
        let etag = headers
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(|e| e.trim_matches('"').to_owned());
        let size = headers
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let last_modified = headers
            .get("last-modified")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();
        let content_type = headers
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_owned());

        Ok(ObjectMeta {
            key: key.to_owned(),
            size,
            etag,
            last_modified,
            content_type,
        })
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

fn should_retry_status(status: reqwest::StatusCode) -> bool {
    status.as_u16() == 408 || status.as_u16() == 429 || status.is_server_error()
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
