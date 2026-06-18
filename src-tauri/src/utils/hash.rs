use anyhow::{Context, Result};
use std::path::Path;
use tokio::fs;
use tokio::io::AsyncReadExt;

/// 파일 전체 MD5 계산 — Path 타입 (S3 단일 업로드 ETag 비교용)
#[allow(dead_code)]
pub async fn calculate_md5(path: &Path) -> Result<String> {
    let path_str = path.to_str().context("유효하지 않은 경로")?;
    compute_file_md5(path_str).await
}

/// 파일 전체 MD5 계산 — &str 경로 (하위 호환)
pub async fn compute_file_md5(path: &str) -> Result<String> {
    let mut file = fs::File::open(path)
        .await
        .with_context(|| format!("파일 열기 실패: {}", path))?;

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

/// S3 Multipart ETag 계산.
/// S3 멀티파트 업로드의 ETag = MD5(각 파트 MD5 바이너리 이어붙인 값) + "-{파트수}".
/// `part_size`는 업로드 시 사용한 파트 크기와 동일해야 정확하게 비교된다.
pub async fn calculate_multipart_etag(path: &Path, part_size: usize) -> Result<String> {
    let mut file = fs::File::open(path)
        .await
        .with_context(|| format!("파일 열기 실패: {}", path.display()))?;

    let mut part_md5s: Vec<u8> = Vec::new();
    let mut num_parts = 0u32;
    let mut buf = vec![0u8; part_size];

    loop {
        let mut filled = 0;
        while filled < buf.len() {
            let n = file
                .read(&mut buf[filled..])
                .await
                .context("파일 읽기 실패")?;
            if n == 0 {
                break;
            }
            filled += n;
        }
        if filled == 0 {
            break;
        }
        let digest = md5::compute(&buf[..filled]);
        part_md5s.extend_from_slice(digest.as_ref());
        num_parts += 1;
    }

    if num_parts == 0 {
        return Ok(format!("{:x}", md5::compute(b"")));
    }

    let final_hash = format!("{:x}", md5::compute(&part_md5s));
    Ok(format!("{}-{}", final_hash, num_parts))
}

#[allow(dead_code)]
pub fn compute_bytes_md5(data: &[u8]) -> String {
    format!("{:x}", md5::compute(data))
}

/// S3 Multipart ETag 파싱 ("abc123-3" → (hash, part_count))
#[allow(dead_code)]
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
