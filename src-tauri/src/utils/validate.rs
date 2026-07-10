/// S3 안전 문자 화이트리스트: 영문 대소문자, 숫자, `! - _ . * ' ( )`
/// (경로 구분자 `/`는 validate_s3_key에서 별도 처리)
fn is_s3_safe_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '!' | '-' | '_' | '.' | '*' | '\'' | '(' | ')')
}

/// 슬래시를 포함하지 않는 단일 경로 세그먼트(폴더명 또는 파일명)를 검증한다.
pub fn validate_s3_key_segment(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("이름은 비워둘 수 없습니다".to_string());
    }
    if let Some(bad_char) = name.chars().find(|c| !is_s3_safe_char(*c)) {
        return Err(format!(
            "'{}'에 허용되지 않는 문자 '{}'가 포함되어 있습니다. \
             영문, 숫자, ! - _ . * ' ( ) 문자만 사용할 수 있습니다.",
            name, bad_char
        ));
    }
    Ok(())
}

/// `/`로 구분된 전체 S3 키의 각 세그먼트를 검증한다. 빈 세그먼트(연속 슬래시, 폴더 표시용
/// trailing slash)는 건너뛴다.
pub fn validate_s3_key(key: &str) -> Result<(), String> {
    for segment in key.split('/') {
        if segment.is_empty() {
            continue;
        }
        validate_s3_key_segment(segment)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_ascii_letters_digits_and_s3_safe_symbols() {
        assert!(validate_s3_key_segment("my-file_v1.2(final)!'*.txt").is_ok());
    }

    #[test]
    fn rejects_korean_characters() {
        assert!(validate_s3_key_segment("한글파일.txt").is_err());
    }

    #[test]
    fn rejects_space() {
        assert!(validate_s3_key_segment("my file.txt").is_err());
    }

    #[test]
    fn rejects_windows_reserved_characters() {
        // `*`는 S3 안전 문자 화이트리스트에 포함되므로 제외
        for ch in ['\\', ':', '?', '"', '<', '>', '|'] {
            let name = format!("bad{}name", ch);
            assert!(validate_s3_key_segment(&name).is_err(), "expected rejection for {:?}", ch);
        }
    }

    #[test]
    fn rejects_empty_names() {
        assert!(validate_s3_key_segment("").is_err());
        assert!(validate_s3_key_segment("   ").is_err());
    }

    #[test]
    fn validate_s3_key_checks_every_segment() {
        assert!(validate_s3_key("folder/sub-folder/file-1.txt").is_ok());
        assert!(validate_s3_key("folder/sub/").is_ok()); // trailing slash 폴더 키
        assert!(validate_s3_key("폴더/file.txt").is_err());
        assert!(validate_s3_key("folder/파일.txt").is_err());
    }
}
