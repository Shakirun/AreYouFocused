# Sleeping Hours + Time Picker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans or subagent-driven-development to implement this plan task-by-task.

**Goal:** Add quiet “sleeping hours” (default 22:00–08:00) that defer activity and daily pings, and replace the awkward native dual-column time control with preset chips plus hour/minute dropdowns.

**Architecture:** Rust `sleep_hours` module owns window math and SQLite settings; `repo::set_next_ping_at_unix` applies deferral on every persisted ping; schedulers skip firing while `is_now_in_sleep`. React `TimePickerField` uses accessible `<select>` controls and preset pills matching the teal schedule UI.

**Tech Stack:** Tauri 2, Rust (chrono, rusqlite), React/TypeScript, Tailwind.

---

## Brainstorm decisions

| Topic | Decision |
|-------|----------|
| Daily reminders during sleep | **Deferred** with activity pings (full quiet hours) |
| Timezone | System local TZ (matches daily reminders) |
| Window semantics | `[start, end)` — e.g. 22:00 asleep, 08:00 awake |
| Time picker UX | Preset chips + hour/minute `<select>` (not OS scroll wheels) |

---

## Task 1: Backend sleep hours

**Files:**
- Create: `src-tauri/src/db/sleep_hours.rs`
- Modify: `src-tauri/src/db/migrations.rs`, `mod.rs`, `repo.rs`, `scheduler.rs`, `daily_reminder.rs`, `commands.rs`, `lib.rs`, `error.rs`

- [x] Settings: `sleep_enabled`, `sleep_start_hm`, `sleep_end_hm` (defaults 0, 22:00, 08:00)
- [x] `defer_unix_outside_sleep`, `is_in_sleep_window_at_unix`, tests
- [x] Integrate into ping scheduling and daily reminder next/due
- [x] IPC: `get_sleep_hours_settings`, `save_sleep_hours_settings`

---

## Task 2: Time picker component

**Files:**
- Create: `src/TimePickerField.tsx`
- Modify: `src/App.tsx` (daily reminder times, sleep start/end)

- [x] Presets for daily, sleep start, sleep end
- [x] Full hour/minute dropdowns, keyboard-friendly

---

## Task 3: Schedule UI

**Files:**
- Modify: `src/App.tsx`

- [x] “Sleeping hours” accordion on Schedule tab
- [x] Apply button persists settings and refreshes scheduler line

---

## Task 4: Verification

- [x] `cargo test` in `src-tauri`
- [ ] `npm run build`
- [ ] Commit on `feat/sleep-hours-time-picker`
