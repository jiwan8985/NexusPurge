use anyhow::{Context, Result};
use reqwest::Client;
use std::path::Path;
use tokio::fs;
use tokio::io::AsyncReadExt;
use tokio::task::JoinSet;
use url::Url;

use crate::adapters::storage::base::{
    ListResult, ObjectMeta, Progress, RemoteFile, StorageAdapter, UploadResult,
};
use crate::commands::s3::FileItem;
use crate::utils::config::AwsCredentials;
use crate::utils::sigv4::Signer;

// ─── Constants ────────────────────────────────────────────────────────────────

/// 이 크기 이상이면 멀티파트 업로드로 전환
pub const MULTIPART_THRESHOLD: u64 = 10 * 1024 * 1024; // 10 MB
/// 파트당 크기 (S3 최소 5 MB, 마지막 파트 제외)
pub const PART_SIZE: usize = 10 * 1024 * 1024; // 10 MB
/// 동시 파트 업로드 수
const MAX_CONCURRENT_PARTS: usize = 4;

// ─── S3Adapter ────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct S3Adapter {
    client:   Client,
    endpoint: String, // "https://s3.{region}.amazonaws.com" or custom
    bucket:   String,
    region:   String,
    creds:    AwsCredentials,
}

impl S3Adapter {
    pub fn new(
        region:   &str,
        bucket:   &str,
        creds:    &AwsCredentials,
        endpoint: Option<&str>,
    ) -> Result<Self> {
        let client = Client::builder()
            .use_native_tls()
            .build()
            .context("HTTP 클라이언트 생성 실패")?;

        let ep = endpoint
            .map(|s| s.trim_end_matches('/').to_owned())
            .unwrap_or_else(|| format!("https://s3.{}.amazonaws.com", region));

        Ok(Self {
            client,
            endpoint: ep,
            bucket: bucket.to_owned(),
            region: region.to_owned(),
            creds: creds.clone(),
        })
    }

    fn signer(&self) -> Signer<'_> {
        Signer {
            access_key_id:     &self.creds.access_key_id,
            secret_access_key: &self.creds.secret_access_key,
            region:            &self.region,
            service:           "s3",
        }
    }

    fn bucket_url(&self) -> String {
        format!("{}/{}", self.endpoint, self.bucket)
    }

    // ── Signed HTTP Helpers ───────────────────────────────────────────────────

    async fn signed_get(&self, url: &Url) -> Result<reqwest::Response> {
        let headers = self.signer().sign_headers("GET", url, &[], b"");
        let mut req = self.client.get(url.as_str());
        for (k, v) in &headers {
            req = req.header(k.as_str(), v.as_str());
        }
        req.send().await.context("HTTP GET 실패")
    }

    async fn signed_head(&self, url: &Url) -> Result<reqwest::Response> {
        let headers = self.signer().sign_headers("HEAD", url, &[], b"");
        let mut req = self.client.head(url.as_str());
        for (k, v) in &headers {
            req = req.header(k.as_str(), v.as_str());
        }
        req.send().await.context("HTTP HEAD 실패")
    }

    async fn signed_put(
        &self,
        url: &Url,
        body: Vec<u8>,
        content_type: &str,
    ) -> Result<reqwest::Response> {
        let headers =
            self.signer()
                .sign_headers("PUT", url, &[("content-type", content_type)], &body);
        let mut req = self
            .client
            .put(url.as_str())
            .header("content-type", content_type)
            .body(body);
        for (k, v) in &headers {
            req = req.header(k.as_str(), v.as_str());
        }
        req.send().await.context("HTTP PUT 실패")
    }

    async fn signed_post(
        &self,
        url: &Url,
        body: Vec<u8>,
        content_type: &str,
    ) -> Result<reqwest::Response> {
        let headers =
            self.signer()
                .sign_headers("POST", url, &[("content-type", content_type)], &body);
        let mut req = self
            .client
            .post(url.as_str())
            .header("content-type", content_type)
            .body(body);
        for (k, v) in &headers {
            req = req.header(k.as_str(), v.as_str());
        }
        req.send().await.context("HTTP POST 실패")
    }

    async fn signed_delete(&self, url: &Url) -> Result<reqwest::Response> {
        let headers = self.signer().sign_headers("DELETE", url, &[], b"");
        let mut req = self.client.delete(url.as_str());
        for (k, v) in &headers {
            req = req.header(k.as_str(), v.as_str());
        }
        req.send().await.context("HTTP DELETE 실패")
    }

    // ── Public Operations ─────────────────────────────────────────────────────

    pub async fn verify_access(&self) -> Result<()> {
        let url =
            Url::parse(&format!("{}/?list-type=2&max-keys=1", self.bucket_url()))
                .context("URL 파싱 실패")?;
        let resp = self.signed_get(&url).await?;
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("버킷 접근 실패: HTTP {}", resp.status()));
        }
        Ok(())
    }

    /// 단일 페이지 목록 조회 (내부용)
    async fn list_objects_page(
        &self,
        prefix: &str,
        continuation_token: Option<&str>,
    ) -> Result<ListResult> {
        let encoded_prefix = percent_encoding::utf8_percent_encode(
            prefix,
            percent_encoding::NON_ALPHANUMERIC,
        )
        .to_string()
        .replace("%2F", "/");

        let mut raw = format!(
            "{}/?list-type=2&prefix={}&delimiter=%2F&max-keys=1000",
            self.bucket_url(),
            encoded_prefix
        );
        if let Some(token) = continuation_token {
            let encoded_token = percent_encoding::utf8_percent_encode(
                token,
                percent_encoding::NON_ALPHANUMERIC,
            )
            .to_string();
            raw.push_str(&format!("&continuation-token={}", encoded_token));
        }

        let url = Url::parse(&raw).context("URL 파싱 실패")?;
        let resp = self.signed_get(&url).await?;
        // C-3 + H-4: HTTP 상태 확인 — 오류 응답을 빈 목록으로 오인하지 않음
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "S3 목록 조회 실패 (HTTP {}): {}",
                status,
                body
            ));
        }
        let text = resp.text().await.context("응답 읽기 실패")?;
        parse_list_response(&text)
    }

    /// C-3: 전체 페이지를 순회해 1000개 초과 오브젝트를 모두 반환
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

    /// 오브젝트 목록 (기존 FileItem 타입, 하위 호환용) — 내부적으로 전체 페이지 조회
    pub async fn list_objects_raw(&self, prefix: &str) -> Result<ListResult> {
        self.list_objects_all(prefix).await
    }

    /// ETag만 반환 (sync 플랜 비교용)
    pub async fn head_object_etag(&self, key: &str) -> Result<Option<String>> {
        let url = Url::parse(&format!("{}/{}", self.bucket_url(), encode_key(key)))
            .context("URL 파싱 실패")?;
        let resp = self.signed_head(&url).await?;

        if resp.status().as_u16() == 404 {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("HeadObject 실패: HTTP {}", resp.status()));
        }

        let etag = resp
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(|e| e.trim_matches('"').to_owned());

        Ok(etag)
    }

    pub async fn delete_objects(&self, keys: &[String]) -> Result<()> {
        if keys.is_empty() {
            return Ok(());
        }

        let items: String = keys
            .iter()
            .map(|k| format!("<Object><Key>{}</Key></Object>", xml_escape(k)))
            .collect();

        let body = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <Delete><Quiet>true</Quiet>{}</Delete>",
            items
        )
        .into_bytes();

        let content_md5 = base64_md5(&body);
        let url = Url::parse(&format!("{}/?delete", self.bucket_url()))
            .context("URL 파싱 실패")?;

        let headers =
            self.signer()
                .sign_headers("POST", &url, &[("content-md5", &content_md5)], &body);

        let mut req = self
            .client
            .post(url.as_str())
            .header("content-type", "application/xml")
            .header("content-md5", &content_md5)
            .body(body);
        for (k, v) in &headers {
            req = req.header(k.as_str(), v.as_str());
        }

        let resp = req.send().await.context("HTTP POST(delete) 실패")?;
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!(
                "DeleteObjects 실패: HTTP {}",
                resp.status()
            ));
        }
        Ok(())
    }

    pub async fn put_object(&self, key: &str, data: Vec<u8>, content_type: &str) -> Result<()> {
        let url = Url::parse(&format!("{}/{}", self.bucket_url(), encode_key(key)))
            .context("URL 파싱 실패")?;
        let resp = self.signed_put(&url, data, content_type).await?;
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("PutObject 실패: HTTP {}", resp.status()));
        }
        Ok(())
    }

    /// 파일 업로드. 10 MB 이상은 자동으로 멀티파트 업로드.
    /// on_progress(transferred, total) 콜백으로 진행률 전달
    pub async fn upload_with_progress(
        &self,
        local_path: &str,
        remote_key: &str,
        on_progress: impl Fn(u64, u64),
    ) -> Result<UploadResult> {
        let metadata = fs::metadata(local_path)
            .await
            .context("파일 메타데이터 읽기 실패")?;
        let total = metadata.len();
        let content_type = mime_guess::from_path(local_path)
            .first_or_octet_stream()
            .to_string();

        if total >= MULTIPART_THRESHOLD {
            self.upload_multipart(local_path, remote_key, &content_type, total, on_progress)
                .await
        } else {
            self.upload_single(local_path, remote_key, &content_type, total, on_progress)
                .await
        }
    }

    /// 스트리밍 다운로드
    pub async fn download_with_progress(
        &self,
        remote_key: &str,
        local_path: &str,
        on_progress: impl Fn(u64, u64),
    ) -> Result<()> {
        use futures::StreamExt;
        use tokio::io::AsyncWriteExt;

        let url = Url::parse(&format!("{}/{}", self.bucket_url(), encode_key(remote_key)))
            .context("URL 파싱 실패")?;
        let resp = self.signed_get(&url).await?;

        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("GetObject 실패: HTTP {}", resp.status()));
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
                .context("디렉토리 생성 실패")?;
        }

        let mut file = fs::File::create(local_path)
            .await
            .context("파일 생성 실패")?;
        let mut stream = resp.bytes_stream();
        let mut received: u64 = 0;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("다운로드 스트림 오류")?;
            file.write_all(&chunk).await.context("파일 쓰기 실패")?;
            received += chunk.len() as u64;
            on_progress(received, total);
        }

        file.flush().await.context("파일 flush 실패")?;
        Ok(())
    }

    /// S3 오브젝트 이름 변경 (CopyObject → DeleteObject)
    pub async fn rename_object(&self, src_key: &str, dst_key: &str) -> Result<()> {
        let copy_source = format!("/{}/{}", self.bucket, encode_key(src_key));
        let url = Url::parse(&format!("{}/{}", self.bucket_url(), encode_key(dst_key)))
            .context("URL 파싱 실패")?;

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

        let resp = req.send().await.context("CopyObject HTTP PUT 실패")?;
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("CopyObject 실패: {}", body));
        }

        self.delete_objects(&[src_key.to_owned()])
            .await
            .context("원본 오브젝트 삭제 실패")
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

        let url = Url::parse(&raw).context("Presign URL 파싱 실패")?;
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

    // ── Multipart Upload Internals ────────────────────────────────────────────

    async fn upload_single(
        &self,
        local_path: &str,
        remote_key: &str,
        content_type: &str,
        total: u64,
        on_progress: impl Fn(u64, u64),
    ) -> Result<UploadResult> {
        on_progress(0, total);

        let data = fs::read(local_path).await.context("파일 읽기 실패")?;

        let url = Url::parse(&format!("{}/{}", self.bucket_url(), encode_key(remote_key)))
            .context("URL 파싱 실패")?;
        let resp = self.signed_put(&url, data, content_type).await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("업로드 실패 ({}): {}", status, body));
        }

        let etag = resp
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(|e| e.trim_matches('"').to_owned());

        on_progress(total, total);
        Ok(UploadResult {
            key: remote_key.to_owned(),
            etag,
            size: total,
            is_multipart: false,
        })
    }

    /// 슬라이딩 윈도우 방식: 최대 4개 파트를 동시에 업로드
    /// 최대 메모리 사용량 = MAX_CONCURRENT_PARTS × PART_SIZE = 40 MB
    async fn upload_multipart(
        &self,
        local_path: &str,
        remote_key: &str,
        content_type: &str,
        total: u64,
        on_progress: impl Fn(u64, u64),
    ) -> Result<UploadResult> {
        on_progress(0, total);

        let upload_id = self
            .initiate_multipart_upload(remote_key, content_type)
            .await?;

        let mut file = fs::File::open(local_path)
            .await
            .context("파일 열기 실패")?;

        let mut part_num: u32 = 1;
        let mut all_etags: Vec<(u32, String)> = Vec::new();
        let mut transferred: u64 = 0;

        loop {
            // 파트 배치 읽기 (최대 MAX_CONCURRENT_PARTS 개)
            let mut batch: Vec<(u32, Vec<u8>)> = Vec::new();
            while batch.len() < MAX_CONCURRENT_PARTS {
                let mut chunk = vec![0u8; PART_SIZE];
                let mut filled = 0;

                // 부분 읽기 처리: PART_SIZE 또는 EOF 까지 채움
                while filled < PART_SIZE {
                    let n = file
                        .read(&mut chunk[filled..])
                        .await
                        .context("파일 읽기 실패")?;
                    if n == 0 {
                        break; // EOF
                    }
                    filled += n;
                }

                if filled == 0 {
                    break; // 배치 내 EOF
                }
                chunk.truncate(filled);
                batch.push((part_num, chunk));
                part_num += 1;
            }

            if batch.is_empty() {
                break; // 파일 끝
            }

            // 배치 병렬 업로드
            let mut tasks: JoinSet<Result<(u32, String, u64)>> = JoinSet::new();
            for (num, data) in batch {
                let adapter = self.clone();
                let key = remote_key.to_owned();
                let uid = upload_id.clone();
                let size = data.len() as u64;

                tasks.spawn(async move {
                    let etag = adapter.upload_part(&key, &uid, num, data).await?;
                    Ok((num, etag, size))
                });
            }

            while let Some(result) = tasks.join_next().await {
                match result {
                    Ok(Ok((num, etag, bytes))) => {
                        transferred += bytes;
                        on_progress(transferred, total);
                        all_etags.push((num, etag));
                    }
                    Ok(Err(e)) => {
                        let _ = self.abort_multipart_upload(remote_key, &upload_id).await;
                        return Err(e.context("파트 업로드 실패"));
                    }
                    Err(join_err) => {
                        let _ = self.abort_multipart_upload(remote_key, &upload_id).await;
                        return Err(anyhow::anyhow!("파트 업로드 태스크 패닉: {}", join_err));
                    }
                }
            }
        }

        // 파트 번호 순으로 정렬 후 완료
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
    ) -> Result<String> {
        let raw = format!("{}/{}?uploads", self.bucket_url(), encode_key(key));
        let url = Url::parse(&raw).context("URL 파싱 실패")?;
        let resp = self.signed_post(&url, vec![], content_type).await?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("InitiateMultipartUpload 실패: {}", body));
        }

        let text = resp.text().await.context("응답 읽기 실패")?;
        xml_extract(&text, "UploadId").context("UploadId 파싱 실패")
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
        let url = Url::parse(&raw).context("URL 파싱 실패")?;

        let headers = self.signer().sign_headers("PUT", &url, &[], &data);
        let mut req = self.client.put(url.as_str()).body(data);
        for (k, v) in &headers {
            req = req.header(k.as_str(), v.as_str());
        }

        let resp = req.send().await.context("HTTP PUT(part) 실패")?;
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "UploadPart 실패 (part {}): {}",
                part_number,
                body
            ));
        }

        resp.headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(|e| e.trim_matches('"').to_owned())
            .context("UploadPart ETag 헤더 없음")
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
        let url = Url::parse(&raw).context("URL 파싱 실패")?;
        let resp = self.signed_post(&url, body, "application/xml").await?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("CompleteMultipartUpload 실패: {}", body));
        }

        let text = resp.text().await.context("응답 읽기 실패")?;
        xml_extract(&text, "ETag")
            .map(|e| e.trim_matches('"').to_owned())
            .context("CompleteMultipartUpload ETag 파싱 실패")
    }

    async fn abort_multipart_upload(&self, key: &str, upload_id: &str) -> Result<()> {
        let raw = format!(
            "{}/{}?uploadId={}",
            self.bucket_url(),
            encode_key(key),
            upload_id
        );
        let url = Url::parse(&raw).context("URL 파싱 실패")?;
        let resp = self.signed_delete(&url).await?;

        // 404는 이미 완료 또는 존재하지 않음 — 무시
        if !resp.status().is_success() && resp.status().as_u16() != 404 {
            return Err(anyhow::anyhow!(
                "AbortMultipartUpload 실패: {}",
                resp.status()
            ));
        }
        Ok(())
    }
}

// ─── StorageAdapter Trait Impl ────────────────────────────────────────────────

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
        let local_str = local.to_str().context("유효하지 않은 파일 경로")?;
        // tx.send() 는 언바운드 채널로 절대 블로킹되지 않음
        self.upload_with_progress(local_str, key, move |transferred, total| {
            let _ = tx.send(Progress { transferred, total });
        })
        .await
    }

    async fn download_file(
        &self,
        key: &str,
        local: &Path,
        tx: tokio::sync::mpsc::UnboundedSender<Progress>,
    ) -> Result<()> {
        let local_str = local.to_str().context("유효하지 않은 파일 경로")?;
        self.download_with_progress(key, local_str, move |transferred, total| {
            let _ = tx.send(Progress { transferred, total });
        })
        .await
    }

    async fn delete_object(&self, key: &str) -> Result<()> {
        self.delete_objects(&[key.to_owned()]).await
    }

    async fn rename_object(&self, old_key: &str, new_key: &str) -> Result<()> {
        self.rename_object(old_key, new_key).await
    }

    async fn head_object(&self, key: &str) -> Result<ObjectMeta> {
        let url = Url::parse(&format!("{}/{}", self.bucket_url(), encode_key(key)))
            .context("URL 파싱 실패")?;
        let resp = self.signed_head(&url).await?;

        if resp.status().as_u16() == 404 {
            return Err(anyhow::anyhow!("오브젝트 없음: {}", key));
        }
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("HeadObject 실패: HTTP {}", resp.status()));
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

// ─── XML Parsing ──────────────────────────────────────────────────────────────

fn parse_list_response(xml: &str) -> Result<ListResult> {
    let mut files: Vec<FileItem> = vec![];

    // 폴더 (CommonPrefixes)
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

    // 파일 (Contents)
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

/// H-5: XML entity 디코딩 — &amp; &lt; &gt; &quot; &apos;
fn xml_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
     .replace("&lt;", "<")
     .replace("&gt;", ">")
     .replace("&quot;", "\"")
     .replace("&apos;", "'")
}

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

fn base64_md5(data: &[u8]) -> String {
    let hash = md5::compute(data);
    base64_encode(hash.as_ref())
}

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
