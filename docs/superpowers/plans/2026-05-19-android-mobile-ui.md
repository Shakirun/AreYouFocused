# Android Build & Mobile UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan step-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Tauri 2 Android build configuration and a mobile-friendly bottom-tab shell while preserving the Windows desktop experience.

**Architecture:** Platform-specific Tauri config (`tauri.android.conf.json`), desktop-only Rust window/tray behavior, responsive React shell with bottom nav on narrow/coarse viewports, stub notifications on Android documented in `ANDROID.md`.

**Tech Stack:** Tauri 2, React 18, Tailwind CSS, Rust (existing platform module)

---

### Task 1: Android Tauri configuration

**Files:**
- Create: `src-tauri/tauri.android.conf.json`
- Create: `ANDROID.md`
- Modify: `package.json`

- [ ] **Step 1:** Add `tauri.android.conf.json` with mobile window defaults and `bundle.android.minSdkVersion: 24`
- [ ] **Step 2:** Add npm scripts: `tauri:android:init`, `tauri:android:dev`, `tauri:android:build`
- [ ] **Step 3:** Document prerequisites (Android Studio, SDK, NDK, JAVA_HOME, ANDROID_HOME, rustup targets) and init command in `ANDROID.md`

### Task 2: Rust mobile guards

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/platform/stub.rs`

- [ ] **Step 1:** Gate `set_resizable(false)` and close-to-hide window handler with `#[cfg(desktop)]`
- [ ] **Step 2:** Clarify stub notifier is Android MVP path (log + in-app events)

### Task 3: Mobile layout hook and bottom nav

**Files:**
- Create: `src/useMobileLayout.ts`
- Create: `src/MobileBottomNav.tsx`
- Modify: `src/App.tsx`
- Modify: `src/index.css`
- Modify: `src/useFitWindowHeight.ts`

- [ ] **Step 1:** `useMobileLayout` — matchMedia `(max-width: 767px)` + coarse pointer
- [ ] **Step 2:** `MobileBottomNav` — 4 tabs, inline SVG icons, min-h 44px, safe-area padding
- [ ] **Step 3:** App shell — bottom nav on mobile, top tabs on desktop; full `dvh` on mobile; skip fit-window on mobile
- [ ] **Step 4:** CSS safe-area + mobile shell; schedule copy without tray reference on mobile

### Task 4: Verify and ship

- [ ] **Step 1:** `npm run build`
- [ ] **Step 2:** `cargo check` in `src-tauri`
- [ ] **Step 3:** Commit on `feat/android-mobile`, push, open PR to `develop`
