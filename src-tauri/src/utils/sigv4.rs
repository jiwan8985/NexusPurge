/// AWS SigV4 서명 (순수 Rust — hmac + sha2만 사용, C 라이브러리 없음)
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

pub struct Signer<'a> {
    pub access_key_id:     &'a str,
    pub secret_access_key: &'a str,
    pub region:            &'a str,
    pub service:           &'a str,
}

impl<'a> Signer<'a> {
    /// HTTP 요청에 추가할 SigV4 서명 헤더 목록 반환
    /// `headers_to_sign`: (lowercase-name, value) 쌍 (host 포함 필수)
    pub fn sign_headers(
        &self,
        method:          &str,
        url:             &url::Url,
        extra_headers:   &[(&str, &str)], // host는 포함하지 말 것 (자동 추가)
        body:            &[u8],
    ) -> Vec<(String, String)> {
        let now = Utc::now();
        let date     = now.format("%Y%m%d").to_string();
        let datetime = now.format("%Y%m%dT%H%M%SZ").to_string();

        let host       = url.host_str().unwrap_or("");
        let body_hash  = sha256_hex(body);

        // ── 헤더 정렬 (lowercase, alphabetical) ──────────────────────────
        let mut headers: Vec<(&str, String)> = vec![
            ("host",                 host.to_owned()),
            ("x-amz-content-sha256", body_hash.clone()),
            ("x-amz-date",           datetime.clone()),
        ];
        for (k, v) in extra_headers {
            headers.push((k, v.to_string()));
        }
        headers.sort_by_key(|(k, _)| *k);

        let canonical_headers: String = headers
            .iter()
            .map(|(k, v)| format!("{}:{}\n", k, v.trim()))
            .collect();

        let signed_headers: String = headers
            .iter()
            .map(|(k, _)| *k)
            .collect::<Vec<_>>()
            .join(";");

        // ── 쿼리 스트링 정렬 (SigV4 필수) ────────────────────────────────
        let mut query_pairs: Vec<(String, String)> = url
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        query_pairs.sort_by_key(|(k, _)| k.clone());

        let canonical_query: String = query_pairs
            .iter()
            .map(|(k, v)| format!("{}={}", uri_encode(k), uri_encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        // ── Canonical Request ─────────────────────────────────────────────
        let canonical_request = format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            method,
            url.path(),
            canonical_query,
            canonical_headers,
            signed_headers,
            body_hash
        );

        // ── String To Sign ────────────────────────────────────────────────
        let credential_scope = format!(
            "{}/{}/{}/aws4_request",
            date, self.region, self.service
        );
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{}\n{}\n{}",
            datetime,
            credential_scope,
            sha256_hex(canonical_request.as_bytes())
        );

        // ── Signing Key ───────────────────────────────────────────────────
        let k_date    = hmac_bytes(format!("AWS4{}", self.secret_access_key).as_bytes(), date.as_bytes());
        let k_region  = hmac_bytes(&k_date,   self.region.as_bytes());
        let k_service = hmac_bytes(&k_region, self.service.as_bytes());
        let k_signing = hmac_bytes(&k_service, b"aws4_request");

        let signature = hmac_hex(&k_signing, string_to_sign.as_bytes());

        // ── Authorization Header ──────────────────────────────────────────
        let authorization = format!(
            "AWS4-HMAC-SHA256 Credential={}/{},SignedHeaders={},Signature={}",
            self.access_key_id, credential_scope, signed_headers, signature
        );

        vec![
            ("x-amz-date".into(),           datetime),
            ("x-amz-content-sha256".into(), body_hash),
            ("Authorization".into(),        authorization),
        ]
    }
}

// ── Crypto helpers ────────────────────────────────────────────────────────────

pub fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

fn hmac_bytes(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn hmac_hex(key: &[u8], data: &[u8]) -> String {
    hex::encode(hmac_bytes(key, data))
}

// SigV4 spec: URI encode everything except unreserved chars
fn uri_encode(s: &str) -> String {
    percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC)
        .to_string()
        // SigV4는 ~를 인코딩하지 않음
        .replace("%7E", "~")
}
