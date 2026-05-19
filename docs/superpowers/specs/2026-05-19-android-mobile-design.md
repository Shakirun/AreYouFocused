# AreYouFocused Android & Mobile UI Design

**Date:** 2026-05-19  
**Status:** Approved for MVP implementation

## Goal

Enable Tauri 2 Android APK builds for phone testing and adapt the four-tab UI (Capture, History, Schedule, Routines) for mobile viewports without breaking the Windows desktop build.

## Constraints (brainstorming)

| Area | Windows desktop | Android mobile |
|------|-----------------|----------------|
| Navigation | Top horizontal tab bar in fixed-width window | Bottom tab bar (4 equal destinations) |
| Window | Fit-to-content height, non-resizable, close → tray hide | Full viewport (`100dvh`), system back closes app |
| Notifications | WinRT toast with action buttons (Still, Done, Snooze) | **MVP:** stub notifier — logs only; in-app `ping-due` events still fire when foreground |
| System tray | Required for background pings | N/A — app runs as normal Android activity |
| Touch | Mouse-sized targets OK | Min 44×44px targets, 8px gap, safe-area insets |

## Navigation decision

**Chosen: bottom tab bar (Option A)** over burger drawer.

- Four tabs are peer primary destinations; bottom nav exposes all at once (Material / iOS HIG pattern for 3–5 tabs).
- Burger menu adds an extra tap and hides wayfinding; better for 5+ destinations or infrequent sections.
- 21st.dev inspiration: Modern Mobile Menu — icon + label, active accent, fixed bottom bar. Implemented with existing Tailwind tokens (no Lucide/framer-motion deps).

## Architecture

- **Tauri 2 mobile:** `#[cfg_attr(mobile, tauri::mobile_entry_point)]` already present; add `tauri.android.conf.json`, npm android scripts, `ANDROID.md` for machine setup.
- **Rust:** Keep `#[cfg(desktop)]` for tray and close-to-hide; Android uses existing `StubNotifier` (document limitation).
- **Frontend:** `useMobileLayout()` (viewport + coarse pointer) switches shell: desktop top tabs + fit-window; mobile bottom nav + full-height scroll.

## Out of scope (MVP)

- Native Android notification channels / action buttons
- iOS build
- Background ping while app is killed (Android Doze)
