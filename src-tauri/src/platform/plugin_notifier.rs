//! Notifications through `tauri-plugin-notification` (Android, iOS, Linux, macOS).
//!
//! Windows keeps its own toast implementation with action buttons; this backend
//! only shows plain notifications — tapping one opens the app.
//!
//! On mobile the process may be frozen or killed in the background, so the next
//! ping is additionally handed to the OS as a scheduled notification (AlarmManager /
//! UNUserNotificationCenter). Both paths share one notification id, so whichever
//! fires first is simply replaced by the other instead of stacking up.

use super::{DailyReminderNotifier, PingNotifier};
use crate::db::repo::{self, NextPingKind};
use crate::error::AppError;
use crate::AppState;
#[cfg(mobile)]
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;
#[cfg(mobile)]
use tauri_plugin_notification::Schedule;

/// Android collapses long notification bodies; keep the preview short.
const PREVIEW_MAX_CHARS: usize = 90;

/// Shared by the in-process ping and its OS-scheduled mirror (see module docs).
const PING_NOTIFICATION_ID: i32 = 1001;

const PING_TITLE: &str = "AreYouFocused";

#[cfg(mobile)]
#[derive(Default)]
struct ScheduledPing {
    /// False until the first successful sync so a stale alarm left by a previous
    /// process run is always cancelled or replaced.
    synced: bool,
    at: Option<i64>,
}

pub struct PluginNotifier {
    #[cfg(mobile)]
    scheduled: Mutex<ScheduledPing>,
}

impl PluginNotifier {
    pub fn new() -> Self {
        Self {
            #[cfg(mobile)]
            scheduled: Mutex::new(ScheduledPing::default()),
        }
    }
}

fn trim_preview(s: &str, max_chars: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max_chars {
        return t.to_string();
    }
    let taken: String = t.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{taken}…")
}

fn ping_body(app: &AppHandle) -> Result<String, AppError> {
    let state = app.state::<AppState>();
    let db = state.db.lock().unwrap_or_else(|p| p.into_inner());
    let conn = &*db;
    let body = match repo::get_next_ping_kind(conn)? {
        NextPingKind::PlannedCheck => {
            let subj = repo::get_planned_check_subject(conn)?.unwrap_or_default();
            format!(
                "Planned time is up — still doing this? “{}”",
                trim_preview(&subj, PREVIEW_MAX_CHARS)
            )
        }
        NextPingKind::Overdue => {
            let subj = repo::get_planned_check_subject(conn)?
                .or_else(|| repo::latest_capture_body(conn).ok().flatten())
                .unwrap_or_default();
            format!(
                "Past your planned finish — still on this? “{}”",
                trim_preview(&subj, PREVIEW_MAX_CHARS)
            )
        }
        NextPingKind::Standard => "What are you doing right now?".to_string(),
    };
    Ok(body)
}

#[cfg(mobile)]
fn cancel_scheduled_ping(app: &AppHandle) {
    if let Err(e) = app.notification().cancel(vec![PING_NOTIFICATION_ID]) {
        tracing::debug!("cancel scheduled ping notification: {e}");
    }
}

impl PingNotifier for PluginNotifier {
    fn notify_ping_due(&self, app: &AppHandle) -> Result<(), AppError> {
        #[cfg(mobile)]
        {
            cancel_scheduled_ping(app);
            let mut s = self.scheduled.lock().unwrap_or_else(|p| p.into_inner());
            s.synced = false;
        }
        let body = ping_body(app)?;
        app.notification()
            .builder()
            .id(PING_NOTIFICATION_ID)
            .title(PING_TITLE)
            .body(body)
            .show()
            .map_err(|e| AppError::Notify(e.to_string()))
    }

    #[cfg(mobile)]
    fn sync_scheduled_ping(&self, app: &AppHandle, next_unix: Option<i64>) {
        let mut s = self.scheduled.lock().unwrap_or_else(|p| p.into_inner());
        if s.synced && s.at == next_unix {
            return;
        }

        cancel_scheduled_ping(app);
        let Some(at) = next_unix else {
            *s = ScheduledPing {
                synced: true,
                at: None,
            };
            return;
        };

        let Ok(date) = time::OffsetDateTime::from_unix_timestamp(at) else {
            tracing::warn!("scheduled ping: invalid unix time {at}");
            return;
        };
        let body = match ping_body(app) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("scheduled ping: body: {e}");
                return;
            }
        };
        let result = app
            .notification()
            .builder()
            .id(PING_NOTIFICATION_ID)
            .title(PING_TITLE)
            .body(body)
            .schedule(Schedule::At {
                date,
                repeating: false,
                allow_while_idle: true,
            })
            .show();
        match result {
            Ok(()) => {
                *s = ScheduledPing {
                    synced: true,
                    at: Some(at),
                };
            }
            Err(e) => {
                // Leave `synced == false` so the next scheduler poll retries.
                tracing::warn!("scheduled ping: schedule: {e}");
                s.synced = false;
            }
        }
    }
}

impl DailyReminderNotifier for PluginNotifier {
    fn notify_daily_reminder(
        &self,
        app: &AppHandle,
        label: &str,
        reminder_id: i64,
    ) -> Result<(), AppError> {
        tracing::debug!("daily reminder notification (id={reminder_id})");
        app.notification()
            .builder()
            .title("Daily reminder")
            .body(trim_preview(label, 120))
            .show()
            .map_err(|e| AppError::Notify(e.to_string()))
    }
}
