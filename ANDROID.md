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

#### `sdkmanager` not found / wrong path (Windows)

Gradle, Tauri, and many scripts expect:

```text
%LOCALAPPDATA%\Android\Sdk\cmdline-tools\latest\bin\sdkmanager.bat
```

On some machines **Command-line Tools are installed but under a different layout** (manual ZIP extract, older Tauri download, or Studio placing files without a `latest` folder). Typical alternate path:

```text
%LOCALAPPDATA%\Android\Sdk\cmdline-tools\bin\sdkmanager.bat
```

**Find `sdkmanager` on your PC (copy-paste in PowerShell):**

```powershell
Get-ChildItem "$env:LOCALAPPDATA\Android\Sdk" -Recurse -Filter sdkmanager.bat -ErrorAction SilentlyContinue
```

If the command prints a path, use that full path (or add its `bin` folder to `PATH` for the session):

```powershell
$sdkmanager = (Get-ChildItem "$env:LOCALAPPDATA\Android\Sdk" -Recurse -Filter sdkmanager.bat -ErrorAction SilentlyContinue | Select-Object -First 1).FullName
& $sdkmanager --list
```

**Install or fix Command-line Tools in Android Studio** (when search returns nothing, or you want the standard `latest` layout):

1. Open **Android Studio**.
2. **File → Settings** (Windows) or **Android Studio → Settings** (macOS).
3. **Languages & Frameworks → Android SDK**.
4. Open the **SDK Tools** tab.
5. Enable **Android SDK Command-line Tools (latest)** (exact checkbox label in current Studio).
6. **Apply → OK** and wait for the download.

After Studio installs them, `sdkmanager` is usually at:

```text
%LOCALAPPDATA%\Android\Sdk\cmdline-tools\latest\bin\sdkmanager.bat
```

**Alternative (no `sdkmanager` in terminal):** install everything from Studio and accept licenses there — you do **not** need `sdkmanager` on `PATH` if Studio manages the SDK:

1. Same path: **Settings → Languages & Frameworks → Android SDK**.
2. **SDK Platforms** tab: install **Android 16 / API 36** (or the API level your project targets).
3. **SDK Tools** tab: install **Android SDK Build-Tools 35** (or newer), **Platform-Tools**, **NDK (Side by side)**.
4. Click **Apply**; Studio downloads packages and accepts SDK licenses in the UI.

Then verify folders exist (PowerShell):

```powershell
Test-Path "$env:ANDROID_HOME\platform-tools\adb.exe"
Get-ChildItem "$env:ANDROID_HOME\build-tools" -ErrorAction SilentlyContinue
Get-ChildItem "$env:ANDROID_HOME\platforms" -ErrorAction SilentlyContinue
```

| SDK folder | Required for Tauri Android | Typical symptom if missing |
|------------|----------------------------|----------------------------|
| `platform-tools` | Yes (`adb`) | `adb` not found |
| `build-tools` | Yes | Gradle / `aapt` errors |
| `platforms` | Yes | Missing `android.jar` for compile SDK |
| `ndk` | Yes | NDK / `clang` errors |
| `cmdline-tools\latest\bin` | Optional if Studio installed the rest | Scripts looking for `sdkmanager.bat` fail |

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
   npm run tauri:android:build:apk
   ```

Shortcut: **Win + R** → `ms-settings:developers`

**Русский интерфейс**

1. **Параметры** → **Конфиденциальность и защита** → **Для разработчиков**
2. Включите **Режим разработчика**
3. Подтвердите запрос Windows, если появится
4. **Перезагрузите** ПК, если режим был выключен (рекомендуется)
5. Откройте **новый** PowerShell в папке проекта и пересоберите:

   ```powershell
   npm run tauri:android:build:apk
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
npm run tauri:android:build:apk
```

Use `npm run tauri:android:build:apk` for release APKs.

For Google Play (AAB only): `npm run tauri:android:build` (no `--apk`).

Artifacts are under `src-tauri/gen/android/app/build/outputs/`.

**Without a release keystore**, Gradle produces `app-universal-release-unsigned.apk`. That file is **not installable** on a phone (see [Install on phone](#install-on-phone-signed-vs-unsigned-apk)).

## Install on phone (signed vs unsigned APK)

### Why `app-universal-release-unsigned.apk` is “invalid”

Android requires every installable APK/AAB to be **signed** with a certificate. Since Android 7 (API 24), sideloaded packages must be signed; unsigned release outputs fail with errors such as:

- **“App not installed”** / **“Invalid package”** when tapping the file in Files
- `adb install`: `INSTALL_PARSE_FAILED_NO_CERTIFICATES` or `Failure [INSTALL_PARSE_FAILED_NO_CERTIFICATES]`

`npm run tauri:android:build:apk` (release, no `keystore.properties`) intentionally builds an **unsigned** artifact for CI or manual signing before Play Store upload. It is not meant for direct phone install.

Tauri does **not** put Android keystore paths in `tauri.conf.json` / `tauri.android.conf.json`. Signing is configured in **Gradle** under `src-tauri/gen/android/` ([official docs](https://v2.tauri.app/distribute/sign/android/)).

### Recommended: debug APK (auto debug-signed)

For day-to-day testing on a physical device:

```powershell
npm run build
npm run tauri:android:build:debug
```

This runs `tauri android build --debug --apk`. Gradle signs with the standard **debug keystore** (`%USERPROFILE%\.android\debug.keystore`, passwords `android` / alias `androiddebugkey`), created on first Android Studio or debug build.

**Output (typical):**

`src-tauri\gen\android\app\build\outputs\apk\universal\debug\app-universal-debug.apk`

### Install with `adb` (USB debugging)

1. On the phone: **Settings → Developer options → USB debugging** (on).
2. Connect USB; accept the RSA prompt on the phone.
3. Verify device:

```powershell
adb devices
```

4. Install (replace path if yours differs):

```powershell
adb install -r "src-tauri\gen\android\app\build\outputs\apk\universal\debug\app-universal-debug.apk"
```

`-r` replaces an existing install with the same `applicationId` (`dev.areyoufocused.app`).

**`adb install` vs copying the APK manually:** Both need a **signed** APK. `adb` shows explicit errors (`INSTALL_*`); the system UI often only says “invalid” or “not installed”. Unsigned release APKs fail either way.

### Alternative: `tauri android dev` (build + deploy)

```powershell
npm run tauri:android:dev
```

With USB debugging enabled, the CLI builds a debug build and installs/launches on the connected device (no manual APK copy).

### Already built unsigned release? Sign with debug keystore (local only)

Not for production or Play Store — only to test a release-shaped APK without rebuilding:

```powershell
.\scripts\sign-android-apk-debug.ps1
adb install -r "src-tauri\gen\android\app\build\outputs\apk\universal\release\app-universal-release-unsigned-signed-debug.apk"
```

Manual signing (same debug keystore), if you prefer:

```powershell
$bt = (Get-ChildItem "$env:ANDROID_HOME\build-tools" | Sort-Object Name -Descending | Select-Object -First 1).FullName
& "$bt\apksigner.bat" sign --ks "$env:USERPROFILE\.android\debug.keystore" --ks-pass pass:android --key-pass pass:android --ks-key-alias androiddebugkey --out signed.apk "src-tauri\gen\android\app\build\outputs\apk\universal\release\app-universal-release-unsigned.apk"
```

(`jarsigner` can sign APKs but **apksigner** is required for modern APK Signature Scheme v2/v3 — use `apksigner` from SDK build-tools.)

### Release APK signed for Play Store / real installs

1. Create a keystore ([Tauri Android signing](https://v2.tauri.app/distribute/sign/android/)).
2. Copy `src-tauri/android/keystore.properties.example` → `src-tauri/gen/android/keystore.properties` and fill in paths/passwords.
3. Run once: `.\scripts\patch-android-release-signing.ps1` (patches `gen/android/app/build.gradle.kts`; re-run after `tauri android init` if Gradle was regenerated).
4. `npm run tauri:android:build:apk` — when `keystore.properties` exists, the release APK is signed (no `-unsigned` suffix).

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
| “Invalid package” / App not installed / `INSTALL_PARSE_FAILED_NO_CERTIFICATES` | Unsigned release APK | [Install on phone](#install-on-phone-signed-vs-unsigned-apk) — use `npm run tauri:android:build:debug` or sign release APK |
