//! Background loop for daily wellness reminders (separate from activity pings).

use crate::db::daily_reminder::{self, DueDailyReminder};
use crate::db::repo;
use crate::platform::DailyReminderNotifier;
use crate::AppState;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

const SLEEP_POLL_SECS: u64 = 5;

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

async fn sleep_until_daily_due(handle: &AppHandle) {
    loop {
        let sleep_secs = {
            let state = handle.state::<AppState>();
            let db = state.db.lock().unwrap_or_else(|p| p.into_inner());
            let conn = &*db;
            match daily_reminder::next_daily_fire_unix(conn, unix_now()) {
                Ok(Some(next)) => {
                    let activity_next = repo::get_next_ping_at_unix(conn).ok().flatten();
                    let adjusted =
                        daily_reminder::defer_unix_if_activity_conflict(conn, next, activity_next);
                    sleep_secs_until(adjusted, unix_now())
                }
                Ok(None) => 300,
                Err(e) => {
                    tracing::error!("daily_scheduler: next fire: {e}");
                    60
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

fn handle_daily_fire(handle: &AppHandle, due: DueDailyReminder, notifier: &dyn DailyReminderNotifier) {
    let label = daily_reminder::display_label(&due.row);
    let reminder_id = due.row.id;

    if let Err(e) = notifier.notify_daily_reminder(handle, &label, reminder_id) {
        tracing::warn!("daily_scheduler: notify: {e}");
    }
    let _ = handle.emit("daily-reminder-fired", ());

    let state = handle.state::<AppState>();
    let mut db = state.db.lock().unwrap_or_else(|p| p.into_inner());
    let conn = &mut *db;
    if due.is_burst {
        if let Err(e) = daily_reminder::advance_burst_after_fire(conn, reminder_id, unix_now()) {
            tracing::error!("daily_scheduler: advance burst: {e}");
        }
    } else if let Err(e) = daily_reminder::start_burst_after_fire(conn, reminder_id, unix_now()) {
        tracing::error!("daily_scheduler: start burst: {e}");
    }
}

pub fn spawn_daily_reminder_loop(handle: AppHandle, notifier: Arc<dyn DailyReminderNotifier>) {
    tauri::async_runtime::spawn(async move {
        loop {
            sleep_until_daily_due(&handle).await;

            let due = {
                let state = handle.state::<AppState>();
                let db = state.db.lock().unwrap_or_else(|p| p.into_inner());
                let conn = &*db;
                match daily_reminder::due_daily_reminder(conn, unix_now()) {
                    Ok(d) => d,
                    Err(e) => {
                        tracing::error!("daily_scheduler: due check: {e}");
                        None
                    }
                }
            };

            if let Some(d) = due {
                handle_daily_fire(&handle, d, notifier.as_ref());
            }

            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
}
