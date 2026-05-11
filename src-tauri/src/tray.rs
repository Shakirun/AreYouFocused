//! System tray: hide to tray, show from tray, quit.

use tauri::image::Image;
use tauri::menu::{Menu, MenuItemBuilder, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, Runtime};

use crate::window_util;

fn tray_icon_image() -> Image<'static> {
    const S: u32 = 32;
    let mut rgba = vec![0u8; (S * S * 4) as usize];
    for px in rgba.chunks_exact_mut(4) {
        px[0] = 13;
        px[1] = 148;
        px[2] = 136;
        px[3] = 255;
    }
    Image::new_owned(rgba, S, S)
}

pub fn setup_tray<R: Runtime>(app: &App<R>) -> tauri::Result<()> {
    let show = MenuItemBuilder::with_id("tray_show", "Show capture window").build(app)?;
    let hide = MenuItemBuilder::with_id("tray_hide", "Hide to tray").build(app)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItemBuilder::with_id("tray_quit", "Quit AreYouFocused").build(app)?;
    let menu = Menu::with_items(app, &[&show, &hide, &sep, &quit])?;

    let _tray = TrayIconBuilder::new()
        .icon(tray_icon_image())
        .tooltip("AreYouFocused")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "tray_show" => window_util::show_and_focus_capture(app),
            "tray_hide" => window_util::hide_capture(app),
            "tray_quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = &event
            {
                window_util::show_and_focus_capture(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}
