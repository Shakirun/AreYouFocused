use super::{DailyReminderNotifier, PingNotifier};
use tauri::AppHandle;

#[cfg_attr(windows, allow(dead_code))]
pub struct StubNotifier;

impl PingNotifier for StubNotifier {
    fn notify_ping_due(&self, _app: &AppHandle) -> Result<(), crate::error::AppError> {
        tracing::info!("ping due (stub notifier)");
        Ok(())
    }
}

impl DailyReminderNotifier for StubNotifier {
    fn notify_daily_reminder(
        &self,
        _app: &AppHandle,
        label: &str,
        reminder_id: i64,
    ) -> Result<(), crate::error::AppError> {
        tracing::info!("daily reminder (stub): {label} (id={reminder_id})");
        Ok(())
    }
}
