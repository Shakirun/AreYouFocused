pub mod commands;
pub mod db;
pub mod domain;
pub mod error;
pub mod platform;

use std::sync::Mutex;
use tauri::Manager;

pub struct AppState {
    pub db: Mutex<rusqlite::Connection>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("resolve app data dir: {e}"))?;
            std::fs::create_dir_all(&dir).map_err(|e| format!("create app data dir: {e}"))?;
            let db_path = dir.join("areyoufocused.db");
            let conn = db::open_database(&db_path).map_err(|e| format!("open database: {e}"))?;
            app.manage(AppState { db: Mutex::new(conn) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::submit_capture])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
