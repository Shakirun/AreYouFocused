//! Background loop: sleep until `next_ping_at`, show OS notification, schedule the next ping.
//!
//! **QA:** set `AREYOUFOCUSED_DEV_PING_SECS` to a positive integer to sleep that many seconds
//! between pings instead of the normal DB-driven schedule. Not for production.

use crate::db::{repo, sleep_hours};
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

/// Whether a ping at `at_unix` would be suppressed by the configured sleeping hours.
fn falls_in_sleep_hours(conn: &rusqlite::Connection, at_unix: i64) -> bool {
    match sleep_hours::read_sleep_hours_settings(conn) {
        Ok(s) if s.enabled => {
            sleep_hours::is_in_sleep_window_at_unix(at_unix, &s.start_hm, &s.end_hm)
                .unwrap_or(false)
        }
        _ => false,
    }
}

async fn sleep_until_next_ping_due(handle: &AppHandle, notifier: &dyn PingNotifier) {
    loop {
        let (sleep_secs, os_ping_at) = {
            let state = handle.state::<AppState>();
            let mut db = state.db.lock().unwrap_or_else(|p| p.into_inner());
            let conn = &mut *db;
            let mut rng = rand::thread_rng();
            let now = unix_now();
            match repo::ensure_next_ping_scheduled(conn, now, &mut rng) {
                Ok(next) => {
                    let os_ping_at = (!falls_in_sleep_hours(conn, next)).then_some(next);
                    (sleep_secs_until(next, unix_now()), os_ping_at)
                }
                Err(e) => {
                    tracing::error!("scheduler: ensure next ping: {e}");
                    (60u64, None)
                }
            }
        };

        if sleep_secs == 0 {
            break;
        }

        // Outside the DB lock: this may call into the platform notification service.
        notifier.sync_scheduled_ping(handle, os_ping_at);

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
                sleep_until_next_ping_due(&handle, notifier.as_ref()).await;
            }

            let in_sleep = {
                let state = handle.state::<AppState>();
                let db = state.db.lock().unwrap_or_else(|p| p.into_inner());
                sleep_hours::is_now_in_sleep(&db).unwrap_or(false)
            };
            if in_sleep {
                tracing::debug!("scheduler: skip ping during sleeping hours");
            } else {
                if let Err(e) = notifier.notify_ping_due(&handle) {
                    tracing::warn!("scheduler: notify: {e}");
                }
                emit_ping_due(&handle);
            }

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
                        repo::NextPingKind::Overdue => {
                            repo::schedule_overdue_next_ping(conn, now, &mut rng)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_memory;
    use chrono::{Local, NaiveTime, TimeZone};

    fn local_unix(h: u32, m: u32) -> i64 {
        let date = Local::now().date_naive();
        let t = NaiveTime::from_hms_opt(h, m, 0).unwrap();
        Local
            .from_local_datetime(&date.and_time(t))
            .single()
            .unwrap()
            .timestamp()
    }

    #[test]
    fn os_ping_mirror_skips_sleeping_hours_only_when_enabled() {
        let conn = open_memory().unwrap();
        sleep_hours::save_sleep_hours_settings(&conn, true, "22:00", "08:00").unwrap();
        assert!(falls_in_sleep_hours(&conn, local_unix(23, 30)));
        assert!(!falls_in_sleep_hours(&conn, local_unix(12, 0)));

        sleep_hours::save_sleep_hours_settings(&conn, false, "22:00", "08:00").unwrap();
        assert!(!falls_in_sleep_hours(&conn, local_unix(23, 30)));
    }

    #[test]
    fn sleep_secs_until_clamps_past_to_zero() {
        assert_eq!(sleep_secs_until(100, 200), 0);
        assert_eq!(sleep_secs_until(200, 100), 100);
    }
}
