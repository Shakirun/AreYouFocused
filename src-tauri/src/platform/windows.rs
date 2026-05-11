use super::PingNotifier;
use crate::db::repo;
use crate::error::AppError;
use crate::window_util;
use crate::AppState;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};
use tauri_winrt_notification::{Duration, Toast};

/// Toast button `arguments`; must match `add_button` second parameter.
const TOAST_ACTION_SNOOZE: &str = "snooze";

pub struct WindowsNotifier;

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn snooze_from_toast(app: &AppHandle) -> Result<(), AppError> {
    let state = app.state::<AppState>();
    let mut db = state.db.lock().unwrap_or_else(|p| p.into_inner());
    let conn = &mut *db;
    repo::snooze_next_ping(conn, unix_now())?;
    let _ = app.emit("scheduler-updated", ());
    Ok(())
}

fn toast_app_id(app: &AppHandle) -> String {
    let identifier = app.config().identifier.clone();
    let Ok(exe) = std::env::current_exe() else {
        return identifier;
    };
    let Some(dir) = exe.parent() else {
        return identifier;
    };
    let path = dir.to_string_lossy();
    if path.ends_with(r"target\debug")
        || path.ends_with("target/debug")
        || path.ends_with(r"target\release")
        || path.ends_with("target/release")
    {
        Toast::POWERSHELL_APP_ID.to_string()
    } else {
        identifier
    }
}

impl PingNotifier for WindowsNotifier {
    fn notify_ping_due(&self, app: &AppHandle) -> Result<(), AppError> {
        let app_id = toast_app_id(app);
        let app_for_activation = app.clone();
        Toast::new(&app_id)
            .title("AreYouFocused")
            .text2("What are you doing right now?")
            .duration(Duration::Short)
            .add_button("Snooze 10 min", TOAST_ACTION_SNOOZE)
            .on_activated(move |action| {
                let app = app_for_activation.clone();
                match action.as_deref() {
                    Some(TOAST_ACTION_SNOOZE) => {
                        let app_run = app.clone();
                        if let Err(e) = app.run_on_main_thread(move || {
                            if let Err(e) = snooze_from_toast(&app_run) {
                                tracing::warn!("toast snooze: {e}");
                            }
                        }) {
                            tracing::warn!("toast activation: run_on_main_thread failed: {e}");
                        }
                    }
                    _ => {
                        let h2 = app.clone();
                        if let Err(e) = app.run_on_main_thread(move || {
                            window_util::show_and_focus_capture(&h2);
                        }) {
                            tracing::warn!("toast activation: run_on_main_thread failed: {e}");
                        }
                    }
                }
                Ok(())
            })
            .show()
            .map_err(|e| AppError::Notify(e.to_string()))
    }
}
