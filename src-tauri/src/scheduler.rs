//! Background loop: sleep until `next_ping_at`, show OS notification, schedule the next ping.

use crate::db::repo;
use crate::domain::ping_plan;
use crate::error::AppError;
use crate::platform::PingNotifier;
use crate::AppState;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

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

pub fn spawn_ping_loop(handle: AppHandle, notifier: Arc<dyn PingNotifier>) {
    tauri::async_runtime::spawn(async move {
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

            if sleep_secs > 0 {
                tokio::time::sleep(Duration::from_secs(sleep_secs)).await;
            }

            if let Err(e) = notifier.notify_ping_due(&handle) {
                tracing::warn!("scheduler: notify: {e}");
            }

            let resched: Result<(), AppError> = (|| {
                let state = handle.state::<AppState>();
                let mut db = state.db.lock().unwrap_or_else(|p| p.into_inner());
                let conn = &mut *db;
                let mut rng = rand::thread_rng();
                let now = unix_now();
                let (min_m, max_m) = repo::ping_min_max_minutes(conn).map_err(AppError::from)?;
                let nxt = ping_plan::next_ping_after(now, min_m, max_m, &mut rng);
                repo::set_next_ping_at_unix(conn, nxt).map_err(AppError::from)?;
                Ok(())
            })();
            if let Err(e) = resched {
                tracing::error!("scheduler: reschedule: {e}");
            }
        }
    });
}
