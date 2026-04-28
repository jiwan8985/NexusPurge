/// M-5: 재시도 가능한 HTTP status code 판단
/// 인증(401/403), 잘못된 요청(400/404), 501 Not Implemented 는 재시도 불가
pub fn is_retryable_status(code: u16) -> bool {
    matches!(code, 429 | 500 | 502 | 503 | 504)
}
