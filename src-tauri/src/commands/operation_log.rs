use tauri::State;

use crate::services::operation_log::{OperationLog, OperationLogService};

#[tauri::command]
pub async fn save_operation_log(
    log: OperationLog,
    service: State<'_, OperationLogService>,
) -> Result<(), String> {
    service.save(log).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_operation_logs(
    service: State<'_, OperationLogService>,
) -> Result<Vec<OperationLog>, String> {
    service.list_recent().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_operation_log(
    id: String,
    service: State<'_, OperationLogService>,
) -> Result<Option<OperationLog>, String> {
    service.get(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_operation_logs(
    service: State<'_, OperationLogService>,
) -> Result<(), String> {
    service.clear().await.map_err(|e| e.to_string())
}
