# Daily Reminder Ping Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add optional daily wellness reminder pings (separate from activity/time-management pings) with preset catalog, per-reminder schedules, burst nudges, and Windows toast **Done** to cancel remaining bursts.

**Architecture:** SQLite `daily_reminders` table + settings flag; dedicated `daily_scheduler` async loop parallel to activity `scheduler`; `db/daily_reminder.rs` for scheduling math (chrono local HH:MM); Windows toasts use distinct actions (`daily_done_{id}`). Activity pings take priority — defer daily fire by 2 minutes if an activity ping is due within ±2 minutes.

**Tech Stack:** Rust (Tauri 2, rusqlite, chrono), React/TypeScript UI matching existing Schedule accordions.

---

## Brainstorming decisions (brief)

**Preset catalog:** Drink water · Eat something · Take medication (+ pill name) · Stretch or move · Take a short break · Stand up · Bathroom break · Check in with yourself · Custom

**Burst UX copy:** UI label **"Nudge until done"** — helper: "Repeat this reminder a few times until you tap Done on the notification." Defaults: **3** times, **5** minutes apart.

**Data model:** `daily_reminder_enabled` setting (default off). Table `daily_reminders` (id, label, times_json, enabled, burst_count, burst_interval_min, preset_key, pill_note). Burst state on `scheduler_state`: `daily_burst_reminder_id`, `daily_burst_remaining`, `daily_burst_next_at_unix`.

---

### Task 1: Database migration and repo module

**Files:**
- Modify: `src-tauri/src/db/migrations.rs`
- Create: `src-tauri/src/db/daily_reminder.rs`
- Modify: `src-tauri/src/db/mod.rs`

- [ ] Add `migrate_daily_reminders` (table + scheduler columns + setting default 0)
- [ ] Implement CRUD, `next_daily_fire_unix`, burst start/clear/done
- [ ] Unit tests for next-fire and burst scheduling

### Task 2: Daily scheduler loop

**Files:**
- Create: `src-tauri/src/daily_scheduler.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] `spawn_daily_reminder_loop` with poll sleep like activity scheduler
- [ ] On fire: notify + start burst chain; on burst tick: notify or clear
- [ ] Defer 2 min when activity ping conflicts

### Task 3: Windows toast integration

**Files:**
- Modify: `src-tauri/src/platform/mod.rs`, `windows.rs`, `stub.rs`

- [ ] `DailyReminderNotifier` trait + `notify_daily_reminder`
- [ ] Toast: title "Daily reminder", line = label, single **Done** button → `daily_done_{id}`

### Task 4: Tauri commands

**Files:**
- Modify: `src-tauri/src/commands.rs`, `lib.rs`

- [ ] `get_daily_reminder_settings`, `set_daily_reminder_enabled`, `list_daily_reminders`, `save_daily_reminder`, `delete_daily_reminder`

### Task 5: Schedule tab UI

**Files:**
- Modify: `src/App.tsx`

- [ ] Accordion **Daily reminder ping** (disabled by default), preset chips, time inputs, burst controls, reminder list

### Task 6: Verify and commit

- [ ] `cargo test` in src-tauri
- [ ] `npm run build`
- [ ] Commit on `feat/daily-reminder-ping`
