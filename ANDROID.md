# Android build (Tauri 2)

AreYouFocused uses **Tauri 2** for Android. The repo includes `src-tauri/tauri.android.conf.json`; the Gradle project under `src-tauri/gen/android/` is created on your machine by `android init` (not committed — see `.gitignore`).

## Prerequisites (Windows)

1. **Android Studio** — [developer.android.com/studio](https://developer.android.com/studio)
2. **SDK components** (SDK Manager): Platform-Tools, Build-Tools, Platform (API 34+), NDK (side by side), Command-line Tools
3. **Environment variables** (PowerShell profile or System):

```powershell
$env:JAVA_HOME = "C:\Program Files\Android\Android Studio\jbr"
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
$env:NDK_HOME = "$env:ANDROID_HOME\ndk\<ndk-version>"   # folder name from Sdk\ndk\
$env:PATH += ";$env:ANDROID_HOME\platform-tools"
```

4. **Rust Android targets:**

```powershell
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
```

## One-time init

From the project root (after prerequisites):

```powershell
npm install
npm run tauri:android:init
```

This generates `src-tauri/gen/android/`. Re-run after major Tauri upgrades if the CLI asks you to.

## Dev on device / emulator

```powershell
npm run tauri:android:dev
```

Connect a phone with USB debugging or start an AVD in Android Studio.

## Release APK / AAB

```powershell
npm run build
npm run tauri:android:build
```

Artifacts are under `src-tauri/gen/android/app/build/outputs/`.

## MVP limitations on Android

- **Notifications:** Windows uses WinRT toasts with actions; Android currently uses the **stub notifier** (log only). Scheduler still emits in-app `ping-due` events when the app is in the foreground.
- **System tray:** Desktop only; not available on Android.
- **Background pings:** Not guaranteed when the OS suspends the app (Doze). Treat phone builds as foreground-first for now.

Windows desktop build is unchanged: `npm run tauri:build`.
