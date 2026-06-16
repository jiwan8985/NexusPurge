use tauri::State;

use crate::services::auth::{AuthAdapter, AuthSession, ExternalAuthAdapter};

#[tauri::command]
pub fn external_auth_login(adapter: State<'_, ExternalAuthAdapter>) -> Result<AuthSession, String> {
    adapter.login().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn external_auth_logout(adapter: State<'_, ExternalAuthAdapter>) -> Result<(), String> {
    adapter.logout().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn external_auth_refresh(
    session: AuthSession,
    adapter: State<'_, ExternalAuthAdapter>,
) -> Result<AuthSession, String> {
    adapter.refresh_token(session).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn external_auth_current_session(
    adapter: State<'_, ExternalAuthAdapter>,
) -> Result<Option<AuthSession>, String> {
    adapter.current_session().map_err(|e| e.to_string())
}
