pub mod commands;
pub mod db;
pub mod domain;
pub mod error;
pub mod platform;
mod scheduler;
#[cfg(desktop)]
mod tray;
mod window_util;

use std::sync::{Arc, Mutex};
use tauri::{Manager, WindowEvent};

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
            app.manage(AppState {
                db: Mutex::new(conn),
            });

            let handle = app.handle().clone();
            let notifier = Arc::from(platform::current_notifier());
            scheduler::spawn_ping_loop(handle, notifier);

            #[cfg(desktop)]
            tray::setup_tray(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "capture" {
                return;
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::submit_capture,
            commands::get_scheduler_status,
            commands::update_ping_interval,
            commands::list_recent_captures,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
