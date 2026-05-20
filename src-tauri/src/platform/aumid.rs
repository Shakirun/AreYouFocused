//! Register App User Model ID so unpackaged / dev builds get proper toast branding and activation.

use std::path::Path;
use windows::core::HSTRING;
use windows_registry::CURRENT_USER;

/// Ensures `HKCU\Software\Classes\AppUserModelId\{app_id}` exists (idempotent).
pub fn ensure_registered(app_id: &str, display_name: &str, icon_path: &Path) -> Result<(), String> {
    let key_path = format!(r"SOFTWARE\Classes\AppUserModelId\{app_id}");
    let key = CURRENT_USER
        .create(&key_path)
        .map_err(|e| format!("create AUMID registry key: {e}"))?;
    key.set_string("DisplayName", display_name)
        .map_err(|e| format!("set DisplayName: {e}"))?;
    key.set_string("IconBackgroundColor", "0")
        .map_err(|e| format!("set IconBackgroundColor: {e}"))?;
    if icon_path.exists() {
        key.set_hstring("IconUri", &HSTRING::from(icon_path.display().to_string()))
            .map_err(|e| format!("set IconUri: {e}"))?;
    }
    Ok(())
}
