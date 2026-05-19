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

            #[cfg(target_os = "windows")]
            platform::init_notifications(&app.handle());

            if let Some(win) = app.get_webview_window("capture") {
                let _ = win.set_resizable(false);
            }

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
            commands::get_current_activity,
            commands::get_scheduler_status,
            commands::snooze_ping,
            commands::mark_task_done,
            commands::shorten_last_capture,
            commands::shorten_last_capture_15,
            commands::submit_gap_after_shorten,
            commands::extend_last_capture,
            commands::extend_last_capture_15,
            commands::repeat_last_capture,
            commands::update_ping_interval,
            commands::update_overdue_ping_interval,
            commands::list_recent_captures,
            commands::list_activity_digest,
            commands::list_top_quick_picks,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
