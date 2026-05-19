use super::aumid;
use super::win_toast::{self, PingToastContent, ToastButton, TOAST_LAUNCH_FOCUS};
use super::PingNotifier;
use crate::db::repo::{self, NextPingKind};
use crate::error::AppError;
use crate::window_util;
use crate::AppState;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

/// Toast button `arguments`; must match `ToastButton::action`.
const TOAST_ACTION_STILL: &str = "still";
const TOAST_ACTION_SNOOZE: &str = "snooze";
const TOAST_ACTION_MINUS_15: &str = "minus_15";
const TOAST_ACTION_DONE: &str = "done";
const TOAST_ACTION_PLUS_15: &str = "plus_15";

static AUMID_READY: OnceLock<()> = OnceLock::new();

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
    app.config().identifier.clone()
}

fn resolve_notification_icon(app: &AppHandle) -> PathBuf {
    if let Ok(dir) = app.path().resource_dir() {
        let ico = dir.join("icons/icon.ico");
        if ico.exists() {
            return ico;
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            for rel in ["icons/icon.ico", "../icons/icon.ico"] {
                let p = parent.join(rel);
                if p.exists() {
                    return p;
                }
            }
        }
    }
    PathBuf::from("icons/icon.ico")
}

fn ensure_aumid(app: &AppHandle) {
    AUMID_READY.get_or_init(|| {
        let app_id = toast_app_id(app);
        let icon = resolve_notification_icon(app);
        if let Err(e) = aumid::ensure_registered(&app_id, "AreYouFocused", &icon) {
            tracing::warn!("AUMID registry setup failed (toasts may stack or mis-activate): {e}");
        }
    });
}

fn focus_capture_on_main(app: &AppHandle) {
    let h = app.clone();
    if let Err(e) = app.run_on_main_thread(move || {
        window_util::show_and_focus_capture(&h);
    }) {
        tracing::warn!("toast activation: run_on_main_thread failed: {e}");
    }
}

fn dispatch_toast_action(app: &AppHandle, action: Option<&str>) {
    match action {
        Some(TOAST_ACTION_STILL) => {
            if let Err(e) = still_from_toast(app) {
                tracing::warn!("toast still: {e}");
            }
        }
        Some(TOAST_ACTION_SNOOZE) => {
            if let Err(e) = snooze_from_toast(app) {
                tracing::warn!("toast snooze: {e}");
            }
        }
        Some(TOAST_ACTION_MINUS_15) => {
            if let Err(e) = minus_15_from_toast(app) {
                tracing::warn!("toast minus_15: {e}");
            }
        }
        Some(TOAST_ACTION_DONE) => {
            if let Err(e) = done_from_toast(app) {
                tracing::warn!("toast done: {e}");
            }
        }
        Some(TOAST_ACTION_PLUS_15) => {
            if let Err(e) = plus_15_from_toast(app) {
                tracing::warn!("toast plus_15: {e}");
            }
        }
        Some(TOAST_LAUNCH_FOCUS) | None => {
            focus_capture_on_main(app);
        }
        Some(other) => {
            tracing::debug!("toast activation unknown action {other:?}, focusing window");
            focus_capture_on_main(app);
        }
    }
}

impl PingNotifier for WindowsNotifier {
    fn notify_ping_due(&self, app: &AppHandle) -> Result<(), AppError> {
        ensure_aumid(app);

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
                NextPingKind::Overdue => {
                    let subj = repo::get_planned_check_subject(conn)?
                        .or_else(|| repo::latest_capture_body(conn).ok().flatten())
                        .unwrap_or_default();
                    let preview = trim_for_toast_line(&subj, 90);
                    format!("Past your planned finish — still on this? “{preview}”")
                }
                NextPingKind::Standard => "What are you doing right now?".to_string(),
            };
            (latest_label, toast_secondary)
        };

        let app_id = toast_app_id(app);
        let app_for_activation = app.clone();

        let mut buttons = vec![
            ToastButton {
                label: "Snooze 10 min".to_string(),
                action: TOAST_ACTION_SNOOZE.to_string(),
            },
            ToastButton {
                label: "−15 min".to_string(),
                action: TOAST_ACTION_MINUS_15.to_string(),
            },
            ToastButton {
                label: "Done".to_string(),
                action: TOAST_ACTION_DONE.to_string(),
            },
            ToastButton {
                label: "+15 min".to_string(),
                action: TOAST_ACTION_PLUS_15.to_string(),
            },
        ];
        if let Some(ref label) = latest_label {
            buttons.push(ToastButton {
                label: label.clone(),
                action: TOAST_ACTION_STILL.to_string(),
            });
        }

        let content = PingToastContent {
            title: "AreYouFocused".to_string(),
            line2: toast_secondary,
            buttons,
        };

        win_toast::show_ping_toast(&app_id, &content, move |action| {
            let app = app_for_activation.clone();
            let app_main = app.clone();
            if let Err(e) = app.run_on_main_thread(move || {
                dispatch_toast_action(&app_main, action.as_deref());
            }) {
                tracing::warn!("toast activation: run_on_main_thread failed: {e}");
            }
            Ok(())
        })
        .map_err(|e| AppError::Notify(e.to_string()))
    }
}

/// Call once at startup so the first ping does not pay registry latency.
#[cfg(target_os = "windows")]
pub fn init_notifications(app: &AppHandle) {
    ensure_aumid(app);
}
