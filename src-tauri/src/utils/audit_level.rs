use std::sync::atomic::{AtomicBool, Ordering};

/// CDN API 감사 로그(audit-*.log)에 응답 본문까지 남길지 여부.
/// `lib.rs` 시작 시 `ProfileStore::get_app_settings()`로 초기화되고,
/// 설정 화면에서 토글할 때마다 `commands::s3::save_detailed_audit_log`가 갱신한다.
/// 어댑터(`adapters/cdn/mod.rs::log_cdn_http`)는 Tauri State 없이 이 값을 바로 읽는다.
static DETAILED: AtomicBool = AtomicBool::new(false);

pub fn enabled() -> bool {
    DETAILED.load(Ordering::Relaxed)
}

pub fn set(value: bool) {
    DETAILED.store(value, Ordering::Relaxed);
}
