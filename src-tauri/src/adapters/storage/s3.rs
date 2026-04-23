use anyhow::{Context, Result};
use reqwest::Client;
use std::path::Path;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use url::Url;

use crate::adapters::storage::base::ListResult;
use crate::commands::s3::FileItem;
use crate::utils::config::AwsCredentials;
use crate::utils::sigv4::Signer;

#[derive(Clone)]
pub struct S3Adapter {
    client:    Client,
    endpoint:  String,   // "https://s3.{region}.amazonaws.com" or custom
    bucket:    String,
    region:    String,
    creds:     AwsCredentials,
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

    // path-style URL: {endpoint}/{bucket}/{key}
    fn bucket_url(&self) -> String {
        format!("{}/{}", self.endpoint, self.bucket)
    }

    // ── 공통 서명 + 전송 헬퍼 ────────────────────────────────────────────

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

    async fn signed_put(&self, url: &Url, body: Vec<u8>, content_type: &str) -> Result<reqwest::Response> {
        let headers = self.signer().sign_headers("PUT", url, &[("content-type", content_type)], &body);
        let mut req = self.client.put(url.as_str()).header("content-type", content_type).body(body);
        for (k, v) in &headers {
            req = req.header(k.as_str(), v.as_str());
        }
        req.send().await.context("HTTP PUT 실패")
    }

    async fn signed_post(&self, url: &Url, body: Vec<u8>, content_type: &str) -> Result<reqwest::Response> {
        let headers = self.signer().sign_headers("POST", url, &[("content-type", content_type)], &body);
        let mut req = self.client.post(url.as_str()).header("content-type", content_type).body(body);
        for (k, v) in &headers {
            req = req.header(k.as_str(), v.as_str());
        }
        req.send().await.context("HTTP POST 실패")
    }

    // ── Public Operations ─────────────────────────────────────────────────

    pub async fn verify_access(&self) -> Result<()> {
        let url = Url::parse(&format!("{}/?list-type=2&max-keys=1", self.bucket_url()))
            .context("URL 파싱 실패")?;
        let resp = self.signed_get(&url).await?;
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("버킷 접근 실패: HTTP {}", resp.status()));
        }
        Ok(())
    }

    pub async fn list_objects(&self, prefix: &str) -> Result<ListResult> {
        let encoded_prefix = percent_encoding::utf8_percent_encode(
            prefix,
            percent_encoding::NON_ALPHANUMERIC,
        )
        .to_string()
        .replace("%2F", "/"); // 슬래시는 인코딩 안 함 (S3 관례)

        let raw = format!(
            "{}/?list-type=2&prefix={}&delimiter=%2F&max-keys=1000",
            self.bucket_url(),
            encoded_prefix
        );
        let url = Url::parse(&raw).context("URL 파싱 실패")?;
        let resp = self.signed_get(&url).await?;
        let text = resp.text().await.context("응답 읽기 실패")?;

        parse_list_response(&text)
    }

    pub async fn head_object(&self, key: &str) -> Result<Option<String>> {
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
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Delete><Quiet>true</Quiet>{}</Delete>"#,
            items
        )
        .into_bytes();

        let content_md5 = base64_md5(&body);
        let url = Url::parse(&format!("{}/?delete", self.bucket_url()))
            .context("URL 파싱 실패")?;

        let headers = self
            .signer()
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
            return Err(anyhow::anyhow!("DeleteObjects 실패: HTTP {}", resp.status()));
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

    pub async fn upload_file(
        &self,
        local_path: &str,
        remote_key: &str,
        on_progress: impl Fn(u64, u64) + Send + 'static,
    ) -> Result<()> {
        let metadata = fs::metadata(local_path).await.context("파일 메타데이터 읽기 실패")?;
        let total = metadata.len();
        let content_type = mime_guess::from_path(local_path)
            .first_or_octet_stream()
            .to_string();

        on_progress(0, total);

        // TODO: 100MB 초과 시 Multipart Upload 전환
        let data = fs::read(local_path).await.context("파일 읽기 실패")?;

        let url = Url::parse(&format!("{}/{}", self.bucket_url(), encode_key(remote_key)))
            .context("URL 파싱 실패")?;
        let resp = self.signed_put(&url, data, &content_type).await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("업로드 실패 ({}): {}", status, body));
        }

        on_progress(total, total);
        Ok(())
    }

    pub async fn download_file(
        &self,
        remote_key: &str,
        local_path: &str,
        on_progress: impl Fn(u64, u64) + Send + 'static,
    ) -> Result<()> {
        use futures::StreamExt;

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
            fs::create_dir_all(parent).await.context("디렉토리 생성 실패")?;
        }

        let mut file = fs::File::create(local_path).await.context("파일 생성 실패")?;
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

    pub async fn presign_get(&self, key: &str, expires_in_seconds: u64) -> Result<String> {
        // SigV4 presigned URL (query string 방식)
        let now = chrono::Utc::now();
        let datetime = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date     = now.format("%Y%m%d").to_string();
        let host     = self.bucket_url().trim_start_matches("https://").split('/').next().unwrap_or("").to_owned();

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
            let mut pairs: Vec<(String, String)> = url.query_pairs().map(|(k, v)| (k.into_owned(), v.into_owned())).collect();
            pairs.sort_by_key(|(k, _)| k.clone());
            pairs.iter()
                .map(|(k, v)| format!("{}={}", percent_encoding::utf8_percent_encode(k, percent_encoding::NON_ALPHANUMERIC), percent_encoding::utf8_percent_encode(v, percent_encoding::NON_ALPHANUMERIC)))
                .collect::<Vec<_>>()
                .join("&")
        };

        let canonical_request = format!(
            "GET\n/{}\n{}\nhost:{}\n\nhost\nUNSIGNED-PAYLOAD",
            encode_key(key), canonical_query, host
        );

        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{}\n{}\n{}",
            datetime, credential_scope,
            crate::utils::sigv4::sha256_hex(canonical_request.as_bytes())
        );

        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        type HmacSha256 = Hmac<Sha256>;

        let k_date    = {let mut m = HmacSha256::new_from_slice(format!("AWS4{}", self.creds.secret_access_key).as_bytes()).unwrap(); m.update(date.as_bytes()); m.finalize().into_bytes().to_vec()};
        let k_region  = {let mut m = HmacSha256::new_from_slice(&k_date).unwrap();   m.update(self.region.as_bytes());    m.finalize().into_bytes().to_vec()};
        let k_service = {let mut m = HmacSha256::new_from_slice(&k_region).unwrap(); m.update(b"s3");                     m.finalize().into_bytes().to_vec()};
        let k_signing = {let mut m = HmacSha256::new_from_slice(&k_service).unwrap(); m.update(b"aws4_request");          m.finalize().into_bytes().to_vec()};
        let signature = {let mut m = HmacSha256::new_from_slice(&k_signing).unwrap(); m.update(string_to_sign.as_bytes()); hex::encode(m.finalize().into_bytes())};

        Ok(format!("{}&X-Amz-Signature={}", raw, signature))
    }
}

// ── XML Parsing ───────────────────────────────────────────────────────────────

fn parse_list_response(xml: &str) -> Result<ListResult> {
    let mut files: Vec<FileItem> = vec![];

    // 폴더 (CommonPrefixes)
    let mut search = xml;
    while let Some(start) = search.find("<CommonPrefixes>") {
        let rest = &search[start + "<CommonPrefixes>".len()..];
        if let Some(prefix) = xml_extract(rest, "Prefix") {
            let name = prefix.trim_end_matches('/').rsplit('/').next().unwrap_or(&prefix).to_owned();
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
                    size: xml_extract(block, "Size").and_then(|s| s.parse().ok()).unwrap_or(0),
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
    let next_token   = xml_extract(xml, "NextContinuationToken");

    Ok(ListResult { files, next_token, is_truncated })
}

fn xml_extract(xml: &str, tag: &str) -> Option<String> {
    let open  = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = xml.find(&open)? + open.len();
    let end   = xml[start..].find(&close)? + start;
    Some(xml[start..end].to_owned())
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
        .map(|seg| percent_encoding::utf8_percent_encode(seg, percent_encoding::NON_ALPHANUMERIC).to_string().replace("%7E", "~"))
        .collect::<Vec<_>>()
        .join("/")
}

fn base64_md5(data: &[u8]) -> String {
    let hash = md5::compute(data);
    base64_encode(hash.as_ref())
}

fn base64_encode(input: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i + 2 < input.len() {
        let b = ((input[i] as u32) << 16) | ((input[i+1] as u32) << 8) | (input[i+2] as u32);
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
            let b = ((input[i] as u32) << 16) | ((input[i+1] as u32) << 8);
            out.push(CHARS[(b >> 18) as usize] as char);
            out.push(CHARS[((b >> 12) & 0x3f) as usize] as char);
            out.push(CHARS[((b >> 6) & 0x3f) as usize] as char);
            out.push('=');
        }
        _ => {}
    }
    out
}
