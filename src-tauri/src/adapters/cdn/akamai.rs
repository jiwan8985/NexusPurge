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
    pub cp_code:   String, // Akamai CP Code (콘텐츠 제공자 코드)
}

impl AkamaiAdapter {
    pub fn new(
        client_token:  String,
        client_secret: String,
        access_token:  String,
        host:          String,
        cp_code:       String,
    ) -> Result<Self> {
        let client = Client::builder()
            .use_native_tls()
            .build()
            .context("HTTP 클라이언트 생성 실패")?;
        Ok(Self { client, client_token, client_secret, access_token, host, cp_code })
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

        // 서명 키 = HMAC-SHA256(client_secret, timestamp)
        let signing_key = {
            let mut mac = HmacSha256::new_from_slice(self.client_secret.as_bytes())
                .expect("HMAC 초기화 실패");
            mac.update(timestamp.as_bytes());
            mac.finalize().into_bytes()
        };

        // 서명 = base64(HMAC-SHA256(signing_key, data_to_sign))
        let signature = {
            let mut mac = HmacSha256::new_from_slice(&signing_key).expect("HMAC 초기화 실패");
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

        let resp = self
            .client
            .post(url.as_str())
            .header("Authorization", &auth_header)
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await
            .context("Akamai Fast Purge 요청 실패")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Akamai Purge 실패 (HTTP {}): {}",
                status,
                text
            ));
        }

        tracing::info!("Akamai Purge 성공: {} URL", urls.len());
        Ok(())
    }

    /// Akamai Fast Purge CCU v3 — CP Code 기반 전체 무효화
    pub async fn purge_by_cpcode(&self) -> Result<()> {
        if self.cp_code.trim().is_empty() {
            return Err(anyhow::anyhow!("Akamai CP Code가 필요합니다"));
        }
        let cp_code_num: u64 = self.cp_code.trim().parse()
            .map_err(|_| anyhow::anyhow!("CP Code는 숫자여야 합니다: {}", self.cp_code))?;

        let endpoint = format!("https://{}/ccu/v3/invalidate/cpcode/production", &self.host);
        let url = Url::parse(&endpoint).context("Akamai URL 파싱 실패")?;

        let body = serde_json::json!({ "objects": [cp_code_num] }).to_string();
        let body_bytes = body.as_bytes();
        let auth_header = self.sign_request("POST", &url, body_bytes);

        let resp = self
            .client
            .post(url.as_str())
            .header("Authorization", &auth_header)
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await
            .context("Akamai CP Code Purge 요청 실패")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Akamai CP Code Purge 실패 (HTTP {}): {}",
                status,
                text
            ));
        }

        tracing::info!("Akamai CP Code Purge 성공: CP Code {}", self.cp_code);
        Ok(())
    }

    pub async fn test_fast_purge_access(&self) -> Result<()> {
        if self.host.trim().is_empty() {
            return Err(anyhow::anyhow!("Akamai EdgeGrid 호스트가 필요합니다"));
        }

        let endpoint = format!("https://{}/ccu/v3/rate-limit-status/url", &self.host);
        let url = Url::parse(&endpoint).context("Akamai URL 파싱 실패")?;
        let auth_header = self.sign_request("GET", &url, b"");

        let resp = self
            .client
            .get(url.as_str())
            .header("Authorization", &auth_header)
            .send()
            .await
            .context("Akamai Fast Purge 권한 테스트 요청 실패")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Akamai Fast Purge 권한 테스트 실패 (HTTP {}): {}",
                status,
                text
            ));
        }

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
