use tauri::State;

use crate::services::log_shipping::{LogShippingResult, LogShippingService, LogShippingTarget};
use crate::services::operation_log::OperationLog;

#[tauri::command]
pub async fn ship_operation_log_to_customer_s3(
    log: OperationLog,
    target: Option<LogShippingTarget>,
    service: State<'_, LogShippingService>,
) -> Result<LogShippingResult, String> {
    service
        .ship_json_log(log, target)
        .await
        .map_err(|e| e.to_string())
}
