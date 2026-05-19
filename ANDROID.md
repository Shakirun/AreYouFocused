# Android build (Tauri 2)

AreYouFocused uses **Tauri 2** for Android. The repo includes `src-tauri/tauri.android.conf.json`; the Gradle project under `src-tauri/gen/android/` is created on your machine by `android init` (not committed — see `.gitignore`).

## Prerequisites (Windows)

> **Build blocked with `os error 4551` / “Application Control policy”?**  
> Windows 11 **Smart App Control** blocks Rust build scripts during `cargo build`. This is an environment issue, not an AreYouFocused bug.  
> **Quick fix:** turn SAC **Off**, reboot, then `cargo clean` and rebuild. Full ranked steps (SAC, WSL2, Defender exclusions, Developer Mode): **[BUILD-WINDOWS.md](BUILD-WINDOWS.md)**.

> **Rust compiled, then failed on `lib*.so` / “Creation symbolic link is not allowed”?**  
> After `cargo build` for Android targets, Tauri links compiled `.so` files into `jniLibs` with **symbolic links**. On Windows 10/11 this requires **Developer Mode** (or an elevated terminal with symlink privilege). There is **no** `tauri.conf.json` flag or CLI switch to copy instead — see [Developer Mode (required for jniLibs symlinks)](#developer-mode-required-for-jnilibs-symlinks-on-windows) below.

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

#### Manual SDK setup (skipped Tauri / cmdline-tools install)

If `tauri android init` reports **SDK not found** or you answered **no** when asked to install command-line tools, install the SDK through Android Studio instead:

1. **Open Android Studio** and finish the first-run setup wizard (this creates the SDK folder and downloads base packages).
2. **File → Settings** (or **Android Studio → Settings** on macOS).
3. **Languages & Frameworks → Android SDK**.
4. Copy **Android SDK Location** (typical Windows path: `C:\Users\<you>\AppData\Local\Android\Sdk`).
5. **SDK Platforms** tab: check **Android 14.0 (API 34)** or newer → **Apply**.
6. **SDK Tools** tab: check at least:
   - Android SDK Build-Tools
   - Android SDK Platform-Tools
   - NDK (Side by side)
   - Android SDK Command-line Tools (latest)
7. **Apply → OK** and wait for downloads to finish.
8. Set the user environment variable (replace the path if yours differs):

   ```powershell
   [Environment]::SetEnvironmentVariable('ANDROID_HOME', 'C:\Users\Oleg\AppData\Local\Android\Sdk', 'User')
   ```

   Or run the setup script (detects JBR + SDK path):

   ```powershell
   .\scripts\setup-android-env.ps1 -SetUserEnv
   ```

9. **Close and reopen** PowerShell, then verify:

   ```powershell
   $env:ANDROID_HOME
   Test-Path "$env:ANDROID_HOME\platform-tools\adb.exe"
   ```

10. Re-run init: `npm run tauri:android:init`

**Empty SDK folder:** If `%LOCALAPPDATA%\Android\Sdk` exists but has no `platform-tools`, `build-tools`, or `platforms` subfolders, Android Studio has not installed components yet — complete steps 1–7 above before running Tauri.

#### Alternative: let Tauri install cmdline-tools

When `npm run tauri:android:init` prompts to install Android command-line tools, answering **yes** can download **cmdline-tools** into your SDK automatically. You still need **Platform**, **Build-Tools**, **NDK**, and **Platform-Tools** from SDK Manager (or `sdkmanager` after cmdline-tools exist). Studio-first setup is usually easier on a fresh Windows machine.

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

If **Java is OK** but the SDK is not found (`ANDROID_HOME` unset, or SDK path empty):

→ Run `.\scripts\setup-android-env.ps1` — it checks `%LOCALAPPDATA%\Android\Sdk`, `%USERPROFILE%\AppData\Local\Android\Sdk`, `C:\Android\Sdk`, Android Studio `android.sdk.path.xml`, and reports which folders exist vs are populated.

→ If the script shows **folder exists but empty / incomplete**, follow [Manual SDK setup](#manual-sdk-setup-skipped-tauri--cmdline-tools-install) above (open Android Studio, SDK Manager, install components).

→ After components are installed, use `-SetUserEnv` and open a **new** terminal before `npm run tauri:android:init`.

### 6. Rust Android targets

```powershell
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
```

### Developer Mode (required for jniLibs symlinks on Windows)

Tauri 2 (via `cargo-mobile2`) always **symlinks** built native libraries from `src-tauri/target/<android-triple>/…/lib*.so` into `src-tauri/gen/android/app/src/main/jniLibs/<abi>/`. On Windows, creating symlinks without privilege fails with:

```text
Failed to create a symbolic link … libare_you_focused_lib.so …
Creation symbolic link is not allowed for this system.
For Windows 10 or newer: You should use developer mode.
```

**There is no copy fallback** in current Tauri CLI (`tauri android build` / `dev`), `tauri.conf.json`, or documented env vars — enable symlinks on the host.

#### Recommended: Developer Mode (no admin each time)

**English UI**

1. **Settings** → **Privacy & security** → **For developers**
2. Turn **Developer Mode** **On**
3. Confirm the dialog if Windows asks
4. **Reboot** if the toggle was off before (recommended)
5. Open a **new** PowerShell in the project folder and rebuild:

   ```powershell
   npm run tauri:android:build -- --apk
   ```

Shortcut: **Win + R** → `ms-settings:developers`

**Русский интерфейс**

1. **Параметры** → **Конфиденциальность и защита** → **Для разработчиков**
2. Включите **Режим разработчика**
3. Подтвердите запрос Windows, если появится
4. **Перезагрузите** ПК, если режим был выключен (рекомендуется)
5. Откройте **новый** PowerShell в папке проекта и пересоберите:

   ```powershell
   npm run tauri:android:build -- --apk
   ```

#### Alternative: elevated terminal (admin)

Running PowerShell **Run as administrator** grants `SeCreateSymbolicLinkPrivilege` for that session. This works but is less convenient than Developer Mode for day-to-day builds. See [BUILD-WINDOWS.md — Developer Mode & symlinks](BUILD-WINDOWS.md#4-developer-mode-android-jnilibs-symlinks--optional-for-sac).

#### Other symlink pitfalls

- Project on **exFAT / FAT32** (common on USB drives): symlinks are not supported — keep the repo on **NTFS** (e.g. `C:\Users\…`).
- Stale `jniLibs` files from a failed run: delete `src-tauri\gen\android\app\src\main\jniLibs\` and rebuild.

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
npm run tauri:android:build -- --apk
```

For Google Play (AAB only): `npm run tauri:android:build` (no `--apk`).

Artifacts are under `src-tauri/gen/android/app/build/outputs/`.

## MVP limitations on Android

- **Notifications:** Windows uses WinRT toasts with actions; Android currently uses the **stub notifier** (log only). Scheduler still emits in-app `ping-due` events when the app is in the foreground.
- **System tray:** Desktop only; not available on Android.
- **Background pings:** Not guaranteed when the OS suspends the app (Doze). Treat phone builds as foreground-first for now.

Windows desktop build is unchanged: `npm run tauri:build`.

## Troubleshooting (Windows)

| Symptom | Likely cause | Action |
|---------|--------------|--------|
| `Application Control policy has blocked this file (os error 4551)` on `build-script-build` | Smart App Control | [BUILD-WINDOWS.md](BUILD-WINDOWS.md) — disable SAC or use WSL2 |
| `Creation symbolic link is not allowed` / `Failed to create a symbolic link` for `lib*.so` in `jniLibs` | Windows symlink privilege | [Developer Mode](#developer-mode-required-for-jnilibs-symlinks-on-windows) **On**, reboot, new terminal, rebuild |
| `Incorrect function` / `os error 1` on jniLibs symlink | exFAT/FAT32 or external drive | Move project to NTFS (e.g. system drive) |
| Build still fails after disabling SAC | Stale blocked artifacts | `cd src-tauri; cargo clean; cd ..` then rebuild |
| `Java not found` / SDK not found | `JAVA_HOME` / `ANDROID_HOME` | [Verify environment](#5-verify-environment), `.\scripts\setup-android-env.ps1` |
| Slow Rust compiles, Defender warnings | Antivirus scanning `target\` | Defender exclusions in [BUILD-WINDOWS.md](BUILD-WINDOWS.md) (does **not** replace disabling SAC) |
