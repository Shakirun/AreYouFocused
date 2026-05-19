mod stub;
#[cfg(target_os = "windows")]
mod aumid;
#[cfg(target_os = "windows")]
mod win_toast;
#[cfg(target_os = "windows")]
mod windows;

use crate::error::AppError;
use tauri::AppHandle;

pub trait PingNotifier: Send + Sync + 'static {
    fn notify_ping_due(&self, app: &AppHandle) -> Result<(), AppError>;
}

#[cfg(target_os = "windows")]
pub fn init_notifications(app: &AppHandle) {
    windows::init_notifications(app);
}

pub fn current_notifier() -> Box<dyn PingNotifier> {
    #[cfg(target_os = "windows")]
    {
        return Box::new(windows::WindowsNotifier);
    }
    #[cfg(not(target_os = "windows"))]
    {
        Box::new(stub::StubNotifier)
    }
}
