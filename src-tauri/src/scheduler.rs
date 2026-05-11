//! Background loop: sleep until `next_ping_at`, show OS notification, schedule the next ping.
//!
//! **QA:** set `AREYOUFOCUSED_DEV_PING_SECS` to a positive integer to sleep that many seconds
//! between pings instead of the normal DB-driven schedule. Not for production.

use crate::db::repo;
use crate::domain::ping_plan;
use crate::error::AppError;
use crate::platform::PingNotifier;
use crate::AppState;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn sleep_secs_until(next_unix: i64, now_unix: i64) -> u64 {
    if next_unix <= now_unix {
        0
    } else {
        (next_unix - now_unix) as u64
    }
}

/// Fixed interval in seconds for local QA (see module docs).
fn dev_ping_secs() -> Option<u64> {
    std::env::var("AREYOUFOCUSED_DEV_PING_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|&s| s > 0)
}

fn emit_ping_due(handle: &AppHandle) {
    if let Err(e) = handle.emit("ping-due", ()) {
        tracing::debug!("scheduler: emit ping-due: {e}");
    }
}

/// Bring the capture window forward so the user can answer after a ping.
fn focus_capture_window(handle: &AppHandle) {
    let Some(win) = handle.get_webview_window("capture") else {
        tracing::debug!("scheduler: no window labeled capture");
        return;
    };
    let _ = win.unminimize();
    let _ = win.show();
    let _ = win.set_focus();
}

pub fn spawn_ping_loop(handle: AppHandle, notifier: Arc<dyn PingNotifier>) {
    let dev_secs = dev_ping_secs();
    if let Some(s) = dev_secs {
        tracing::warn!(
            "AREYOUFOCUSED_DEV_PING_SECS={s}: fixed-second ping interval for QA (do not ship to production)"
        );
    }

    tauri::async_runtime::spawn(async move {
        loop {
            if let Some(dev_s) = dev_secs {
                tokio::time::sleep(Duration::from_secs(dev_s)).await;
            } else {
                let sleep_secs = {
                    let state = handle.state::<AppState>();
                    let mut db = state.db.lock().unwrap_or_else(|p| p.into_inner());
                    let conn = &mut *db;
                    let mut rng = rand::thread_rng();
                    let now = unix_now();
                    match repo::ensure_next_ping_scheduled(conn, now, &mut rng) {
                        Ok(next) => sleep_secs_until(next, unix_now()),
                        Err(e) => {
                            tracing::error!("scheduler: ensure next ping: {e}");
                            60u64
                        }
                    }
                };

                if sleep_secs > 0 {
                    tokio::time::sleep(Duration::from_secs(sleep_secs)).await;
                }
            }

            if let Err(e) = notifier.notify_ping_due(&handle) {
                tracing::warn!("scheduler: notify: {e}");
            }
            emit_ping_due(&handle);
            focus_capture_window(&handle);

            if let Some(dev_s) = dev_secs {
                let resched: Result<(), AppError> = (|| {
                    let state = handle.state::<AppState>();
                    let mut db = state.db.lock().unwrap_or_else(|p| p.into_inner());
                    let conn = &mut *db;
                    let nxt = unix_now() + dev_s as i64;
                    repo::set_next_ping_at_unix(conn, nxt)?;
                    Ok(())
                })();
                if let Err(e) = resched {
                    tracing::error!("scheduler: reschedule (dev): {e}");
                }
            } else {
                let resched: Result<(), AppError> = (|| {
                    let state = handle.state::<AppState>();
                    let mut db = state.db.lock().unwrap_or_else(|p| p.into_inner());
                    let conn = &mut *db;
                    let mut rng = rand::thread_rng();
                    let now = unix_now();
                    let (min_m, max_m) =
                        repo::ping_min_max_minutes(conn).map_err(AppError::from)?;
                    let nxt = ping_plan::next_ping_after(now, min_m, max_m, &mut rng);
                    repo::set_next_ping_at_unix(conn, nxt).map_err(AppError::from)?;
                    Ok(())
                })();
                if let Err(e) = resched {
                    tracing::error!("scheduler: reschedule: {e}");
                }
            }
        }
    });
}
