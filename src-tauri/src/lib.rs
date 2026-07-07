mod adapters;
mod commands;
mod services;
mod utils;

use commands::{auth, cdn, log_shipping, operation_log, s3, sync};
use services::auth::ExternalAuthAdapter;
use services::operation_log::OperationLogService;
use utils::adapter_cache::AdapterCache;
use utils::config::ProfileStore;
use utils::transfer_control::TransferControl;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cdn_upload_tool_lib=debug".into()),
        )
        .init();

    let profile_store = ProfileStore::new().expect("ProfileStore 초기화 실패");
    let adapter_cache = AdapterCache::new();
    let operation_log_service = OperationLogService::new(
        dirs::data_local_dir()
            .expect("data_local_dir lookup failed")
            .join("cdn-upload-tool"),
    );

    tauri::Builder::default()
        .manage(profile_store)
        .manage(adapter_cache)
        .manage(TransferControl::default())
        .manage(operation_log_service)
        .manage(ExternalAuthAdapter)
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
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
            // Local FS (H-1)
            s3::get_home_dir,
            s3::list_local_dir,
            s3::create_local_dir,
            s3::delete_local_files,
            s3::rename_local_file,
            // S3 Operations
            s3::list_s3_objects,
            s3::delete_s3_objects,
            s3::put_s3_object,
            s3::get_presigned_url,
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
            cdn::verify_cdn_urls,
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
