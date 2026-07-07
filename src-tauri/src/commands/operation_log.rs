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

/// 날짜별 텍스트 로그가 저장되는 폴더를 OS 파일 탐색기로 연다.
#[tauri::command]
pub async fn open_operation_log_dir(
    service: State<'_, OperationLogService>,
) -> Result<String, String> {
    let dir = service.log_files_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("로그 폴더 생성 실패: {}", e))?;
    let path = dir.to_string_lossy().into_owned();

    #[cfg(target_os = "windows")]
    let cmd = std::process::Command::new("explorer").arg(&dir).spawn();
    #[cfg(target_os = "macos")]
    let cmd = std::process::Command::new("open").arg(&dir).spawn();
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let cmd = std::process::Command::new("xdg-open").arg(&dir).spawn();

    cmd.map_err(|e| format!("로그 폴더 열기 실패: {}", e))?;
    Ok(path)
}
