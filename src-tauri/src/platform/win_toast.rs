//! WinRT ping toasts: unique tags (no stacking), reminder scenario, launch activation,
//! retained handles for Action Center clicks while the tray app runs.

use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri_winrt_notification::Error;
use windows::core::{IInspectable, Interface, HSTRING};
use windows::Data::Xml::Dom::XmlDocument;
use windows::Foundation::{DateTime, IReference, PropertyValue, TypedEventHandler};
use windows::UI::Notifications::{
    ToastActivatedEventArgs, ToastNotification, ToastNotificationManager,
};

/// Passed on toast body tap / Action Center click when no button was pressed.
pub const TOAST_LAUNCH_FOCUS: &str = "focus";

/// How long notifications remain in Action Center before the OS may prune them.
const ACTION_CENTER_RETENTION: Duration = Duration::from_secs(5 * 60);

/// Keep enough handles that historical Action Center entries still activate the app.
const MAX_RETAINED_TOASTS: usize = 64;

static RETAINED_TOASTS: Mutex<Vec<ToastNotification>> = Mutex::new(Vec::new());

pub struct ToastButton {
    pub label: String,
    pub action: String,
}

pub struct PingToastContent {
    pub title: String,
    pub line2: String,
    pub buttons: Vec<ToastButton>,
}

fn unique_toast_tag() -> String {
    format!(
        "ping-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    )
}

fn system_time_to_winrt(st: SystemTime) -> DateTime {
    const WINDOWS_EPOCH_TICKS: i64 = 116_444_736_000_000_000;
    let duration = st.duration_since(UNIX_EPOCH).unwrap_or_default();
    let ticks = WINDOWS_EPOCH_TICKS
        + (duration.as_secs() as i64) * 10_000_000
        + (duration.subsec_nanos() as i64 / 100);
    DateTime { UniversalTime: ticks }
}

fn retain_toast(toast: ToastNotification) {
    let mut guard = RETAINED_TOASTS.lock().unwrap_or_else(|e| e.into_inner());
    guard.push(toast);
    let overflow = guard.len().saturating_sub(MAX_RETAINED_TOASTS);
    if overflow > 0 {
        guard.drain(0..overflow);
    }
}

fn activated_argument(insp: &Option<IInspectable>) -> Option<String> {
    let insp = insp.as_ref()?;
    let args = insp.cast::<ToastActivatedEventArgs>().ok()?;
    let arguments = args.Arguments().ok()?;
    if arguments.is_empty() {
        None
    } else {
        Some(arguments.to_string())
    }
}

fn build_actions(buttons: &[ToastButton]) -> String {
    if buttons.is_empty() {
        return String::new();
    }
    let mut actions = String::from("<actions>");
    for b in buttons {
        actions.push_str(&format!(
            "<action content='{}' arguments='{}'/>",
            quick_xml::escape::escape(&b.label),
            quick_xml::escape::escape(&b.action)
        ));
    }
    actions.push_str("</actions>");
    actions
}

/// Show a ping toast:
/// - `scenario="reminder"` — stays on screen until dismissed (OS may still auto-hide the banner)
/// - `duration="long"` — up to ~25s banner visibility where supported
/// - unique `tag` per notification — avoids "+N notifications" stacking
/// - `launch` — body / Action Center activation argument
/// - retained `ToastNotification` — historical clicks work while the process lives
pub fn show_ping_toast<F>(
    app_id: &str,
    content: &PingToastContent,
    mut on_activated: F,
) -> Result<(), Error>
where
    F: FnMut(Option<String>) -> Result<(), Error> + Send + 'static,
{
    let tag = unique_toast_tag();
    let actions = build_actions(&content.buttons);

    let xml = format!(
        r#"<toast launch="{}" duration="long" scenario="reminder">
            <visual>
                <binding template="ToastGeneric">
                    <text id="1">{}</text>
                    <text id="3">{}</text>
                </binding>
            </visual>
            <audio src="ms-winsoundevent:Notification.Reminder" />
            {}
        </toast>"#,
        quick_xml::escape::escape(TOAST_LAUNCH_FOCUS),
        quick_xml::escape::escape(&content.title),
        quick_xml::escape::escape(&content.line2),
        actions
    );

    let toast_xml = XmlDocument::new()?;
    toast_xml.LoadXml(&HSTRING::from(xml))?;

    let toast_template = ToastNotification::CreateToastNotification(&toast_xml)?;
    toast_template.SetTag(&HSTRING::from(&tag))?;

    let expires_at = SystemTime::now() + ACTION_CENTER_RETENTION;
    let expiration_value = PropertyValue::CreateDateTime(system_time_to_winrt(expires_at))?;
    let expiration: IReference<DateTime> = expiration_value.cast()?;
    toast_template.SetExpirationTime(&expiration)?;

    toast_template.Activated(&TypedEventHandler::new(
        move |_, insp| {
            let _ = on_activated(activated_argument(&insp));
            Ok(())
        },
    ))?;

    let toast_notifier =
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(app_id))?;

    toast_notifier.Show(&toast_template)?;
    retain_toast(toast_template);
    std::thread::sleep(std::time::Duration::from_millis(10));
    Ok(())
}
