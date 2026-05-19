//! Shared helpers for the main capture webview window.

use tauri::{AppHandle, Manager, Runtime};

pub fn show_and_focus_capture<R: Runtime>(app: &AppHandle<R>) {
    let Some(win) = app.get_webview_window("capture") else {
        tracing::debug!("window_util: no webview window labeled capture");
        return;
    };
    #[cfg(desktop)]
    let _ = win.unminimize();
    let _ = win.show();
    let _ = win.set_focus();
}
