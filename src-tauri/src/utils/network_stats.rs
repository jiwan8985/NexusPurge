use serde::Serialize;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

/// 최근 왕복 응답시간(RTT) 샘플을 얼마나 보관할지 — 오래된 표본이 평균을 지배하지 않도록 고정 크기 윈도우
const WINDOW: usize = 20;

/// S3/CDN "빠른 왕복 호출"(HeadObject, ListObjectsV2, Purge 등)의 평균 응답시간과
/// 현재 진행 중인 S3 요청 수를 추적하는 프로세스 전역 카운터.
///
/// 대용량 업로드/다운로드(`upload_with_options`/`download_with_cancel`)는 파일 크기에 따라
/// 초~분 단위로 걸리므로 여기서 다루는 "RTT"에는 포함하지 않는다 — 포함하면 지연시간 지표로서의
/// 의미가 사라진다.
pub struct NetworkStats {
    samples: Mutex<VecDeque<u64>>,
    active_s3_calls: AtomicUsize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStatsSnapshot {
    pub avg_rtt_ms: Option<u64>,
    pub active_s3_calls: usize,
}

impl NetworkStats {
    fn new() -> Self {
        Self {
            samples: Mutex::new(VecDeque::with_capacity(WINDOW)),
            active_s3_calls: AtomicUsize::new(0),
        }
    }

    /// 프로세스 전역 싱글턴 — `S3Adapter`가 커맨드 계층을 거치지 않고도(Tauri State 없이도)
    /// 어댑터 메서드 내부에서 바로 기록할 수 있도록 한다.
    pub fn global() -> &'static Arc<NetworkStats> {
        static INSTANCE: OnceLock<Arc<NetworkStats>> = OnceLock::new();
        INSTANCE.get_or_init(|| Arc::new(NetworkStats::new()))
    }

    /// S3 호출 시작 시 호출 — 반환된 가드가 drop될 때 자동으로 활성 카운트 감소 + RTT 기록.
    pub fn begin_s3_call(self: &Arc<Self>) -> RttGuard {
        self.active_s3_calls.fetch_add(1, Ordering::Relaxed);
        RttGuard {
            stats: self.clone(),
            start: Instant::now(),
        }
    }

    /// CDN 호출 1건의 응답시간(ms)을 기록 — `adapters/cdn/mod.rs::log_cdn_http()`에서 호출.
    pub fn record_cdn(&self, elapsed_ms: u64) {
        self.push_sample(elapsed_ms);
    }

    fn push_sample(&self, elapsed_ms: u64) {
        let mut samples = self.samples.lock().unwrap_or_else(|e| e.into_inner());
        if samples.len() >= WINDOW {
            samples.pop_front();
        }
        samples.push_back(elapsed_ms);
    }

    pub fn snapshot(&self) -> NetworkStatsSnapshot {
        let samples = self.samples.lock().unwrap_or_else(|e| e.into_inner());
        let avg_rtt_ms = if samples.is_empty() {
            None
        } else {
            Some(samples.iter().sum::<u64>() / samples.len() as u64)
        };
        NetworkStatsSnapshot {
            avg_rtt_ms,
            active_s3_calls: self.active_s3_calls.load(Ordering::Relaxed),
        }
    }
}

/// `NetworkStats::begin_s3_call()`가 반환하는 RAII 가드.
/// drop 시 활성 호출 수를 줄이고 경과 시간을 표본에 추가한다.
pub struct RttGuard {
    stats: Arc<NetworkStats>,
    start: Instant,
}

impl Drop for RttGuard {
    fn drop(&mut self) {
        self.stats.active_s3_calls.fetch_sub(1, Ordering::Relaxed);
        let elapsed_ms = self.start.elapsed().as_millis() as u64;
        self.stats.push_sample(elapsed_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_is_empty_before_any_sample() {
        let stats = Arc::new(NetworkStats::new());
        let snap = stats.snapshot();
        assert_eq!(snap.avg_rtt_ms, None);
        assert_eq!(snap.active_s3_calls, 0);
    }

    #[test]
    fn begin_s3_call_tracks_active_count_and_records_rtt_on_drop() {
        let stats = Arc::new(NetworkStats::new());
        let guard = stats.begin_s3_call();
        assert_eq!(stats.snapshot().active_s3_calls, 1);
        drop(guard);
        let snap = stats.snapshot();
        assert_eq!(snap.active_s3_calls, 0);
        assert!(snap.avg_rtt_ms.is_some());
    }

    #[test]
    fn record_cdn_feeds_into_average() {
        let stats = Arc::new(NetworkStats::new());
        stats.record_cdn(100);
        stats.record_cdn(200);
        assert_eq!(stats.snapshot().avg_rtt_ms, Some(150));
    }

    #[test]
    fn window_caps_at_fixed_size_and_drops_oldest() {
        let stats = Arc::new(NetworkStats::new());
        for i in 0..(WINDOW + 5) {
            stats.record_cdn(i as u64);
        }
        // 가장 오래된 5개(0..5)는 밀려나고 5..(WINDOW+5)만 남아야 함
        let expected_sum: u64 = (5..(WINDOW as u64 + 5)).sum();
        let expected_avg = expected_sum / WINDOW as u64;
        assert_eq!(stats.snapshot().avg_rtt_ms, Some(expected_avg));
    }
}
