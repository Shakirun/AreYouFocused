# AreYouFocused

Desktop-first **random-ping** productivity tracker (WhatNow-style): honest, local-first, SQLite. Ships as a **Windows** installer and a test-grade **Android APK** (see **Android** below).

**Language policy:** All **public repo docs**, **in-app UI copy**, and **code comments** are **English** by default.

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

### Notifications (Android / Linux / macOS)

- Pings and daily reminders are plain system notifications via `tauri-plugin-notification` (no action buttons; tap opens the app). Toast **Snooze / Still / ±15** buttons remain Windows-only.
- **Android 13+** asks for notification permission on first launch. If you decline, the Capture tab shows a warning until you enable notifications in the phone's app settings.

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

(For phones see **Android** below.)

- **OS:** Windows 10 or later (64-bit; matches Tauri target).
- **WebView2:** UI runs in **Microsoft Edge WebView2**. **Evergreen** runtime ships with current Windows 10/11 for most users. If the window is blank or the app exits on startup, install or repair from [WebView2 Runtime — consumer download](https://developer.microsoft.com/microsoft-edge/webview2/consumer/) or [WebView2 overview](https://developer.microsoft.com/microsoft-edge/webview2/).
- **Installers:** NSIS uses **`webviewInstallMode.downloadBootstrapper`** so users without WebView2 get the official bootstrapper when needed; see [Distribute your app and the WebView2 Runtime](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution).

## Download (Windows)

Stable installers are published on **[GitHub Releases](https://github.com/Shakirun/AreYouFocused/releases)** (not stored in git).

1. Open the latest release (or the version you need).
2. Download **`AreYouFocused_<version>_x64-setup.exe`** (e.g. `AreYouFocused_0.3.0_x64-setup.exe`).
3. Run the installer. **WebView2** is required — see **Runtime (Windows desktop)** above.

Direct link pattern: `https://github.com/Shakirun/AreYouFocused/releases/download/v<version>/AreYouFocused_<version>_x64-setup.exe`

## Android

The same Tauri app builds into an APK for **Android 7.0+ (API 24)**, arm64 + armv7. It is meant for **hands-on testing on your own phone**, not for store distribution yet.

### Get the APK

- **Releases:** every `v*` tag also gets `AreYouFocused_<version>_android.apk` attached to the [GitHub Release](https://github.com/Shakirun/AreYouFocused/releases).
- **Any branch / PR:** the **[Android APK workflow](.github/workflows/android.yml)** runs on pushes to `main`/`develop`, on pull requests, and manually (**Actions → Android APK → Run workflow**). Download the `AreYouFocused_<version>_android.apk` artifact from the run summary and unzip it.

### Install on the phone

1. Copy the `.apk` to the phone (USB, cloud drive, messenger, `adb install -r AreYouFocused_<version>_android.apk`).
2. Open it and allow **Install unknown apps** for the app you opened it from (Files, browser, …).
3. On first launch allow **notifications** — pings arrive as notifications.
4. **Updating:** without a configured release keystore each CI run signs with a fresh **debug** key, so Android refuses to install a new build over an old one (“App not installed”). Uninstall the previous build first, or set up signing (below) once so updates install in place.

### What differs from desktop

- **Background pings:** the scheduler is an in-process loop, and Android freezes background apps. To bridge that, the app also hands the **next** ping to the OS as a scheduled notification (`AlarmManager`), so it fires even when the app is frozen or swiped away. Only that one ping is pre-scheduled — the chain continues once you open the app (tapping the notification is enough). **Daily reminders** are in-process only for now, so they need the app to be running. Excluding AreYouFocused from battery optimization makes both more reliable.
- Notifications have **no action buttons** (Snooze / Still / ±15 are handled inside the app).
- **CSV / XLSX / PDF export** is hidden on the phone (needs a native save dialog and a print window); **Copy** on the History tab still works.
- No system tray; the app layout fills the screen and the tab panel scrolls.

### Build locally

Prerequisites: Node 18+, Rust stable (1.85+), **JDK 17+**, Android SDK with **NDK 27** (`sdkmanager "ndk;27.2.12479018" "platform-tools"`), and Rust targets `rustup target add aarch64-linux-android armv7-linux-androideabi`.

```bash
export ANDROID_HOME=~/Android/Sdk            # or wherever the SDK lives
export NDK_HOME=$ANDROID_HOME/ndk/27.2.12479018
npm ci
npm run tauri android build -- --apk --target aarch64   # add --target armv7 for 32-bit phones
# → src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk
npm run tauri android dev                                # live-reload on a connected device/emulator
```

The Android project lives in **`src-tauri/gen/android`** and is committed (manifest, theme, edge-to-edge inset handling in `MainActivity.kt`, signing in `app/build.gradle.kts`). Do **not** re-run `tauri android init`; it would overwrite those changes.

### Release signing (optional, recommended for repeated installs)

1. Create a keystore once: `keytool -genkey -v -keystore areyoufocused.jks -keyalg RSA -keysize 2048 -validity 10000 -alias areyoufocused`
2. **Locally:** create `src-tauri/gen/android/keystore.properties` (git-ignored):

   ```properties
   keyAlias=areyoufocused
   password=<store and key password>
   storeFile=/absolute/path/areyoufocused.jks
   ```

3. **CI:** add repository secrets `ANDROID_KEYSTORE_BASE64` (`base64 -w0 areyoufocused.jks`), `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`. The workflow signs with them when present and falls back to the debug key otherwise.

## Releases (maintainers)

1. Merge **`develop` → `main`** when the release is ready; ensure **`src-tauri/tauri.conf.json`**, **`src-tauri/Cargo.toml`**, and **`package.json`** (non-`-dev` version on main) match the release number.
2. On **`main`**, create and push an annotated tag: `git tag -a v0.3.0 -m "v0.3.0"` then `git push origin v0.3.0`.
3. The **[Release workflow](.github/workflows/release.yml)** builds the NSIS installer on `windows-latest` and attaches it to the GitHub Release for that tag; the **[Android APK workflow](.github/workflows/android.yml)** attaches the APK to the same release.
4. Optional: run the same workflow manually via **Actions → Release → Run workflow** (version input must match `tauri.conf.json`).

Code signing is not configured in CI yet; Windows SmartScreen may warn on first download until signing is added later.

## Git branches

Integrate on **`develop`** (unstable `*-dev.*` versions); ship stable releases via MR **`develop` → `main`**, then tag on **`main`** as above.
