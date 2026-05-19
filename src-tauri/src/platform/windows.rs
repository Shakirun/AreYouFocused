use super::PingNotifier;
use crate::db::repo::{self, NextPingKind};
use crate::error::AppError;
use crate::window_util;
use crate::AppState;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};
use tauri_winrt_notification::{Duration, Toast};

/// Toast button `arguments`; must match `add_button` second parameter.
const TOAST_ACTION_STILL: &str = "still";
const TOAST_ACTION_SNOOZE: &str = "snooze";
const TOAST_ACTION_MINUS_15: &str = "minus_15";
const TOAST_ACTION_DONE: &str = "done";
const TOAST_ACTION_PLUS_15: &str = "plus_15";

/// Windows toast action labels are short; keep total length modest.
fn still_button_label(body: &str) -> String {
    const PREFIX: &str = "Still: ";
    const MAX_CHARS: usize = 42;
    let t = body.trim();
    if t.is_empty() {
        return "Still".to_string();
    }
    let prefix_len = PREFIX.chars().count();
    let avail = MAX_CHARS.saturating_sub(prefix_len);
    if t.chars().count() <= avail {
        format!("{PREFIX}{t}")
    } else {
        let truncated: String = t.chars().take(avail.saturating_sub(1)).collect();
        format!("{PREFIX}{truncated}…")
    }
}

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

fn still_from_toast(app: &AppHandle) -> Result<(), AppError> {
    let state = app.state::<AppState>();
    let mut db = state.db.lock().unwrap_or_else(|p| p.into_inner());
    let conn = &mut *db;
    repo::persist_repeat_latest(conn, unix_now(), &mut rand::thread_rng())?;
    let _ = app.emit("scheduler-updated", ());
    Ok(())
}

fn minus_15_from_toast(app: &AppHandle) -> Result<(), AppError> {
    let state = app.state::<AppState>();
    let mut db = state.db.lock().unwrap_or_else(|p| p.into_inner());
    let conn = &mut *db;
    let info = repo::shorten_last_capture_duration(conn, unix_now(), repo::TOAST_ADJUST_MINUTES)?;
    let _ = app.emit("gap-fill-needed", &info);
    let _ = app.emit("scheduler-updated", ());
    window_util::show_and_focus_capture(app);
    Ok(())
}

fn plus_15_from_toast(app: &AppHandle) -> Result<(), AppError> {
    let state = app.state::<AppState>();
    let mut db = state.db.lock().unwrap_or_else(|p| p.into_inner());
    let conn = &mut *db;
    repo::extend_last_capture_and_delay_ping(conn, unix_now(), repo::TOAST_ADJUST_MINUTES)?;
    let _ = app.emit("scheduler-updated", ());
    Ok(())
}

fn done_from_toast(app: &AppHandle) -> Result<(), AppError> {
    let state = app.state::<AppState>();
    let mut db = state.db.lock().unwrap_or_else(|p| p.into_inner());
    let conn = &mut *db;
    repo::mark_latest_timed_capture_finished(conn)?;
    let _ = app.emit("scheduler-updated", ());
    window_util::show_and_focus_capture(app);
    Ok(())
}

fn trim_for_toast_line(s: &str, max_chars: usize) -> String {
    let t = s.trim();
    if t.is_empty() {
        return String::new();
    }
    let count = t.chars().count();
    if count <= max_chars {
        return t.to_string();
    }
    let taken: String = t.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{taken}…")
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
        let (latest_label, toast_secondary) = {
            let state = app.state::<AppState>();
            let mut db = state.db.lock().unwrap_or_else(|p| p.into_inner());
            let conn = &mut *db;
            let latest_label = repo::latest_capture_body(conn)?
                .filter(|s| !s.trim().is_empty())
                .map(|s| still_button_label(&s));
            let toast_secondary = match repo::get_next_ping_kind(conn)? {
                NextPingKind::PlannedCheck => {
                    let subj = repo::get_planned_check_subject(conn)?.unwrap_or_default();
                    let preview = trim_for_toast_line(&subj, 90);
                    format!("Planned time is up — still doing this? “{preview}”")
                }
                NextPingKind::Standard => "What are you doing right now?".to_string(),
            };
            (latest_label, toast_secondary)
        };

        let app_id = toast_app_id(app);
        let app_for_activation = app.clone();

        let mut toast_builder = Toast::new(&app_id)
            .title("AreYouFocused")
            .text2(&toast_secondary)
            .duration(Duration::Short);

        toast_builder = toast_builder.add_button("Snooze 10 min", TOAST_ACTION_SNOOZE);
        toast_builder =
            toast_builder.add_button("−15 min", TOAST_ACTION_MINUS_15);
        toast_builder = toast_builder.add_button("Done", TOAST_ACTION_DONE);
        toast_builder = toast_builder.add_button("+15 min", TOAST_ACTION_PLUS_15);
        if let Some(ref label) = latest_label {
            toast_builder = toast_builder.add_button(label, TOAST_ACTION_STILL);
        }

        toast_builder
            .on_activated(move |action| {
                let app = app_for_activation.clone();
                match action.as_deref() {
                    Some(TOAST_ACTION_STILL) => {
                        let app_run = app.clone();
                        if let Err(e) = app.run_on_main_thread(move || {
                            if let Err(e) = still_from_toast(&app_run) {
                                tracing::warn!("toast still: {e}");
                            }
                        }) {
                            tracing::warn!("toast activation: run_on_main_thread failed: {e}");
                        }
                    }
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
                    Some(TOAST_ACTION_MINUS_15) => {
                        let app_run = app.clone();
                        if let Err(e) = app.run_on_main_thread(move || {
                            if let Err(e) = minus_15_from_toast(&app_run) {
                                tracing::warn!("toast minus_15: {e}");
                            }
                        }) {
                            tracing::warn!("toast activation: run_on_main_thread failed: {e}");
                        }
                    }
                    Some(TOAST_ACTION_DONE) => {
                        let app_run = app.clone();
                        if let Err(e) = app.run_on_main_thread(move || {
                            if let Err(e) = done_from_toast(&app_run) {
                                tracing::warn!("toast done: {e}");
                            }
                        }) {
                            tracing::warn!("toast activation: run_on_main_thread failed: {e}");
                        }
                    }
                    Some(TOAST_ACTION_PLUS_15) => {
                        let app_run = app.clone();
                        if let Err(e) = app.run_on_main_thread(move || {
                            if let Err(e) = plus_15_from_toast(&app_run) {
                                tracing::warn!("toast plus_15: {e}");
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
