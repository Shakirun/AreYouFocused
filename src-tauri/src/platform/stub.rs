use super::PingNotifier;
use tauri::AppHandle;

#[cfg_attr(windows, allow(dead_code))]
pub struct StubNotifier;

impl PingNotifier for StubNotifier {
    fn notify_ping_due(&self, _app: &AppHandle) -> Result<(), crate::error::AppError> {
        tracing::info!("ping due (stub notifier)");
        Ok(())
    }
}
