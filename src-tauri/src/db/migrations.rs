use crate::db::sleep_hours::{
    DEFAULT_SLEEP_ENABLED, DEFAULT_SLEEP_END, DEFAULT_SLEEP_START,
};
use rusqlite::{Connection, OptionalExtension};

const INITIAL_SQL: &str = include_str!("../../migrations/001_initial.sql");

pub fn apply_initial(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(INITIAL_SQL)?;
    migrate_scheduler_followup(conn)?;
    migrate_captures_analytics(conn)?;
    migrate_captures_finished(conn)?;
    migrate_overdue_ping_settings(conn)?;
    migrate_daily_reminders(conn)?;
    migrate_sleep_hours_settings(conn)?;
    Ok(())
}

fn setting_exists(conn: &Connection, key: &str) -> rusqlite::Result<bool> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM settings WHERE key = ?1",
            rusqlite::params![key],
            |row| row.get(0),
        )
        .optional()?;
    Ok(exists.is_some())
}

/// Overdue / random ping toggles and overdue interval bounds.
fn migrate_overdue_ping_settings(conn: &Connection) -> rusqlite::Result<()> {
    if !setting_exists(conn, "random_ping_enabled")? {
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('random_ping_enabled', '1')",
            [],
        )?;
    }
    if !setting_exists(conn, "overdue_ping_enabled")? {
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('overdue_ping_enabled', '1')",
            [],
        )?;
    }
    if !setting_exists(conn, "overdue_ping_min_minutes")? {
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('overdue_ping_min_minutes', '15')",
            [],
        )?;
    }
    if !setting_exists(conn, "overdue_ping_max_minutes")? {
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('overdue_ping_max_minutes', '30')",
            [],
        )?;
    }
    Ok(())
}

fn captures_column_exists(conn: &Connection, name: &str) -> rusqlite::Result<bool> {
    let mut stmt = conn.prepare("PRAGMA table_info(captures)")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for r in rows {
        if r? == name {
            return Ok(true);
        }
    }
    Ok(false)
}

fn scheduler_column_exists(conn: &Connection, name: &str) -> rusqlite::Result<bool> {
    let mut stmt = conn.prepare("PRAGMA table_info(scheduler_state)")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for r in rows {
        if r? == name {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Adds activity follow-up columns (see `repo::NextPingKind`).
fn migrate_scheduler_followup(conn: &Connection) -> rusqlite::Result<()> {
    if !scheduler_column_exists(conn, "next_ping_kind")? {
        conn.execute(
            "ALTER TABLE scheduler_state ADD COLUMN next_ping_kind TEXT NOT NULL DEFAULT 'standard'",
            [],
        )?;
    }
    if !scheduler_column_exists(conn, "awaiting_followup")? {
        conn.execute(
            "ALTER TABLE scheduler_state ADD COLUMN awaiting_followup INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    if !scheduler_column_exists(conn, "planned_check_subject")? {
        conn.execute(
            "ALTER TABLE scheduler_state ADD COLUMN planned_check_subject TEXT",
            [],
        )?;
    }
    Ok(())
}

/// Segment duration + stable activity key for stacked time / charts (`thread_root`).
fn migrate_captures_analytics(conn: &Connection) -> rusqlite::Result<()> {
    if !captures_column_exists(conn, "duration_minutes")? {
        conn.execute(
            "ALTER TABLE captures ADD COLUMN duration_minutes INTEGER",
            [],
        )?;
    }
    if !captures_column_exists(conn, "thread_root")? {
        conn.execute("ALTER TABLE captures ADD COLUMN thread_root TEXT", [])?;
        conn.execute(
            "UPDATE captures SET thread_root = body WHERE thread_root IS NULL",
            [],
        )?;
    }
    Ok(())
}

/// Daily wellness reminders (separate from activity pings).
fn migrate_daily_reminders(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS daily_reminders (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            label TEXT NOT NULL,
            times_json TEXT NOT NULL,
            enabled INTEGER NOT NULL DEFAULT 1,
            burst_count INTEGER NOT NULL DEFAULT 3,
            burst_interval_min INTEGER NOT NULL DEFAULT 5,
            preset_key TEXT,
            pill_note TEXT
        );",
    )?;
    if !setting_exists(conn, "daily_reminder_enabled")? {
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('daily_reminder_enabled', '0')",
            [],
        )?;
    }
    if !scheduler_column_exists(conn, "daily_burst_reminder_id")? {
        conn.execute(
            "ALTER TABLE scheduler_state ADD COLUMN daily_burst_reminder_id INTEGER",
            [],
        )?;
    }
    if !scheduler_column_exists(conn, "daily_burst_remaining")? {
        conn.execute(
            "ALTER TABLE scheduler_state ADD COLUMN daily_burst_remaining INTEGER",
            [],
        )?;
    }
    if !scheduler_column_exists(conn, "daily_burst_next_at_unix")? {
        conn.execute(
            "ALTER TABLE scheduler_state ADD COLUMN daily_burst_next_at_unix INTEGER",
            [],
        )?;
    }
    Ok(())
}

/// Quiet hours for activity + daily pings (`sleep_hours` module).
fn migrate_sleep_hours_settings(conn: &Connection) -> rusqlite::Result<()> {
    if !setting_exists(conn, "sleep_enabled")? {
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('sleep_enabled', ?1)",
            rusqlite::params![if DEFAULT_SLEEP_ENABLED { "1" } else { "0" }],
        )?;
    }
    if !setting_exists(conn, "sleep_start_hm")? {
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('sleep_start_hm', ?1)",
            rusqlite::params![DEFAULT_SLEEP_START],
        )?;
    }
    if !setting_exists(conn, "sleep_end_hm")? {
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('sleep_end_hm', ?1)",
            rusqlite::params![DEFAULT_SLEEP_END],
        )?;
    }
    Ok(())
}

/// Explicit task completion (`repo::mark_latest_timed_capture_finished`).
fn migrate_captures_finished(conn: &Connection) -> rusqlite::Result<()> {
    if !captures_column_exists(conn, "finished")? {
        conn.execute(
            "ALTER TABLE captures ADD COLUMN finished INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    Ok(())
}
