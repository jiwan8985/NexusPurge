// Tauri 2: main.rs는 최소화, 실제 setup은 lib.rs에서 처리
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    cdn_upload_tool_lib::run();
}
