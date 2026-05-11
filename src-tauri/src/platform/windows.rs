use super::PingNotifier;
use crate::error::AppError;
use crate::window_util;
use tauri::AppHandle;
use tauri_winrt_notification::{Duration, Toast};

pub struct WindowsNotifier;

fn toast_app_id(app: &AppHandle) -> String {
    let identifier = app.config().identifier.clone();
    let Ok(exe) = std::env::current_exe() else {
        return identifier;
    };
    let Some(dir) = exe.parent() else {
        return identifier;
    };
    let path = dir.to_string_lossy();
    if path.ends_with(r"target\debug")
        || path.ends_with("target/debug")
        || path.ends_with(r"target\release")
        || path.ends_with("target/release")
    {
        Toast::POWERSHELL_APP_ID.to_string()
    } else {
        identifier
    }
}

impl PingNotifier for WindowsNotifier {
    fn notify_ping_due(&self, app: &AppHandle) -> Result<(), AppError> {
        let app_id = toast_app_id(app);
        let app_for_activation = app.clone();
        Toast::new(&app_id)
            .title("AreYouFocused")
            .text2("What are you doing right now?")
            .duration(Duration::Short)
            .on_activated(move |_action| {
                let h = app_for_activation.clone();
                let h2 = h.clone();
                if let Err(e) = h.run_on_main_thread(move || {
                    window_util::show_and_focus_capture(&h2);
                }) {
                    tracing::warn!("toast activation: run_on_main_thread failed: {e}");
                }
                Ok(())
            })
            .show()
            .map_err(|e| AppError::Notify(e.to_string()))
    }
}
