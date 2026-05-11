use super::PingNotifier;
use crate::error::AppError;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

pub struct WindowsNotifier;

impl PingNotifier for WindowsNotifier {
    fn notify_ping_due(&self, app: &AppHandle) -> Result<(), AppError> {
        app.notification()
            .builder()
            .title("AreYouFocused")
            .body("What are you doing right now?")
            .show()
            .map_err(|e| AppError::Notify(e.to_string()))
    }
}
