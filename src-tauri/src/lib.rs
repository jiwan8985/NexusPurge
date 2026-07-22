mod adapters;
mod commands;
mod services;
mod utils;

use commands::{auth, cdn, log_shipping, operation_log, s3, sync};
use services::auth::ExternalAuthAdapter;
use services::operation_log::OperationLogService;
use utils::adapter_cache::AdapterCache;
use utils::config::ProfileStore;
use utils::network_stats::NetworkStats;
use utils::transfer_control::TransferControl;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let data_dir = dirs::data_local_dir()
        .expect("data_local_dir lookup failed")
        .join("cdn-upload-tool");

    // tracing을 콘솔 + 파일(logs/audit.YYYY-MM-DD.log)에 동시 기록.
    // CDN 어댑터의 요청/응답 상세(HTTP 상태·소요시간·응답 본문)가 릴리즈 빌드에서도
    // 파일로 남아, CDN Purge 실패 원인을 사후에 추적할 수 있다 (LogPanel "로그 폴더"에서 열람).
    {
        use tracing_subscriber::layer::SubscriberExt;
        use tracing_subscriber::util::SubscriberInitExt;

        let audit_appender = tracing_appender::rolling::Builder::new()
            .rotation(tracing_appender::rolling::Rotation::DAILY)
            .filename_prefix("audit")
            .filename_suffix("log")
            // 고객사 환경에서 audit 로그가 무한정 쌓이지 않도록 최근 30일치만 보관
            .max_log_files(30)
            .build(data_dir.join("logs"))
            .expect("audit 로그 파일 초기화 실패");
        let (audit_writer, audit_guard) = tracing_appender::non_blocking(audit_appender);
        // non_blocking writer는 guard가 drop되면 기록이 멈추므로 앱 수명 동안 유지
        Box::leak(Box::new(audit_guard));

        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "cdn_upload_tool_lib=debug".into()),
            )
            .with(tracing_subscriber::fmt::layer())
            .with(
                tracing_subscriber::fmt::layer()
                    .with_ansi(false)
                    .with_writer(audit_writer),
            )
            .init();
    }

    let profile_store = ProfileStore::new().expect("ProfileStore 초기화 실패");
    let adapter_cache = AdapterCache::new();
    let operation_log_service = OperationLogService::new(data_dir);

    // 날짜별 텍스트 로그(system/transfer/cdn-*.log) 중 30일 지난 파일 정리 (앱 시작 시 1회)
    if let Err(err) = tauri::async_runtime::block_on(operation_log_service.cleanup_old_logs()) {
        tracing::warn!("오래된 로그 파일 정리 실패: {}", err);
    }

    // 감사 로그 상세 레벨을 저장된 설정값으로 초기화 (기본은 요약만)
    match tauri::async_runtime::block_on(profile_store.get_app_settings()) {
        Ok(settings) => utils::audit_level::set(settings.detailed_audit_log),
        Err(err) => tracing::warn!("앱 설정 로드 실패, 감사 로그 요약 모드로 시작: {}", err),
    }

    tauri::Builder::default()
        .manage(profile_store)
        .manage(adapter_cache)
        .manage(TransferControl::default())
        .manage(operation_log_service)
        .manage(ExternalAuthAdapter)
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            use tauri::Emitter;
            // 상태바 네트워크 위젯(업로드/다운로드 연결 수·평균 RTT)용 스냅샷을 2초 간격으로 push.
            // NetworkStats는 프로세스 전역 싱글턴이라 별도 Tauri State 없이 바로 조회 가능.
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
                loop {
                    interval.tick().await;
                    let snapshot = NetworkStats::global().snapshot();
                    let _ = app_handle.emit("network:stats", snapshot);
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Profile & Connection
            s3::load_profiles,
            s3::save_profile,
            s3::delete_profile,
            s3::connect_s3,
            s3::test_s3_connection, // H-3
            s3::cancel_transfer,
            // Settings (H-7)
            s3::save_last_profile_id,
            s3::get_last_profile_id,
            s3::get_app_settings,
            s3::save_detailed_audit_log,
            // Local FS (H-1)
            s3::get_home_dir,
            s3::list_local_dir,
            s3::create_local_dir,
            s3::delete_local_files,
            s3::rename_local_file,
            // S3 Operations
            s3::list_s3_objects,
            s3::list_s3_keys,
            s3::delete_s3_objects,
            s3::put_s3_object,
            s3::get_presigned_url,
            s3::get_s3_object_detail,
            s3::upload_files,
            s3::rename_s3_object, // H-1
            // 암호화 프로필
            s3::export_encrypted_profile,
            s3::import_encrypted_profile,
            s3::import_profile_file,
            // Sync & Transfer
            sync::build_sync_plan,
            sync::sync_preview,
            sync::start_uploads,
            sync::start_downloads,
            // CDN Purge
            cdn::purge_cloudfront,
            cdn::purge_cdn,
            cdn::test_cdn_connection,
            cdn::get_purge_status,
            cdn::inspect_url,
            // Operation Logs
            operation_log::save_operation_log,
            operation_log::list_operation_logs,
            operation_log::get_operation_log,
            operation_log::clear_operation_logs,
            operation_log::open_operation_log_dir,
            // External Auth Stub
            auth::external_auth_login,
            auth::external_auth_logout,
            auth::external_auth_refresh,
            auth::external_auth_current_session,
            // Customer S3 Log Shipping Stub
            log_shipping::ship_operation_log_to_customer_s3,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
