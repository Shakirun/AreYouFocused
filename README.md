# AreYouFocused

Desktop-first **random-ping** productivity tracker (WhatNow-style): honest, local-first, SQLite.

**Language policy:** All **public repo docs**, **in-app UI copy**, and **code comments** are **English** by default. Planned **i18n** for additional locales is described in [I18N.md](I18N.md).

## Features

### Scheduling

- **Random pings:** You configure a **minimum** and **maximum** interval (minutes). The next ping time is rolled at random within that range (persisted in SQLite). Change bounds in the app under **Ping interval** (`update_ping_interval`); applying rolls the next ping from “now.”
- **Optional planned duration:** Below the capture field you can enter **how many minutes** you plan to keep doing what you described. If set, the next notification is scheduled after exactly that many minutes (“planned check”). When that ping fires, the app asks whether you’re **still on that activity**. Afterward you can enter **extra minutes** to schedule another planned check, or leave minutes **blank** so the next ping follows your usual **random min–max** interval.
- **Snooze 10 min:** Moves the next ping to **now + 10 minutes** (capture UI and, on Windows, the ping toast). Snooze also **clears** an active planned-activity / follow-up state so timing stays predictable.
- **Stale next ping:** If the stored next ping is unreasonably far for your current max interval (e.g. after tightening bounds), the app **re-rolls** it when reading scheduler status or in the ping loop—except for delays that look like **snooze** or an active **planned** deadline.

### Capture UI

- **Right now:** Main text capture; **Save** stores a row in `captures` and schedules the next ping (random interval, or planned deadline if minutes are set).
- **Plan for about (minutes):** Optional; see **Scheduling** above. In **follow-up** mode (after a planned check), the same field means **extra minutes** if you’re still on the activity; leave it empty to use your random interval next.
- **Common answers:** Up to **five** most frequent saved texts as chips—tap to insert into the field, edit or save as-is.
- **Still:** Logs another entry with the **same text** as your **last** save (no retyping). Disabled until you have at least one capture.
- **Recent answers:** Collapsible list of recent captures (newest first).
- **Window height:** The capture window grows with content (logical width fixed); tray / notifications unchanged.

### Notifications (Windows desktop)

- Ping **toast** includes **Snooze 10 min** and, when there is prior history, **Still** (same as last capture). Opening the capture window works from the toast body tap / tray as before.
- When the ping is a **planned check**, toast copy reflects that (planned time up — still doing this?).

### Maintainer / QA

- **`AREYOUFOCUSED_DEV_PING_SECS`:** Set to a positive integer (seconds) to fire pings on a fixed short interval for local testing. **Unset** for normal behavior—a stray env var will affect release builds too and can look like notification spam. Example (PowerShell): `$env:AREYOUFOCUSED_DEV_PING_SECS = "15"; npm run tauri:dev`

## Development

- **Tests:** Rust domain and DB logic live in the library crate—prefer **red → green** for scheduler/repo behavior (`cd src-tauri && cargo test`). The `are-you-focused` binary is a thin shim (`test = false` on the bin).
- **Frontend:** Node 18+, `npm install`, `npm run dev` (Vite + React + Tailwind). Design tokens are in `tailwind.config.js`.
- **Desktop (Tauri 2):** Install **Rust** (stable via [rustup](https://rustup.rs/)); on Windows also **MSVC Build Tools**. The UI needs **WebView2** on Windows—see **Runtime (Windows desktop)** below if the window fails to open.
  - `npm run tauri:dev` — app + Vite dev server; background scheduler waits for `next_ping_at`, shows a ping notification, then applies rescheduling rules above. While the capture window is hidden, pings **do not** force it open; when visible, **Last ping at …** updates via the `ping-due` event.
  - **System tray:** Closing the capture window **minimizes to tray** (does not quit). **Left-click** tray icon or menu **Show capture window** / **Quit AreYouFocused**. Requires the `tray-icon` feature on `tauri` (enabled in this repo).
  - **Scheduler API:** `get_scheduler_status` exposes next ping time, interval bounds, and follow-up state (`awaitingFollowup`, `plannedCheckSubject`). `submit_capture` accepts `plannedDurationMinutes` (optional). `list_recent_captures`, `list_top_quick_picks`, `repeat_last_capture`, `snooze_ping`, `update_ping_interval` as documented in code.
  - **`npm run tauri:build`** — production frontend + Rust + **NSIS** installer (Windows x64). Output under **`src-tauri/target/release/bundle/nsis/`** as `AreYouFocused_*_x64-setup.exe` (version in filename).
  - **Icons:** Source **`src-tauri/icons-source/app-icon.png`**. Regenerate with `npm run tauri -- icon src-tauri/icons-source/app-icon.png`. Release builds need [Microsoft Visual Studio C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/).

Optional editor-specific tooling or personal notes can live outside tracked files (see `.gitignore`); they are **not** required to build or run the app.

## Runtime (Windows desktop)

- **OS:** Windows 10 or later (64-bit; matches Tauri target).
- **WebView2:** UI runs in **Microsoft Edge WebView2**. **Evergreen** runtime ships with current Windows 10/11 for most users. If the window is blank or the app exits on startup, install or repair from [WebView2 Runtime — consumer download](https://developer.microsoft.com/microsoft-edge/webview2/consumer/) or [WebView2 overview](https://developer.microsoft.com/microsoft-edge/webview2/).
- **Installers:** NSIS uses **`webviewInstallMode.downloadBootstrapper`** so users without WebView2 get the official bootstrapper when needed; see [Distribute your app and the WebView2 Runtime](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution).

## Git branches

Integrate on **`develop`** (unstable `*-dev.*` versions); ship stable releases via MR **`develop` → `main`**. Details: [RELEASING.md](RELEASING.md).
