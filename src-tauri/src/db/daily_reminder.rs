//! Daily wellness reminders — fixed local times, optional burst nudges, separate from activity pings.

use crate::db::sleep_hours;
use crate::error::AppError;
use chrono::{Local, NaiveTime, TimeZone};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

pub const DEFAULT_BURST_COUNT: i64 = 3;
pub const DEFAULT_BURST_INTERVAL_MIN: i64 = 5;
pub const MIN_BURST_COUNT: i64 = 1;
pub const MAX_BURST_COUNT: i64 = 10;
pub const MIN_BURST_INTERVAL_MIN: i64 = 1;
pub const MAX_BURST_INTERVAL_MIN: i64 = 60;

/// Defer daily toast when an activity ping is due within this window (seconds).
pub const ACTIVITY_PING_CONFLICT_SECS: i64 = 120;

pub const PRESET_CATALOG: &[(&str, &str)] = &[
    ("water", "Drink water"),
    ("food", "Eat something"),
    ("pills", "Take medication"),
    ("stretch", "Stretch or move"),
    ("break", "Take a short break"),
    ("stand", "Stand up"),
    ("bathroom", "Bathroom break"),
    ("check_in", "Check in with yourself"),
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DailyReminderRow {
    pub id: i64,
    pub label: String,
    pub times: Vec<String>,
    pub enabled: bool,
    pub burst_count: i64,
    pub burst_interval_min: i64,
    pub preset_key: Option<String>,
    pub pill_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyReminderSettings {
    pub enabled: bool,
    pub reminders: Vec<DailyReminderRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDailyReminderInput {
    pub id: Option<i64>,
    pub label: String,
    pub times: Vec<String>,
    pub enabled: bool,
    pub burst_count: i64,
    pub burst_interval_min: i64,
    pub preset_key: Option<String>,
    pub pill_note: Option<String>,
}

pub fn daily_reminder_enabled(conn: &Connection) -> rusqlite::Result<bool> {
    let v: String = conn.query_row(
        "SELECT value FROM settings WHERE key = 'daily_reminder_enabled'",
        [],
        |row| row.get(0),
    )?;
    Ok(v == "1")
}

pub fn set_daily_reminder_enabled(conn: &Connection, enabled: bool) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES ('daily_reminder_enabled', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![if enabled { "1" } else { "0" }],
    )?;
    Ok(())
}

pub fn list_daily_reminders(conn: &Connection) -> rusqlite::Result<Vec<DailyReminderRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, label, times_json, enabled, burst_count, burst_interval_min, preset_key, pill_note
         FROM daily_reminders ORDER BY id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        let times_json: String = row.get(2)?;
        let times: Vec<String> = serde_json::from_str(&times_json).unwrap_or_default();
        Ok(DailyReminderRow {
            id: row.get(0)?,
            label: row.get(1)?,
            times,
            enabled: row.get::<_, i64>(3)? != 0,
            burst_count: row.get(4)?,
            burst_interval_min: row.get(5)?,
            preset_key: row.get(6)?,
            pill_note: row.get(7)?,
        })
    })?;
    rows.collect()
}

pub fn read_daily_reminder_settings(conn: &Connection) -> rusqlite::Result<DailyReminderSettings> {
    Ok(DailyReminderSettings {
        enabled: daily_reminder_enabled(conn)?,
        reminders: list_daily_reminders(conn)?,
    })
}

pub fn validate_times(times: &[String]) -> Result<Vec<String>, AppError> {
    if times.is_empty() {
        return Err(AppError::InvalidDailyReminder(
            "add at least one time".into(),
        ));
    }
    let mut normalized = Vec::with_capacity(times.len());
    for t in times {
        let n = normalize_hhmm(t)?;
        if normalized.iter().any(|x| x == &n) {
            continue;
        }
        normalized.push(n);
    }
    if normalized.is_empty() {
        return Err(AppError::InvalidDailyReminder(
            "add at least one valid time (HH:MM)".into(),
        ));
    }
    normalized.sort();
    Ok(normalized)
}

pub fn validate_burst(count: i64, interval_min: i64) -> Result<(), AppError> {
    if !(MIN_BURST_COUNT..=MAX_BURST_COUNT).contains(&count) {
        return Err(AppError::InvalidDailyReminder(format!(
            "nudge count must be {MIN_BURST_COUNT}–{MAX_BURST_COUNT}"
        )));
    }
    if !(MIN_BURST_INTERVAL_MIN..=MAX_BURST_INTERVAL_MIN).contains(&interval_min) {
        return Err(AppError::InvalidDailyReminder(format!(
            "nudge interval must be {MIN_BURST_INTERVAL_MIN}–{MAX_BURST_INTERVAL_MIN} minutes"
        )));
    }
    Ok(())
}

pub fn save_daily_reminder(conn: &Connection, input: &SaveDailyReminderInput) -> Result<DailyReminderRow, AppError> {
    let label = input.label.trim();
    if label.is_empty() {
        return Err(AppError::InvalidDailyReminder("label is required".into()));
    }
    let times = validate_times(&input.times)?;
    validate_burst(input.burst_count, input.burst_interval_min)?;
    let times_json = serde_json::to_string(&times).map_err(|e| AppError::InvalidDailyReminder(e.to_string()))?;
    let enabled = if input.enabled { 1 } else { 0 };

    if let Some(id) = input.id {
        let updated = conn.execute(
            "UPDATE daily_reminders SET label = ?1, times_json = ?2, enabled = ?3,
             burst_count = ?4, burst_interval_min = ?5, preset_key = ?6, pill_note = ?7
             WHERE id = ?8",
            params![
                label,
                times_json,
                enabled,
                input.burst_count,
                input.burst_interval_min,
                input.preset_key,
                input.pill_note,
                id,
            ],
        )?;
        if updated == 0 {
            return Err(AppError::InvalidDailyReminder("reminder not found".into()));
        }
        get_daily_reminder_by_id(conn, id)?.ok_or_else(|| {
            AppError::InvalidDailyReminder("reminder not found after save".into())
        })
    } else {
        conn.execute(
            "INSERT INTO daily_reminders (label, times_json, enabled, burst_count, burst_interval_min, preset_key, pill_note)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                label,
                times_json,
                enabled,
                input.burst_count,
                input.burst_interval_min,
                input.preset_key,
                input.pill_note,
            ],
        )?;
        let id = conn.last_insert_rowid();
        get_daily_reminder_by_id(conn, id)?.ok_or_else(|| {
            AppError::InvalidDailyReminder("reminder not found after insert".into())
        })
    }
}

pub fn delete_daily_reminder(conn: &Connection, id: i64) -> Result<(), AppError> {
    clear_burst_if_reminder(conn, id)?;
    let n = conn.execute("DELETE FROM daily_reminders WHERE id = ?1", params![id])?;
    if n == 0 {
        return Err(AppError::InvalidDailyReminder("reminder not found".into()));
    }
    Ok(())
}

pub fn get_daily_reminder_by_id(
    conn: &Connection,
    id: i64,
) -> rusqlite::Result<Option<DailyReminderRow>> {
    conn.query_row(
        "SELECT id, label, times_json, enabled, burst_count, burst_interval_min, preset_key, pill_note
         FROM daily_reminders WHERE id = ?1",
        params![id],
        |row| {
            let times_json: String = row.get(2)?;
            let times: Vec<String> = serde_json::from_str(&times_json).unwrap_or_default();
            Ok(DailyReminderRow {
                id: row.get(0)?,
                label: row.get(1)?,
                times,
                enabled: row.get::<_, i64>(3)? != 0,
                burst_count: row.get(4)?,
                burst_interval_min: row.get(5)?,
                preset_key: row.get(6)?,
                pill_note: row.get(7)?,
            })
        },
    )
    .optional()
}

pub fn display_label(row: &DailyReminderRow) -> String {
    if row.preset_key.as_deref() == Some("pills") {
        if let Some(note) = row.pill_note.as_ref().filter(|s| !s.trim().is_empty()) {
            return format!("{} ({})", row.label.trim(), note.trim());
        }
    }
    row.label.trim().to_string()
}

fn normalize_hhmm(raw: &str) -> Result<String, AppError> {
    let t = raw.trim();
    let parsed = NaiveTime::parse_from_str(t, "%H:%M")
        .or_else(|_| NaiveTime::parse_from_str(t, "%H:%M:%S"))
        .map_err(|_| AppError::InvalidDailyReminder(format!("invalid time: {t}")))?;
    Ok(parsed.format("%H:%M").to_string())
}

/// Unix instant for `hh:mm` on the local calendar day containing `anchor`, or next day if not after anchor.
fn unix_for_local_hhmm_on_day(anchor_unix: i64, hhmm: &str) -> Option<i64> {
    let anchor = Local.timestamp_opt(anchor_unix, 0).single()?;
    let date = anchor.date_naive();
    let time = NaiveTime::parse_from_str(hhmm, "%H:%M").ok()?;
    let dt = date.and_time(time);
    let local_dt = Local.from_local_datetime(&dt).single()?;
    let mut unix = local_dt.timestamp();
    if unix <= anchor_unix {
        let next = date.succ_opt()?.and_time(time);
        unix = Local.from_local_datetime(&next).single()?.timestamp();
    }
    Some(unix)
}

fn defer_daily_fire(conn: &Connection, unix: i64) -> Result<i64, AppError> {
    sleep_hours::defer_unix_outside_sleep_conn(conn, unix)
}

/// Next fire: active burst continuation, else earliest enabled reminder slot after `now`.
pub fn next_daily_fire_unix(conn: &Connection, now_unix: i64) -> Result<Option<i64>, AppError> {
    if !daily_reminder_enabled(conn)? {
        return Ok(None);
    }

    if let Some(burst_at) = get_burst_next_at(conn)? {
        if burst_at > now_unix {
            return Ok(Some(defer_daily_fire(conn, burst_at)?));
        }
        if burst_at <= now_unix {
            return Ok(Some(defer_daily_fire(conn, now_unix)?));
        }
    }

    let reminders = list_daily_reminders(conn)?;
    let mut next: Option<i64> = None;
    for r in reminders {
        if !r.enabled {
            continue;
        }
        for t in &r.times {
            if let Some(u) = unix_for_local_hhmm_on_day(now_unix, t) {
                if u > now_unix {
                    next = Some(next.map(|n| n.min(u)).unwrap_or(u));
                }
            }
        }
    }
    match next {
        Some(u) => Ok(Some(defer_daily_fire(conn, u)?)),
        None => Ok(None),
    }
}

pub struct DueDailyReminder {
    pub row: DailyReminderRow,
    pub is_burst: bool,
}

/// What to show now: burst tick for stored id, or a scheduled slot (earliest matching).
pub fn due_daily_reminder(conn: &Connection, now_unix: i64) -> Result<Option<DueDailyReminder>, AppError> {
    if !daily_reminder_enabled(conn)? {
        return Ok(None);
    }
    let sleep = sleep_hours::read_sleep_hours_settings(conn)?;
    if sleep.enabled
        && sleep_hours::is_in_sleep_window_at_unix(now_unix, &sleep.start_hm, &sleep.end_hm)?
    {
        return Ok(None);
    }

    if let (Some(id), Some(burst_at)) = (get_burst_reminder_id(conn)?, get_burst_next_at(conn)?) {
        if burst_at <= now_unix {
            if let Some(row) = get_daily_reminder_by_id(conn, id)? {
                if row.enabled {
                    return Ok(Some(DueDailyReminder {
                        row,
                        is_burst: true,
                    }));
                }
            }
            clear_burst(conn)?;
        }
    }

    let reminders = list_daily_reminders(conn)?;
    let mut best: Option<(i64, DailyReminderRow)> = None;
    for r in reminders {
        if !r.enabled {
            continue;
        }
        for t in &r.times {
            if let Some(slot) = unix_for_local_hhmm_on_day(now_unix.saturating_sub(90), t) {
                if slot <= now_unix && slot > now_unix.saturating_sub(90) {
                    let replace = best
                        .as_ref()
                        .map(|(u, _)| slot > *u)
                        .unwrap_or(true);
                    if replace {
                        best = Some((slot, r.clone()));
                    }
                }
            }
        }
    }
    Ok(best.map(|(_, row)| DueDailyReminder {
        row,
        is_burst: false,
    }))
}

pub fn start_burst_after_fire(conn: &Connection, reminder_id: i64, now_unix: i64) -> rusqlite::Result<()> {
    let Some(row) = get_daily_reminder_by_id(conn, reminder_id)? else {
        return Ok(());
    };
    if row.burst_count <= 1 {
        clear_burst(conn)?;
        return Ok(());
    }
    let remaining = row.burst_count - 1;
    let next_at = now_unix + row.burst_interval_min.saturating_mul(60);
    conn.execute(
        "UPDATE scheduler_state SET daily_burst_reminder_id = ?1, daily_burst_remaining = ?2,
         daily_burst_next_at_unix = ?3 WHERE id = 1",
        params![reminder_id, remaining, next_at],
    )?;
    Ok(())
}

pub fn advance_burst_after_fire(conn: &Connection, reminder_id: i64, now_unix: i64) -> rusqlite::Result<()> {
    let remaining = get_burst_remaining(conn)?.unwrap_or(0);
    if remaining <= 1 {
        clear_burst(conn)?;
        return Ok(());
    }
    let Some(row) = get_daily_reminder_by_id(conn, reminder_id)? else {
        clear_burst(conn)?;
        return Ok(());
    };
    let next_remaining = remaining - 1;
    let next_at = now_unix + row.burst_interval_min.saturating_mul(60);
    conn.execute(
        "UPDATE scheduler_state SET daily_burst_remaining = ?1, daily_burst_next_at_unix = ?2 WHERE id = 1",
        params![next_remaining, next_at],
    )?;
    Ok(())
}

pub fn daily_done_from_toast(conn: &Connection, reminder_id: i64) -> rusqlite::Result<()> {
    clear_burst_if_reminder(conn, reminder_id)
}

fn clear_burst_if_reminder(conn: &Connection, reminder_id: i64) -> rusqlite::Result<()> {
    if get_burst_reminder_id(conn)? == Some(reminder_id) {
        clear_burst(conn)?;
    }
    Ok(())
}

pub fn clear_burst(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE scheduler_state SET daily_burst_reminder_id = NULL, daily_burst_remaining = NULL,
         daily_burst_next_at_unix = NULL WHERE id = 1",
        [],
    )?;
    Ok(())
}

fn get_burst_reminder_id(conn: &Connection) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT daily_burst_reminder_id FROM scheduler_state WHERE id = 1",
        [],
        |row| row.get::<_, Option<i64>>(0),
    )
}

fn get_burst_remaining(conn: &Connection) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT daily_burst_remaining FROM scheduler_state WHERE id = 1",
        [],
        |row| row.get::<_, Option<i64>>(0),
    )
}

fn get_burst_next_at(conn: &Connection) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT daily_burst_next_at_unix FROM scheduler_state WHERE id = 1",
        [],
        |row| row.get::<_, Option<i64>>(0),
    )
}

/// Activity ping due soon — defer daily reminder to avoid stacking.
pub fn defer_unix_if_activity_conflict(
    _conn: &Connection,
    proposed_unix: i64,
    activity_next: Option<i64>,
) -> i64 {
    let Some(act) = activity_next else {
        return proposed_unix;
    };
    let delta = (act - proposed_unix).abs();
    if delta <= ACTIVITY_PING_CONFLICT_SECS {
        proposed_unix + ACTIVITY_PING_CONFLICT_SECS
    } else {
        proposed_unix
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_memory;

    #[test]
    fn normalize_hhmm_accepts_standard() {
        assert_eq!(normalize_hhmm("9:05").unwrap(), "09:05");
    }

    #[test]
    fn next_fire_finds_today_slot() {
        let conn = open_memory().unwrap();
        let now = Local::now().timestamp();
        save_daily_reminder(
            &conn,
            &SaveDailyReminderInput {
                id: None,
                label: "Water".into(),
                times: vec!["23:59".into()],
                enabled: true,
                burst_count: 1,
                burst_interval_min: 5,
                preset_key: Some("water".into()),
                pill_note: None,
            },
        )
        .unwrap();
        set_daily_reminder_enabled(&conn, true).unwrap();
        let next = next_daily_fire_unix(&conn, now).unwrap();
        assert!(next.is_some());
    }

    #[test]
    fn burst_cleared_on_done() {
        let conn = open_memory().unwrap();
        let row = save_daily_reminder(
            &conn,
            &SaveDailyReminderInput {
                id: None,
                label: "Pills".into(),
                times: vec!["08:00".into()],
                enabled: true,
                burst_count: 3,
                burst_interval_min: 5,
                preset_key: Some("pills".into()),
                pill_note: Some("vitamin D".into()),
            },
        )
        .unwrap();
        start_burst_after_fire(&conn, row.id, 1000).unwrap();
        daily_done_from_toast(&conn, row.id).unwrap();
        assert!(get_burst_next_at(&conn).unwrap().is_none());
    }
}
