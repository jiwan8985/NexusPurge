use anyhow::{Context, Result};
use tokio::fs;
use tokio::io::AsyncReadExt;

/// 파일 전체 MD5 계산 (S3 ETag 비교용)
/// S3 단일 업로드의 ETag == MD5 (Multipart 업로드 시 불일치 — parse_multipart_etag 참고)
pub async fn compute_file_md5(path: &str) -> Result<String> {
    let mut file = fs::File::open(path)
        .await
        .with_context(|| format!("파일 열기 실패: {}", path))?;

    // md5 v0.7은 스트리밍 Context API 사용
    let mut ctx = md5::Context::new();
    let mut buf = vec![0u8; 1024 * 1024];

    loop {
        let n = file.read(&mut buf).await.context("파일 읽기 실패")?;
        if n == 0 {
            break;
        }
        ctx.consume(&buf[..n]);
    }

    Ok(format!("{:x}", ctx.compute()))
}

/// 바이트 슬라이스 MD5
pub fn compute_bytes_md5(data: &[u8]) -> String {
    format!("{:x}", md5::compute(data))
}

/// S3 Multipart ETag 파싱 ("abc123-3" → (hash, part_count))
pub fn parse_multipart_etag(etag: &str) -> Option<(&str, u32)> {
    let cleaned = etag.trim_matches('"');
    let mut parts = cleaned.splitn(2, '-');
    let hash  = parts.next()?;
    let count = parts.next()?.parse().ok()?;
    Some((hash, count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_multipart_etag() {
        assert_eq!(parse_multipart_etag("\"abc123-3\""), Some(("abc123", 3)));
        assert_eq!(parse_multipart_etag("abc123"), None);
    }

    #[test]
    fn test_compute_bytes_md5() {
        assert_eq!(compute_bytes_md5(b"hello"), "5d41402abc4b2a76b9719d911017c592");
    }
}
