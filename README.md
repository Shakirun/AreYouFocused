# AreYouFocused

Desktop-first **random-ping** productivity tracker (WhatNow-style): honest, local-first, SQLite. Product charter and Cursor rules live under **`.cursor/`** locally (not in git).

**Language policy:** All **public repo docs**, **in-app UI copy**, and **code comments** are **English** by default. Planned **i18n** for additional locales is described in [I18N.md](I18N.md).

## Development

- **TDD only**: red → green → refactor; failing test before implementation (local rule pack under `.cursor/rules/`).
- **Frontend:** Node 18+, `npm install`, `npm run dev` (Vite + React + **Tailwind**). Design tokens follow the ui-ux-pro-max design system (see `tailwind.config.js`).
- **Desktop (Tauri 2):** install **Rust** (stable via [rustup](https://rustup.rs/)), on Windows also **MSVC Build Tools**. On Windows the UI needs **WebView2** — see **Runtime (Windows desktop)** below if the window fails to open.
  - `npm run tauri:dev` — run the app with the Vite dev server (includes a **background scheduler** that waits for `next_ping_at`, shows a ping notification, then rolls the next random interval).
  - **Faster pings for local QA:** set env **`AREYOUFOCUSED_DEV_PING_SECS`** to a positive integer (seconds). The scheduler then sleeps that long between pings and updates `next_ping_at` deterministically. **Do not use in production.** Example (PowerShell): `$env:AREYOUFOCUSED_DEV_PING_SECS = "15"; npm run tauri:dev`
  - The UI loads **next ping** and interval bounds from Rust (`get_scheduler_status`), listens for **`ping-due`** (after each OS notification) to refresh, and shows “Last ping at …” plus the upcoming ping time. Use the in-app **Ping interval** section to change min/max minutes (`update_ping_interval`); the next ping is re-rolled from the current time when you apply new bounds.
  - `npm run tauri:build` — production build (bundling is currently off in `src-tauri/tauri.conf.json` until icons and installer work are done).
  - Rust: `cd src-tauri` then `cargo test` / `cargo check`. Unit tests live in the **library** crate; the `are-you-focused` binary is only a thin `main` shim, so its `[[bin]]` has `test = false` and Cargo will not print a second “0 tests” harness for it.
- **Cursor + 21st.dev:** after edits to `src/App.tsx`, an optional hook injects a follow-up to run `21st_magic_component_builder` — see `devtools/cursor/README.md` (`.cursor/` is local-only).

## Runtime (Windows desktop)

- **OS:** Windows 10 or later (64-bit; same as your Tauri target triple).
- **WebView2:** the UI runs inside the **Microsoft Edge WebView2** control. The **Evergreen** runtime ships with current Windows 10/11 updates for most users. If the window is blank or the app exits on startup, install or repair the runtime from [WebView2 Runtime — consumer download](https://developer.microsoft.com/microsoft-edge/webview2/consumer/) or the overview at [WebView2](https://developer.microsoft.com/microsoft-edge/webview2/).
- **Installers:** for packaged apps, Microsoft documents the **Evergreen Bootstrapper** and fixed-version layouts for offline or locked-down machines; Tauri’s Windows bundler can pull the bootstrapper by default — see [Distribute your app and the WebView2 Runtime](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution) when you turn bundling back on in `tauri.conf.json`.

## Git branches

Integrate on **`develop`** (unstable `*-dev.*` versions); ship stable releases via MR **`develop` → `main`**. Details: [RELEASING.md](RELEASING.md).

## Agent / Cursor (local workspace only)

Handbook (`AGENTS.md`), `docs/`, and `.cursor/` are **gitignored** — keep them on your machine; they are not part of the shared repo. Copy `.cursor/mcp.json.example` → `.cursor/mcp.json` for MCP when needed.
