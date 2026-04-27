mod commands;
mod adapters;
mod utils;

use commands::{s3, sync, cdn};
use utils::config::ProfileStore;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cdn_upload_tool_lib=debug".into()),
        )
        .init();

    let profile_store = ProfileStore::new().expect("ProfileStore 초기화 실패");

    tauri::Builder::default()
        .manage(profile_store)
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            // Profile & Connection
            s3::load_profiles,
            s3::save_profile,
            s3::delete_profile,
            s3::connect_s3,
            // Local FS
            s3::list_local_dir,
            // S3 Operations
            s3::list_s3_objects,
            s3::delete_s3_objects,
            s3::put_s3_object,
            s3::get_presigned_url,
            s3::upload_files,
            // Sync & Transfer
            sync::build_sync_plan,
            sync::sync_preview,
            sync::start_uploads,
            sync::start_downloads,
            // CDN Purge
            cdn::purge_cloudfront,
            cdn::purge_cdn,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
