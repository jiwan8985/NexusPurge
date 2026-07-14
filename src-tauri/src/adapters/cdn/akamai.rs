use anyhow::{Context, Result};
use hmac::{Hmac, Mac};
use reqwest::Client;
use sha2::{Digest, Sha256};
use url::Url;

type HmacSha256 = Hmac<Sha256>;

pub struct AkamaiAdapter {
    client:        Client,
    client_token:  String,
    client_secret: String,
    access_token:  String,
    host:          String, // EdgeGrid API 호스트 (e.g. akab-xxxx.luna.akamaiapis.net)
}

impl AkamaiAdapter {
    pub fn new(
        client_token:  String,
        client_secret: String,
        access_token:  String,
        host:          String,
    ) -> Result<Self> {
        let client = Client::builder()
            .use_native_tls()
            .build()
            .context("HTTP 클라이언트 생성 실패")?;
        // 사용자가 스킴/슬래시를 붙여 입력해도 서명·요청 host가 일치하도록 정규화
        let host = host
            .trim()
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_end_matches('/')
            .to_owned();
        Ok(Self { client, client_token, client_secret, access_token, host })
    }

    /// Akamai EdgeGrid 서명 생성
    /// 참고: https://techdocs.akamai.com/developer/docs/authenticate-with-edgegrid
    fn sign_request(&self, method: &str, url: &Url, body: &[u8]) -> String {
        let now = chrono::Utc::now();
        let timestamp = now.format("%Y%m%dT%H:%M:%S+0000").to_string();
        let nonce = uuid::Uuid::new_v4().to_string();

        // 서명 없는 Authorization 헤더 prefix
        let auth_prefix = format!(
            "EG1-HMAC-SHA256 client_token={};access_token={};timestamp={};nonce={};",
            self.client_token, self.access_token, timestamp, nonce
        );

        // 요청 본문 SHA-256 해시 (base64)
        let content_hash = if body.is_empty() {
            String::new()
        } else {
            let mut hasher = Sha256::new();
            hasher.update(body);
            base64_encode(&hasher.finalize())
        };

        // 경로 + 쿼리스트링
        let path_query = match url.query() {
            Some(q) => format!("{}?{}", url.path(), q),
            None    => url.path().to_owned(),
        };

        // 서명 대상 문자열:
        // method \t scheme \t host \t path_query \t signed_headers \t content_hash \t auth_prefix
        let data_to_sign = format!(
            "{}\thttps\t{}\t{}\t\t{}\t{}",
            method.to_uppercase(),
            &self.host,
            path_query,
            content_hash,
            auth_prefix
        );

        // 서명 키 = base64(HMAC-SHA256(client_secret, timestamp))
        // 주의: EdgeGrid 스펙상 2차 HMAC의 키는 raw 바이트가 아니라 base64 "문자열"이다
        let signing_key = {
            let mut mac = HmacSha256::new_from_slice(self.client_secret.as_bytes())
                .expect("HMAC 초기화 실패");
            mac.update(timestamp.as_bytes());
            base64_encode(&mac.finalize().into_bytes())
        };

        // 서명 = base64(HMAC-SHA256(signing_key, data_to_sign))
        let signature = {
            let mut mac =
                HmacSha256::new_from_slice(signing_key.as_bytes()).expect("HMAC 초기화 실패");
            mac.update(data_to_sign.as_bytes());
            base64_encode(&mac.finalize().into_bytes())
        };

        format!("{}signature={}", auth_prefix, signature)
    }

    /// Akamai Fast Purge CCU v3 — URL 기반 무효화
    pub async fn purge_urls(&self, urls: &[String]) -> Result<()> {
        if urls.is_empty() { return Ok(()); }

        let endpoint = format!("https://{}/ccu/v3/invalidate/url/production", &self.host);
        let url = Url::parse(&endpoint).context("Akamai URL 파싱 실패")?;

        let body = serde_json::json!({ "objects": urls }).to_string();
        let body_bytes = body.as_bytes();
        let auth_header = self.sign_request("POST", &url, body_bytes);

        let started = std::time::Instant::now();
        let resp = self
            .client
            .post(url.as_str())
            .header("Authorization", &auth_header)
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await
            .context("Akamai Fast Purge 요청 실패")?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        crate::adapters::cdn::log_cdn_http(
            "Akamai",
            "POST(URL Purge)",
            url.as_str(),
            status,
            started.elapsed().as_millis(),
            &text,
        );
        if !status.is_success() {
            return Err(anyhow::anyhow!(
                "Akamai Purge 실패 (HTTP {}): {}",
                status,
                text
            ));
        }

        tracing::info!("Akamai Purge 성공: {} URL", urls.len());
        Ok(())
    }

    /// Fast Purge v3에는 조회용 GET 엔드포인트가 없으므로,
    /// 빈 objects로 invalidate를 호출해 인증 여부만 판별한다:
    /// 400(본문 유효성 오류) = 인증 통과, 401/403 = 자격증명·권한 실패
    pub async fn test_fast_purge_access(&self) -> Result<()> {
        if self.host.trim().is_empty() {
            return Err(anyhow::anyhow!("Akamai EdgeGrid 호스트가 필요합니다"));
        }

        let endpoint = format!("https://{}/ccu/v3/invalidate/url/production", &self.host);
        let url = Url::parse(&endpoint).context("Akamai URL 파싱 실패")?;

        let body = serde_json::json!({ "objects": [] }).to_string();
        let auth_header = self.sign_request("POST", &url, body.as_bytes());

        let resp = self
            .client
            .post(url.as_str())
            .header("Authorization", &auth_header)
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await
            .context("Akamai Fast Purge 권한 테스트 요청 실패")?;

        let status = resp.status();
        if status.is_success() || status.as_u16() == 400 {
            // 400은 "objects 비어 있음" 유효성 오류 — 서명·권한은 통과했다는 뜻
            return Ok(());
        }

        let text = resp.text().await.unwrap_or_default();
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(anyhow::anyhow!(
                "Akamai 인증 실패 (HTTP {}): client_token/client_secret/access_token 또는 EdgeGrid 호스트를 확인하세요. {}",
                status,
                text
            ));
        }
        Err(anyhow::anyhow!(
            "Akamai Fast Purge 권한 테스트 실패 (HTTP {}): {}",
            status,
            text
        ))
    }

    /// Akamai Fast Purge CCU v3 — CP Code 기반 무효화 (해당 CP Code 전체 캐시 무효화)
    pub async fn purge_cp_codes(&self, cp_codes: &[u64]) -> Result<()> {
        if cp_codes.is_empty() { return Ok(()); }

        let endpoint = format!("https://{}/ccu/v3/invalidate/cpcode/production", &self.host);
        let url = Url::parse(&endpoint).context("Akamai URL 파싱 실패")?;

        let body = serde_json::json!({ "objects": cp_codes }).to_string();
        let auth_header = self.sign_request("POST", &url, body.as_bytes());

        let started = std::time::Instant::now();
        let resp = self
            .client
            .post(url.as_str())
            .header("Authorization", &auth_header)
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await
            .context("Akamai CP Code Purge 요청 실패")?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        crate::adapters::cdn::log_cdn_http(
            "Akamai",
            "POST(CP Code Purge)",
            url.as_str(),
            status,
            started.elapsed().as_millis(),
            &text,
        );
        if !status.is_success() {
            return Err(anyhow::anyhow!(
                "Akamai CP Code Purge 실패 (HTTP {}): {}",
                status,
                text
            ));
        }

        tracing::info!("Akamai CP Code Purge 성공: {:?}", cp_codes);
        Ok(())
    }
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
