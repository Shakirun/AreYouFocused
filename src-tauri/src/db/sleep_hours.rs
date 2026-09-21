//! Sleeping hours: defer activity and daily pings outside the quiet window (system TZ).

use crate::error::AppError;
use chrono::{Local, NaiveTime, TimeZone, Timelike};
use rusqlite::{params, Connection};

pub const DEFAULT_SLEEP_START: &str = "22:00";
pub const DEFAULT_SLEEP_END: &str = "06:00";
pub const DEFAULT_SLEEP_ENABLED: bool = true;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SleepHoursSettings {
    pub enabled: bool,
    pub start_hm: String,
    pub end_hm: String,
}

fn setting_str(conn: &Connection, key: &str) -> rusqlite::Result<String> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
}

fn set_setting_str(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE settings SET value = ?1 WHERE key = ?2",
        params![value, key],
    )?;
    Ok(())
}

fn setting_bool(conn: &Connection, key: &str) -> rusqlite::Result<bool> {
    let v = setting_str(conn, key)?;
    Ok(v.trim() == "1" || v.eq_ignore_ascii_case("true"))
}

fn set_setting_bool(conn: &Connection, key: &str, enabled: bool) -> rusqlite::Result<()> {
    set_setting_str(conn, key, if enabled { "1" } else { "0" })
}

pub fn read_sleep_hours_settings(conn: &Connection) -> rusqlite::Result<SleepHoursSettings> {
    Ok(SleepHoursSettings {
        enabled: setting_bool(conn, "sleep_enabled")?,
        start_hm: setting_str(conn, "sleep_start_hm")?,
        end_hm: setting_str(conn, "sleep_end_hm")?,
    })
}

pub fn save_sleep_hours_settings(
    conn: &Connection,
    enabled: bool,
    start_hm: &str,
    end_hm: &str,
) -> Result<SleepHoursSettings, AppError> {
    let start_hm = normalize_hhmm(start_hm)?;
    let end_hm = normalize_hhmm(end_hm)?;
    set_setting_bool(conn, "sleep_enabled", enabled)?;
    set_setting_str(conn, "sleep_start_hm", &start_hm)?;
    set_setting_str(conn, "sleep_end_hm", &end_hm)?;
    Ok(SleepHoursSettings {
        enabled,
        start_hm,
        end_hm,
    })
}

pub fn normalize_hhmm(raw: &str) -> Result<String, AppError> {
    let t = raw.trim();
    let parsed = NaiveTime::parse_from_str(t, "%H:%M")
        .or_else(|_| NaiveTime::parse_from_str(t, "%H:%M:%S"))
        .map_err(|_| AppError::InvalidSleepHours(format!("invalid time: {t}")))?;
    Ok(parsed.format("%H:%M").to_string())
}

fn minutes_since_midnight(h: u32, m: u32) -> u32 {
    h * 60 + m
}

fn parse_hm(hm: &str) -> Result<(u32, u32), AppError> {
    let (h, m) = {
        let n = normalize_hhmm(hm)?;
        let parts: Vec<_> = n.split(':').collect();
        (
            parts[0].parse::<u32>().unwrap_or(0),
            parts[1].parse::<u32>().unwrap_or(0),
        )
    };
    Ok((h, m))
}

/// Sleep window `[start, end)` — `end` is the first minute **awake** (e.g. 06:00).
pub fn is_in_sleep_window_at_unix(
    unix: i64,
    start_hm: &str,
    end_hm: &str,
) -> Result<bool, AppError> {
    let local = Local
        .timestamp_opt(unix, 0)
        .single()
        .ok_or_else(|| AppError::InvalidSleepHours("invalid timestamp".into()))?;
    let now_m = minutes_since_midnight(local.hour(), local.minute());
    let (sh, sm) = parse_hm(start_hm)?;
    let (eh, em) = parse_hm(end_hm)?;
    let start_m = minutes_since_midnight(sh, sm);
    let end_m = minutes_since_midnight(eh, em);

    if start_m == end_m {
        return Ok(false);
    }
    if start_m < end_m {
        Ok(now_m >= start_m && now_m < end_m)
    } else {
        Ok(now_m >= start_m || now_m < end_m)
    }
}

/// Earliest local `end_hm` instant at or after `from_unix`.
pub fn wake_unix_on_or_after(from_unix: i64, end_hm: &str) -> Result<i64, AppError> {
    let anchor = Local
        .timestamp_opt(from_unix, 0)
        .single()
        .ok_or_else(|| AppError::InvalidSleepHours("invalid timestamp".into()))?;
    let (eh, em) = parse_hm(end_hm)?;
    let end_time = NaiveTime::from_hms_opt(eh, em, 0)
        .ok_or_else(|| AppError::InvalidSleepHours("invalid end time".into()))?;

    let mut date = anchor.date_naive();
    for _ in 0..3 {
        let dt = date.and_time(end_time);
        if let Some(local_dt) = Local.from_local_datetime(&dt).single() {
            let unix = local_dt.timestamp();
            if unix >= from_unix {
                return Ok(unix);
            }
        }
        date = date
            .succ_opt()
            .ok_or_else(|| AppError::InvalidSleepHours("date overflow".into()))?;
    }
    Err(AppError::InvalidSleepHours(
        "could not resolve wake time".into(),
    ))
}

/// If `proposed_unix` falls inside the sleep window, move to the next wake (`end_hm`).
pub fn defer_unix_outside_sleep(
    proposed_unix: i64,
    enabled: bool,
    start_hm: &str,
    end_hm: &str,
) -> Result<i64, AppError> {
    if !enabled {
        return Ok(proposed_unix);
    }
    if !is_in_sleep_window_at_unix(proposed_unix, start_hm, end_hm)? {
        return Ok(proposed_unix);
    }
    wake_unix_on_or_after(proposed_unix, end_hm)
}

pub fn defer_unix_outside_sleep_conn(conn: &Connection, proposed_unix: i64) -> Result<i64, AppError> {
    let s = read_sleep_hours_settings(conn)?;
    defer_unix_outside_sleep(
        proposed_unix,
        s.enabled,
        &s.start_hm,
        &s.end_hm,
    )
}

pub fn is_now_in_sleep(conn: &Connection) -> Result<bool, AppError> {
    let s = read_sleep_hours_settings(conn)?;
    if !s.enabled {
        return Ok(false);
    }
    let now = Local::now().timestamp();
    is_in_sleep_window_at_unix(now, &s.start_hm, &s.end_hm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_memory;

    fn seed(conn: &Connection) {
        save_sleep_hours_settings(conn, true, "22:00", "08:00").unwrap();
    }

    fn local_unix(h: u32, m: u32) -> i64 {
        let now = Local::now();
        let date = now.date_naive();
        let t = NaiveTime::from_hms_opt(h, m, 0).unwrap();
        Local
            .from_local_datetime(&date.and_time(t))
            .single()
            .unwrap()
            .timestamp()
    }

    #[test]
    fn fresh_db_sleep_hours_defaults_enabled_22_to_06() {
        let conn = open_memory().unwrap();
        let s = read_sleep_hours_settings(&conn).unwrap();
        assert_eq!(s.enabled, DEFAULT_SLEEP_ENABLED);
        assert_eq!(s.start_hm, DEFAULT_SLEEP_START);
        assert_eq!(s.end_hm, DEFAULT_SLEEP_END);
    }

    #[test]
    fn explicit_sleep_settings_are_preserved() {
        let conn = open_memory().unwrap();
        save_sleep_hours_settings(&conn, false, "21:00", "07:00").unwrap();
        let s = read_sleep_hours_settings(&conn).unwrap();
        assert!(!s.enabled);
        assert_eq!(s.start_hm, "21:00");
        assert_eq!(s.end_hm, "07:00");
    }

    #[test]
    fn midnight_crossing_detects_night_and_morning() {
        let conn = open_memory().unwrap();
        seed(&conn);
        let s = read_sleep_hours_settings(&conn).unwrap();
        assert!(is_in_sleep_window_at_unix(local_unix(23, 0), &s.start_hm, &s.end_hm).unwrap());
        assert!(is_in_sleep_window_at_unix(local_unix(7, 30), &s.start_hm, &s.end_hm).unwrap());
        assert!(!is_in_sleep_window_at_unix(local_unix(12, 0), &s.start_hm, &s.end_hm).unwrap());
        assert!(!is_in_sleep_window_at_unix(local_unix(8, 0), &s.start_hm, &s.end_hm).unwrap());
    }

    #[test]
    fn defer_moves_night_ping_to_morning_wake() {
        let conn = open_memory().unwrap();
        seed(&conn);
        let s = read_sleep_hours_settings(&conn).unwrap();
        let night = local_unix(23, 15);
        let wake = defer_unix_outside_sleep(night, s.enabled, &s.start_hm, &s.end_hm).unwrap();
        let wake_local = Local.timestamp_opt(wake, 0).single().unwrap();
        assert_eq!(wake_local.hour(), 8);
        assert_eq!(wake_local.minute(), 0);
        assert!(wake > night);
    }

    #[test]
    fn disabled_sleep_does_not_defer() {
        let conn = open_memory().unwrap();
        save_sleep_hours_settings(&conn, false, "22:00", "08:00").unwrap();
        let night = local_unix(23, 0);
        assert_eq!(
            defer_unix_outside_sleep_conn(&conn, night).unwrap(),
            night
        );
    }
}
