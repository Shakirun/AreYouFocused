# Windows build troubleshooting

AreYouFocused uses **Rust + Cargo** for desktop (`npm run tauri:build`) and Android (`npm run tauri:android:build`). On **Windows 11**, **Smart App Control (SAC)** can block Cargo **build scripts** — small helper executables compiled during `cargo build`.

## Symptom

```text
error: failed to run custom build command for `thiserror v2.0.18` (or `quote`, `proc-macro2`, …)

Caused by:
  …\build-script-build` (never executed)

Caused by:
  An Application Control policy has blocked this file. (os error 4551)
```

German locale: *Eine Anwendungssteuerungsrichtlinie hat diese Datei blockiert.*

This is **not** a project bug. SAC blocks **unsigned** executables that Cargo generates under `src-tauri\target\` and `%LOCALAPPDATA%\Temp\`. No code change in this repo can bypass SAC.

Same root cause affects **desktop** and **Android** Tauri builds on Windows.

---

## What to try first (ranked)

| # | Fix | Effort | Fixes SAC 4551? | Notes |
|---|-----|--------|-----------------|-------|
| 1 | **Turn off Smart App Control** | Low | **Yes** | Most reliable on native Windows. See [Disable SAC](#1-disable-smart-app-control-recommended-on-windows). |
| 2 | **Build from WSL2 (Linux)** | High | **Yes** | SAC applies to Windows processes only. See [WSL2 Android build](#3-build-from-wsl2-linux-side). |
| 3 | **Enable Developer Mode** | Low | **Sometimes** | Worth toggling; Microsoft may disable SAC for dev machines — **not guaranteed**. |
| 4 | **Defender folder exclusions** | Low | **No** | Speeds up Rust builds and avoids *other* blocks; **does not replace** turning SAC off. See [Defender exclusions](#2-defender-exclusions-not-a-sac-bypass). |

After any change that should unblock builds, run:

```powershell
cd src-tauri
cargo clean
cd ..
npm run tauri:android:build   # or: npm run tauri:build
```

Stale blocked artifacts in `target\` can make the first retry fail even after SAC is off — **`cargo clean` is recommended**.

---

## 1. Disable Smart App Control (recommended on Windows)

**There is no per-app allowlist for SAC.** Microsoft’s FAQ: you must turn SAC off, or the app publisher must sign the binary ([Smart App Control FAQ](https://support.microsoft.com/en-us/windows/smart-app-control-frequently-asked-questions-285ea03d-fa88-4d56-882e-6698afdb7003)).

### Settings path (English UI)

1. **Settings** → **Privacy & security** → **Windows Security** → **Open Windows Security**
2. **App & browser control** → **Smart App Control settings**
3. Set to **Off**
4. **Reboot** (recommended), open a **new** PowerShell, then `cargo clean` and rebuild.

Shortcut: press **Win + R**, run `windowsdefender://smartapp/`

### Путь в параметрах (русский интерфейс)

1. **Параметры** → **Конфиденциальность и защита** → **Безопасность Windows** → **Открыть службу "Безопасность Windows"**
2. **Управление приложениями и браузером** → **Параметры Smart App Control**
3. **Отключить** (Off)
4. Перезагрузка, новый терминал, `cargo clean`, сборка заново.

### Important notes

- SAC exists only on **Windows 11** (not Windows 10).
- If SAC is **On**, unsigned `build-script-build.exe` files from Cargo will keep failing with **4551**.
- After you manually switch SAC **On/Off**, you **cannot return to Evaluation mode** without resetting/reinstalling Windows ([Microsoft docs](https://support.microsoft.com/en-us/windows/app-browser-control-in-the-windows-security-app-8f68fb65-ebb4-3cfb-4bd7-ef0f376f3dc3)). Recent Windows 11 updates allow turning SAC **back on** later without a full reinstall, but Evaluation mode is still one-way.
- If the **Smart App Control** page is missing, SAC was never enabled on this PC — look for **AppLocker / WDAC** (corporate policy) instead.

---

## 2. Defender exclusions (not a SAC bypass)

**Windows Security → Virus & threat protection → Manage settings → Exclusions → Add an exclusion → Folder**

Add these folders (adjust username):

| Folder | Why |
|--------|-----|
| `%USERPROFILE%\.cargo` | Cargo registry, `build-script-build` cache |
| `%USERPROFILE%\.rustup` | Rust toolchains |
| `C:\Users\<you>\AreYouFocused\AreYouFocused\src-tauri\target` | Project build output |

PowerShell (opens Defender exclusions UI):

```powershell
start ms-settings:windowsdefender
```

**Expectation:** fewer slow scans and fewer Defender “threat” quarantines. **SAC can still block with 4551** even when Defender excludes these paths.

---

## 3. Build from WSL2 (Linux side)

Running `cargo` / `tauri android build` **inside WSL2 Ubuntu** avoids Windows SAC because build scripts execute in Linux.

**Tauri docs:** [Prerequisites — Configure for Mobile Targets (Android)](https://v2.tauri.app/start/prerequisites/) document Android setup for **Windows, macOS, and Linux** separately. They do **not** document a dedicated “WSL2 on Windows” flow; use the **Linux** Android steps inside WSL.

### Checklist (WSL2)

1. Install **WSL2** + **Ubuntu** (`wsl --install`), update packages.
2. Clone or copy the repo into the **Linux filesystem**, e.g. `~/AreYouFocused` — **not** `/mnt/c/...` (Tauri/desktop builds on `/mnt/c` are slow and can break WiX/NSIS; keep sources under `$HOME`).
3. In WSL: **rustup**, **Node 18+**, **npm install** at repo root.
4. Install **Android SDK** in Linux (Android Studio for Linux, or `sdkmanager` + cmdline-tools). Set in `~/.bashrc`:

   ```bash
   export JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64   # or Android Studio jbr path
   export ANDROID_HOME=$HOME/Android/Sdk
   export NDK_HOME=$ANDROID_HOME/ndk/$(ls -1 $ANDROID_HOME/ndk | head -1)
   export PATH=$PATH:$ANDROID_HOME/platform-tools
   ```

5. `rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android`
6. From repo root in WSL:

   ```bash
   npm run tauri:android:init    # once
   npm run tauri:android:build
   ```

### Device / emulator from WSL

- **Emulator:** run AVD from Android Studio on **Windows**, or install emulator inside WSL (heavier).
- **Physical device over USB:** use [usbipd-win](https://learn.microsoft.com/en-us/windows/wsl/connect-usb) to attach the phone to WSL, then `adb devices`.

---

## 4. Developer Mode (optional, unreliable for SAC)

**Settings** → **Privacy & security** → **For developers** → **Developer Mode** → **On**

**Параметры** → **Конфиденциальность и защита** → **Для разработчиков** → **Режим разработчика**

Microsoft states SAC may stay off on machines configured for development, but many users still see **4551** with Developer Mode on. Treat this as a **quick try**, not a fix.

---

## 5. What does not help SAC 4551

- Reinstalling Rust alone
- `CARGO_TARGET_DIR` on another drive (build scripts are still unsigned)
- Running PowerShell as Administrator (SAC is not UAC)
- Signing your **app** — SAC blocks **Cargo’s intermediate** build scripts, not your final APK/EXE

---

## Related docs

- **Android prerequisites (Windows):** [ANDROID.md](ANDROID.md)
- **Desktop dev/build:** [README.md](README.md#development)
- **Tauri Android (official):** [Prerequisites — Android](https://v2.tauri.app/start/prerequisites/)
