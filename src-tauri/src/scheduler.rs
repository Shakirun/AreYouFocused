//! Background loop: sleep until `next_ping_at`, show OS notification, schedule the next ping.
//!
//! **QA:** set `AREYOUFOCUSED_DEV_PING_SECS` to a positive integer to sleep that many seconds
//! between pings instead of the normal DB-driven schedule. Not for production.

use crate::db::repo;
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

/// Re-read `next_ping_at` frequently so snooze / capture updates can shorten an in-flight sleep.
const SLEEP_POLL_SECS: u64 = 5;

async fn sleep_until_next_ping_due(handle: &AppHandle) {
    loop {
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

        if sleep_secs == 0 {
            break;
        }

        let chunk = sleep_secs.min(SLEEP_POLL_SECS);
        tokio::time::sleep(Duration::from_secs(chunk)).await;
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
                sleep_until_next_ping_due(&handle).await;
            }

            if let Err(e) = notifier.notify_ping_due(&handle) {
                tracing::warn!("scheduler: notify: {e}");
            }
            emit_ping_due(&handle);

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
                    match repo::get_next_ping_kind(conn)? {
                        repo::NextPingKind::PlannedCheck => {
                            repo::apply_after_planned_check_ping(conn)?;
                        }
                        repo::NextPingKind::Standard => {
                            repo::schedule_random_next_ping(conn, now, &mut rng)?;
                        }
                    }
                    Ok(())
                })();
                if let Err(e) = resched {
                    tracing::error!("scheduler: reschedule: {e}");
                }
            }
        }
    });
}
