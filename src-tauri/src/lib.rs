pub mod config;

#[cfg(feature = "desktop")]
use config::AppSettings;

#[cfg(feature = "desktop")]
#[tauri::command]
fn get_default_settings() -> AppSettings {
    AppSettings::default()
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn health_check() -> &'static str {
    "ok"
}

#[cfg(feature = "desktop")]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![get_default_settings, health_check])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(not(feature = "desktop"))]
pub fn run() {}
