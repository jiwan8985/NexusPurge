use tauri::State;

use crate::services::log_shipping::{ship_log_with_profile, LogShippingResult, LogShippingTarget};
use crate::services::operation_log::OperationLog;
use crate::utils::config::ProfileStore;

#[tauri::command]
pub async fn ship_operation_log_to_customer_s3(
    log: OperationLog,
    profile_id: String,
    target: LogShippingTarget,
    store: State<'_, ProfileStore>,
) -> Result<LogShippingResult, String> {
    ship_log_with_profile(&log, &profile_id, &target, &store)
        .await
        .map_err(|e| e.to_string())
}
