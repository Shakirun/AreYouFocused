//! Shared helpers for the main capture webview window (desktop only: tray and toast
//! activation bring the window back; mobile has a single full-screen activity).

use tauri::{AppHandle, Manager, Runtime};

pub fn show_and_focus_capture<R: Runtime>(app: &AppHandle<R>) {
    let Some(win) = app.get_webview_window("capture") else {
        tracing::debug!("window_util: no webview window labeled capture");
        return;
    };
    let _ = win.unminimize();
    let _ = win.show();
    let _ = win.set_focus();
}
