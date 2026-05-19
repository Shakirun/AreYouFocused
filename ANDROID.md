# Android build (Tauri 2)

AreYouFocused uses **Tauri 2** for Android. The repo includes `src-tauri/tauri.android.conf.json`; the Gradle project under `src-tauri/gen/android/` is created on your machine by `android init` (not committed — see `.gitignore`).

## Prerequisites (Windows)

### 1. Install Android Studio

If Android Studio is not installed:

```powershell
winget install Google.AndroidStudio
```

Or download from [developer.android.com/studio](https://developer.android.com/studio).

After install, open Android Studio once and complete the setup wizard so it downloads the SDK.

### 2. SDK components (SDK Manager)

Install: **Platform-Tools**, **Build-Tools**, **Platform (API 34+)**, **NDK (side by side)**, **Command-line Tools**.

Default SDK location:

```text
%LOCALAPPDATA%\Android\Sdk
```

(PowerShell: `$env:LOCALAPPDATA\Android\Sdk`)

### 3. Set JAVA_HOME (required for `tauri android init`)

Tauri/Gradle need a JDK. Android Studio ships **JetBrains Runtime (JBR)** — use that instead of installing a separate JDK.

Tauri checks, in order:

1. `java` on `PATH`
2. Default Android Studio JBR (see paths below)
3. `JAVA_HOME`

**Common JBR locations on Windows** (use whichever exists on your machine):

| Install type | JBR path |
|--------------|----------|
| System-wide (most common) | `C:\Program Files\Android\Android Studio\jbr` |
| User install (winget / per-user) | `%LOCALAPPDATA%\Programs\Android Studio\jbr` |

**Set permanently (User environment — no admin):**

```powershell
# Replace with the path that exists on your machine (see setup script below)
[Environment]::SetEnvironmentVariable('JAVA_HOME', 'C:\Program Files\Android\Android Studio\jbr', 'User')

# Optional: put java on PATH for this account
$javaBin = "$([Environment]::GetEnvironmentVariable('JAVA_HOME','User'))\bin"
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if ($userPath -notlike "*$javaBin*") {
  [Environment]::SetEnvironmentVariable('Path', "$userPath;$javaBin", 'User')
}
```

Open a **new** PowerShell window after changing user env vars.

**Quick detect + export commands:**

```powershell
.\scripts\setup-android-env.ps1
```

Use `-SetUserEnv` to apply `JAVA_HOME`, `ANDROID_HOME`, and SDK `platform-tools` to your user profile:

```powershell
.\scripts\setup-android-env.ps1 -SetUserEnv
```

### 4. Set ANDROID_HOME

```powershell
[Environment]::SetEnvironmentVariable('ANDROID_HOME', "$env:LOCALAPPDATA\Android\Sdk", 'User')
```

Session-only (current terminal):

```powershell
$env:JAVA_HOME = "C:\Program Files\Android\Android Studio\jbr"   # or LocalAppData path
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
$env:PATH += ";$env:ANDROID_HOME\platform-tools"
```

**NDK** (after SDK Manager installs NDK side-by-side):

```powershell
$env:NDK_HOME = "$env:ANDROID_HOME\ndk\<ndk-version>"   # folder name from Sdk\ndk\
```

### 5. Verify environment

In a **new** PowerShell window:

```powershell
java -version
$env:JAVA_HOME
$env:ANDROID_HOME
adb version
```

Expected:

- `java -version` prints OpenJDK/JBR 17+ (from Android Studio JBR)
- `$env:JAVA_HOME` points to the `jbr` folder (not `...\jbr\bin`)
- `$env:ANDROID_HOME` points to your SDK folder
- `adb version` works if `platform-tools` is on `PATH`

If you see:

```text
failed to ensure Android environment: Java not found in PATH, default Android Studio Java installation not found at {default_java_home} and JAVA_HOME environment variable not set
```

→ Android Studio is missing, JBR is in a non-default path, or `JAVA_HOME` is unset. Run `.\scripts\setup-android-env.ps1` and set `JAVA_HOME` to the reported JBR path.

### 6. Rust Android targets

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
