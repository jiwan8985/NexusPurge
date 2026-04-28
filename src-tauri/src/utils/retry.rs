/// M-5: 재시도 가능한 HTTP status code 판단
/// 인증(401/403), 잘못된 요청(400/404), 501 Not Implemented 는 재시도 불가
pub fn is_retryable_status(code: u16) -> bool {
    matches!(code, 429 | 500 | 502 | 503 | 504)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retryable_codes_are_recognised() {
        for code in [429, 500, 502, 503, 504] {
            assert!(is_retryable_status(code), "expected {code} to be retryable");
        }
    }

    #[test]
    fn non_retryable_codes_are_rejected() {
        for code in [200, 201, 204, 301, 400, 401, 403, 404, 409, 410, 422, 501] {
            assert!(!is_retryable_status(code), "expected {code} NOT to be retryable");
        }
    }
}
