//! Data access helpers.

use crate::domain::ping_plan;
use crate::error::AppError;
use rand::Rng;
use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NextPingKind {
    Standard,
    PlannedCheck,
    Overdue,
}

impl NextPingKind {
    pub fn from_db(s: &str) -> Self {
        match s.trim() {
            "planned_check" => NextPingKind::PlannedCheck,
            "overdue" => NextPingKind::Overdue,
            _ => NextPingKind::Standard,
        }
    }

    pub fn as_db(self) -> &'static str {
        match self {
            NextPingKind::Standard => "standard",
            NextPingKind::PlannedCheck => "planned_check",
            NextPingKind::Overdue => "overdue",
        }
    }
}

pub fn get_next_ping_kind(conn: &Connection) -> rusqlite::Result<NextPingKind> {
    let s: String = conn.query_row(
        "SELECT COALESCE(next_ping_kind, 'standard') FROM scheduler_state WHERE id = 1",
        [],
        |row| row.get(0),
    )?;
    Ok(NextPingKind::from_db(&s))
}

fn set_next_ping_kind(conn: &Connection, kind: NextPingKind) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE scheduler_state SET next_ping_kind = ?1 WHERE id = 1",
        params![kind.as_db()],
    )?;
    Ok(())
}

pub fn get_awaiting_followup(conn: &Connection) -> rusqlite::Result<bool> {
    let v: i64 = conn.query_row(
        "SELECT COALESCE(awaiting_followup, 0) FROM scheduler_state WHERE id = 1",
        [],
        |row| row.get(0),
    )?;
    Ok(v != 0)
}

fn set_awaiting_followup(conn: &Connection, awaiting: bool) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE scheduler_state SET awaiting_followup = ?1 WHERE id = 1",
        params![if awaiting { 1 } else { 0 }],
    )?;
    Ok(())
}

pub fn get_planned_check_subject(conn: &Connection) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT planned_check_subject FROM scheduler_state WHERE id = 1",
        [],
        |row| row.get::<_, Option<String>>(0),
    )
}

fn set_planned_check_subject(conn: &Connection, subject: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE scheduler_state SET planned_check_subject = ?1 WHERE id = 1",
        params![subject],
    )?;
    Ok(())
}

fn clear_planned_check_subject(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE scheduler_state SET planned_check_subject = NULL WHERE id = 1",
        [],
    )?;
    Ok(())
}

pub fn validate_planned_duration_minutes(m: i64) -> Result<(), AppError> {
    if m < 1 || m > PING_MAX_MINUTES_CAP {
        return Err(AppError::InvalidPlannedDuration);
    }
    Ok(())
}

/// True when a planned-check ping is still scheduled in the future.
pub fn has_future_planned_ping(conn: &Connection, now_unix: i64) -> rusqlite::Result<bool> {
    if get_next_ping_kind(conn)? != NextPingKind::PlannedCheck {
        return Ok(false);
    }
    Ok(get_next_ping_at_unix(conn)?
        .map(|t| t > now_unix)
        .unwrap_or(false))
}

/// Latest unfinished timed capture `(started_at_unix, duration_minutes)`.
fn latest_unfinished_timed_capture(
    conn: &Connection,
) -> rusqlite::Result<Option<(i64, i64)>> {
    conn.query_row(
        "SELECT created_at_unix, duration_minutes FROM captures
         WHERE duration_minutes IS NOT NULL AND COALESCE(finished, 0) = 0
         ORDER BY created_at_unix DESC LIMIT 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()
}

/// Planned end unix for the active unfinished timed capture, if any.
pub fn unfinished_timed_planned_end_unix(conn: &Connection) -> rusqlite::Result<Option<i64>> {
    Ok(latest_unfinished_timed_capture(conn)?
        .map(|(started, dur)| started.saturating_add(dur.saturating_mul(60))))
}

/// True when a timed task's planned end is in the past and it is not marked done.
pub fn is_timed_capture_overdue(conn: &Connection, now_unix: i64) -> rusqlite::Result<bool> {
    Ok(unfinished_timed_planned_end_unix(conn)?
        .map(|end| end <= now_unix)
        .unwrap_or(false))
}

/// Overdue follow-up pings apply when the planned end passed and the task is still open.
pub fn should_use_overdue_schedule(conn: &Connection, now_unix: i64) -> rusqlite::Result<bool> {
    if !overdue_ping_enabled(conn)? {
        return Ok(false);
    }
    Ok(is_timed_capture_overdue(conn, now_unix)?)
}

/// Latest timed capture row that has not been explicitly marked done.
pub fn has_unfinished_timed_capture(conn: &Connection) -> rusqlite::Result<bool> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM captures
             WHERE duration_minutes IS NOT NULL AND COALESCE(finished, 0) = 0
             ORDER BY created_at_unix DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    Ok(exists.is_some())
}

/// Random interval pings are allowed only with no active timed task and no pending follow-up.
pub fn should_use_standard_schedule(
    conn: &Connection,
    now_unix: i64,
) -> rusqlite::Result<bool> {
    if get_awaiting_followup(conn)? {
        return Ok(false);
    }
    if has_future_planned_ping(conn, now_unix)? {
        return Ok(false);
    }
    if has_unfinished_timed_capture(conn)? {
        return Ok(false);
    }
    Ok(true)
}

/// Rolls the next ping using the user's random min/max interval and marks it **standard**.
pub fn schedule_random_next_ping<R: Rng + ?Sized>(
    conn: &Connection,
    now_unix: i64,
    rng: &mut R,
) -> Result<(), AppError> {
    if !random_ping_enabled(conn)? {
        return Ok(());
    }
    if !should_use_standard_schedule(conn, now_unix)? {
        return Ok(());
    }
    let (min_m, max_m) = ping_min_max_minutes(conn)?;
    let next = ping_plan::next_ping_after(now_unix, min_m, max_m, rng);
    set_next_ping_at_unix(conn, next)?;
    set_next_ping_kind(conn, NextPingKind::Standard)?;
    Ok(())
}

/// Rolls the next ping using overdue min/max while a timed task is past its planned end.
pub fn schedule_overdue_next_ping<R: Rng + ?Sized>(
    conn: &Connection,
    now_unix: i64,
    rng: &mut R,
) -> Result<(), AppError> {
    if !should_use_overdue_schedule(conn, now_unix)? {
        return Ok(());
    }
    let (min_m, max_m) = overdue_ping_min_max_minutes(conn)?;
    let next = ping_plan::next_ping_after(now_unix, min_m, max_m, rng);
    set_next_ping_at_unix(conn, next)?;
    set_next_ping_kind(conn, NextPingKind::Overdue)?;
    Ok(())
}

/// After a **planned_check** notification fires: wait for user reply; do **not** schedule a random ping.
pub fn apply_after_planned_check_ping(conn: &Connection) -> rusqlite::Result<()> {
    set_awaiting_followup(conn, true)?;
    conn.execute(
        "UPDATE scheduler_state SET next_ping_at_unix = NULL WHERE id = 1",
        [],
    )?;
    Ok(())
}

/// Marks the latest unfinished timed capture as explicitly completed and clears follow-up scheduling.
pub fn mark_latest_timed_capture_finished(conn: &Connection) -> Result<(), AppError> {
    conn.execute(
        "UPDATE captures SET finished = 1
         WHERE rowid = (
           SELECT rowid FROM captures
           WHERE duration_minutes IS NOT NULL AND COALESCE(finished, 0) = 0
           ORDER BY created_at_unix DESC LIMIT 1
         )",
        [],
    )?;
    set_awaiting_followup(conn, false)?;
    clear_planned_check_subject(conn)?;
    set_next_ping_kind(conn, NextPingKind::Standard)?;
    conn.execute(
        "UPDATE scheduler_state SET next_ping_at_unix = NULL WHERE id = 1",
        [],
    )?;
    Ok(())
}

pub fn get_next_ping_at_unix(conn: &Connection) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT next_ping_at_unix FROM scheduler_state WHERE id = 1",
        [],
        |row| row.get::<_, Option<i64>>(0),
    )
}

pub fn set_next_ping_at_unix(conn: &Connection, unix: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE scheduler_state SET next_ping_at_unix = ?1 WHERE id = 1",
        params![unix],
    )?;
    Ok(())
}

/// Seconds tolerance when recognizing a **snooze** (`snooze_next_ping`): delay ≈ 10 minutes.
fn approx_snooze_delay_secs(delta_secs: i64) -> bool {
    let target = SNOOZE_MINUTES * 60;
    (delta_secs - target).abs() <= 90
}

/// Latest moment a newly rolled ping may fall, relative to `now`: `max` interval + small slack.
fn max_valid_delay_secs(max_m: i64) -> i64 {
    max_m.saturating_mul(60).saturating_add(60)
}

/// Returns a `next_ping_at` strictly after `now_unix`.
///
/// If the row is missing **or** still stores a time in the past (stale DB after
/// reinstall, clock change, or long downtime), rolls a new instant and persists it.
///
/// If the stored time is **too far** in the future for the **current** min/max bounds
/// (e.g. user lowered max interval but `next_ping_at` still reflected old long-range
/// schedule), re-rolls — otherwise the scheduler would sleep for hours with no notifications.
/// Snooze delays (~10 min) are preserved when they exceed the current max interval (e.g. 1 min).
pub fn ensure_next_ping_scheduled<R: Rng + ?Sized>(
    conn: &Connection,
    now_unix: i64,
    rng: &mut R,
) -> Result<i64, AppError> {
    if should_use_overdue_schedule(conn, now_unix)? {
        return ensure_overdue_ping_scheduled(conn, now_unix, rng);
    }

    if get_awaiting_followup(conn)? {
        return Ok(now_unix.saturating_add(AWAITING_IDLE_SLEEP_SECS));
    }

    let (min_m, max_m) = ping_min_max_minutes(conn)?;

    match get_next_ping_at_unix(conn)? {
        Some(t) => {
            if t > now_unix {
                let delta = t.saturating_sub(now_unix);
                let kind = get_next_ping_kind(conn)?;
                let cap = match kind {
                    NextPingKind::Overdue => {
                        max_valid_delay_secs(overdue_ping_min_max_minutes(conn)?.1)
                    }
                    _ => max_valid_delay_secs(max_m),
                };
                let preserve_far_future = approx_snooze_delay_secs(delta)
                    || kind == NextPingKind::PlannedCheck
                    || kind == NextPingKind::Overdue;
                if delta > cap && !preserve_far_future {
                    if random_ping_enabled(conn)? {
                        let next = ping_plan::next_ping_after(now_unix, min_m, max_m, rng);
                        set_next_ping_at_unix(conn, next)?;
                        set_next_ping_kind(conn, NextPingKind::Standard)?;
                        Ok(next)
                    } else {
                        Ok(now_unix.saturating_add(AWAITING_IDLE_SLEEP_SECS))
                    }
                } else {
                    Ok(t)
                }
            } else if get_next_ping_kind(conn)? == NextPingKind::PlannedCheck {
                Ok(now_unix)
            } else if should_use_standard_schedule(conn, now_unix)? {
                schedule_random_next_ping(conn, now_unix, rng)?;
                Ok(get_next_ping_at_unix(conn)?.unwrap_or(now_unix))
            } else {
                Ok(now_unix.saturating_add(AWAITING_IDLE_SLEEP_SECS))
            }
        }
        None => {
            if should_use_standard_schedule(conn, now_unix)? {
                schedule_random_next_ping(conn, now_unix, rng)?;
                Ok(get_next_ping_at_unix(conn)?.unwrap_or(now_unix))
            } else {
                Ok(now_unix.saturating_add(AWAITING_IDLE_SLEEP_SECS))
            }
        }
    }
}

fn ensure_overdue_ping_scheduled<R: Rng + ?Sized>(
    conn: &Connection,
    now_unix: i64,
    rng: &mut R,
) -> Result<i64, AppError> {
    let (min_m, max_m) = overdue_ping_min_max_minutes(conn)?;
    match get_next_ping_at_unix(conn)? {
        Some(t) if t > now_unix => {
            let kind = get_next_ping_kind(conn)?;
            if kind == NextPingKind::Overdue {
                Ok(t)
            } else {
                let next = ping_plan::next_ping_after(now_unix, min_m, max_m, rng);
                set_next_ping_at_unix(conn, next)?;
                set_next_ping_kind(conn, NextPingKind::Overdue)?;
                Ok(next)
            }
        }
        _ => {
            let next = ping_plan::next_ping_after(now_unix, min_m, max_m, rng);
            set_next_ping_at_unix(conn, next)?;
            set_next_ping_kind(conn, NextPingKind::Overdue)?;
            Ok(next)
        }
    }
}

fn setting_bool(conn: &Connection, key: &str) -> rusqlite::Result<bool> {
    let v: String = conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )?;
    Ok(v.trim() == "1" || v.eq_ignore_ascii_case("true"))
}

fn set_setting_bool(conn: &Connection, key: &str, enabled: bool) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE settings SET value = ?1 WHERE key = ?2",
        params![if enabled { "1" } else { "0" }, key],
    )?;
    Ok(())
}

pub fn random_ping_enabled(conn: &Connection) -> rusqlite::Result<bool> {
    setting_bool(conn, "random_ping_enabled")
}

pub fn overdue_ping_enabled(conn: &Connection) -> rusqlite::Result<bool> {
    setting_bool(conn, "overdue_ping_enabled")
}

pub fn ping_min_max_minutes(conn: &Connection) -> rusqlite::Result<(i64, i64)> {
    let min: i64 = conn.query_row(
        "SELECT CAST(value AS INTEGER) FROM settings WHERE key = 'ping_min_minutes'",
        [],
        |row| row.get(0),
    )?;
    let max: i64 = conn.query_row(
        "SELECT CAST(value AS INTEGER) FROM settings WHERE key = 'ping_max_minutes'",
        [],
        |row| row.get(0),
    )?;
    Ok((min, max))
}

pub fn overdue_ping_min_max_minutes(conn: &Connection) -> rusqlite::Result<(i64, i64)> {
    let min: i64 = conn.query_row(
        "SELECT CAST(value AS INTEGER) FROM settings WHERE key = 'overdue_ping_min_minutes'",
        [],
        |row| row.get(0),
    )?;
    let max: i64 = conn.query_row(
        "SELECT CAST(value AS INTEGER) FROM settings WHERE key = 'overdue_ping_max_minutes'",
        [],
        |row| row.get(0),
    )?;
    Ok((min, max))
}

/// Upper bound for `ping_max_minutes` (one week). Keeps scheduling math predictable.
pub const PING_MAX_MINUTES_CAP: i64 = 10_080;

/// Fixed snooze delay (minutes). Next ping is set to `now + this many minutes`, persisted in `scheduler_state`.
pub const SNOOZE_MINUTES: i64 = 10;

/// When awaiting a reply after a planned-check ping, the scheduler sleeps in short polls instead of
/// rolling a random interval (see `ensure_next_ping_scheduled`).
const AWAITING_IDLE_SLEEP_SECS: i64 = 86_400;

/// Toast quick-adjust step (minutes) for ending early / extending the last logged segment.
pub const TOAST_ADJUST_MINUTES: i64 = 15;

/// Pushes the next ping to **now + [SNOOZE_MINUTES]**, overwriting any earlier scheduled time.
pub fn snooze_next_ping(conn: &Connection, now_unix: i64) -> rusqlite::Result<i64> {
    let next = now_unix.saturating_add(SNOOZE_MINUTES * 60);
    set_next_ping_at_unix(conn, next)?;
    set_next_ping_kind(conn, NextPingKind::Standard)?;
    set_awaiting_followup(conn, false)?;
    clear_planned_check_subject(conn)?;
    Ok(next)
}

pub fn set_ping_min_max_minutes(
    conn: &Connection,
    min_m: i64,
    max_m: i64,
    random_enabled: bool,
) -> Result<(), AppError> {
    if min_m < 1 {
        return Err(AppError::InvalidPingBounds(
            "minimum must be at least 1 minute".into(),
        ));
    }
    if max_m < min_m {
        return Err(AppError::InvalidPingBounds(
            "maximum must be greater than or equal to minimum".into(),
        ));
    }
    if max_m > PING_MAX_MINUTES_CAP {
        return Err(AppError::InvalidPingBounds(format!(
            "maximum must be at most {PING_MAX_MINUTES_CAP} minutes (one week)"
        )));
    }
    conn.execute(
        "UPDATE settings SET value = ?1 WHERE key = 'ping_min_minutes'",
        params![min_m.to_string()],
    )?;
    conn.execute(
        "UPDATE settings SET value = ?1 WHERE key = 'ping_max_minutes'",
        params![max_m.to_string()],
    )?;
    set_setting_bool(conn, "random_ping_enabled", random_enabled)?;
    Ok(())
}

pub fn set_overdue_ping_min_max_minutes(
    conn: &Connection,
    min_m: i64,
    max_m: i64,
    overdue_enabled: bool,
) -> Result<(), AppError> {
    if min_m < 1 {
        return Err(AppError::InvalidPingBounds(
            "minimum must be at least 1 minute".into(),
        ));
    }
    if max_m < min_m {
        return Err(AppError::InvalidPingBounds(
            "maximum must be greater than or equal to minimum".into(),
        ));
    }
    if max_m > PING_MAX_MINUTES_CAP {
        return Err(AppError::InvalidPingBounds(format!(
            "maximum must be at most {PING_MAX_MINUTES_CAP} minutes (one week)"
        )));
    }
    conn.execute(
        "UPDATE settings SET value = ?1 WHERE key = 'overdue_ping_min_minutes'",
        params![min_m.to_string()],
    )?;
    conn.execute(
        "UPDATE settings SET value = ?1 WHERE key = 'overdue_ping_max_minutes'",
        params![max_m.to_string()],
    )?;
    set_setting_bool(conn, "overdue_ping_enabled", overdue_enabled)?;
    Ok(())
}

/// Re-roll `next_ping_at` from **now** using current settings (used after interval change).
pub fn reschedule_next_ping_from_now<R: Rng + ?Sized>(
    conn: &Connection,
    now_unix: i64,
    rng: &mut R,
) -> Result<(), AppError> {
    let (min_m, max_m) = ping_min_max_minutes(conn)?;
    let next = ping_plan::next_ping_after(now_unix, min_m, max_m, rng);
    set_next_ping_at_unix(conn, next)?;
    set_next_ping_kind(conn, NextPingKind::Standard)?;
    set_awaiting_followup(conn, false)?;
    clear_planned_check_subject(conn)?;
    Ok(())
}

pub fn latest_capture_body(conn: &Connection) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT body FROM captures ORDER BY created_at_unix DESC LIMIT 1",
        [],
        |row| row.get::<_, String>(0),
    )
    .optional()
}

/// Latest capture row `(rowid, duration_minutes, body)` for toast duration tweaks.
fn latest_capture_row(conn: &Connection) -> rusqlite::Result<Option<(i64, Option<i64>, String)>> {
    conn.query_row(
        "SELECT rowid, duration_minutes, body FROM captures
         ORDER BY created_at_unix DESC LIMIT 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )
    .optional()
}

/// Latest row that still has a logged segment duration (skips blank-minute repeats).
fn latest_capture_with_duration_row(
    conn: &Connection,
) -> rusqlite::Result<Option<(i64, i64, i64, String)>> {
    conn.query_row(
        "SELECT rowid, created_at_unix, duration_minutes, body FROM captures
         WHERE duration_minutes IS NOT NULL
         ORDER BY created_at_unix DESC LIMIT 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )
    .optional()
}

/// Gap window after shortening the last segment (for UI prompt).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortenGapInfo {
    pub adjusted_end_unix: i64,
    pub gap_minutes: i64,
    pub suggested_activity: String,
}

fn gap_minutes_after_adjusted_end(adjusted_end_unix: i64, now_unix: i64) -> i64 {
    let gap_secs = now_unix.saturating_sub(adjusted_end_unix);
    (gap_secs / 60).min(PING_MAX_MINUTES_CAP)
}

/// Shortens the latest capture's logged segment (action ended earlier than planned).
pub fn shorten_last_capture_duration(
    conn: &Connection,
    now_unix: i64,
    delta_minutes: i64,
) -> Result<ShortenGapInfo, AppError> {
    let Some((rowid, created_at, dur, body)) = latest_capture_with_duration_row(conn)? else {
        return Err(AppError::NoAdjustableDuration);
    };
    let new_dur = (dur - delta_minutes).max(1);
    conn.execute(
        "UPDATE captures SET duration_minutes = ?1 WHERE rowid = ?2",
        params![new_dur, rowid],
    )?;
    let adjusted_end = created_at.saturating_add(new_dur.saturating_mul(60));
    let suggested = body.trim().to_string();
    Ok(ShortenGapInfo {
        adjusted_end_unix: adjusted_end,
        gap_minutes: gap_minutes_after_adjusted_end(adjusted_end, now_unix),
        suggested_activity: suggested,
    })
}

/// Logs the gap since the shortened segment ended and schedules the next planned check.
pub fn persist_gap_after_shorten<R: Rng + ?Sized>(
    conn: &Connection,
    text: &str,
    now_unix: i64,
    gap_minutes: i64,
    planned_duration_minutes: Option<i64>,
    rng: &mut R,
) -> Result<(), AppError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(AppError::EmptyCapture);
    }
    if gap_minutes > 0 {
        validate_planned_duration_minutes(gap_minutes)?;
        conn.execute(
            "INSERT INTO captures (body, created_at_unix, duration_minutes, thread_root)
             VALUES (?1, ?2, ?3, ?4)",
            params![trimmed, now_unix, gap_minutes, trimmed],
        )?;
    }
    match planned_duration_minutes {
        Some(d) => {
            validate_planned_duration_minutes(d)?;
            let next = now_unix.saturating_add(d.saturating_mul(60));
            set_next_ping_at_unix(conn, next)?;
            set_next_ping_kind(conn, NextPingKind::PlannedCheck)?;
            set_planned_check_subject(conn, trimmed)?;
            set_awaiting_followup(conn, false)?;
        }
        None => {
            if should_use_standard_schedule(conn, now_unix)? {
                schedule_random_next_ping(conn, now_unix, rng)?;
                clear_planned_check_subject(conn)?;
            }
        }
    }
    Ok(())
}

/// Extends the latest capture's logged segment and schedules a planned check after `delay_minutes`.
pub fn extend_last_capture_and_delay_ping(
    conn: &Connection,
    now_unix: i64,
    delay_minutes: i64,
) -> Result<i64, AppError> {
    validate_planned_duration_minutes(delay_minutes)?;
    let (rowid, new_dur, subject) =
        if let Some((rowid, _, dur, body)) = latest_capture_with_duration_row(conn)? {
            let trimmed = body.trim();
            if trimmed.is_empty() {
                return Err(AppError::NoPriorCapture);
            }
            let nd = dur.saturating_add(delay_minutes);
            validate_planned_duration_minutes(nd)?;
            (rowid, nd, trimmed.to_string())
        } else if let Some((rowid, _, body)) = latest_capture_row(conn)? {
            let trimmed = body.trim();
            if trimmed.is_empty() {
                return Err(AppError::NoPriorCapture);
            }
            (rowid, delay_minutes, trimmed.to_string())
        } else {
            return Err(AppError::NoPriorCapture);
        };
    conn.execute(
        "UPDATE captures SET duration_minutes = ?1 WHERE rowid = ?2",
        params![new_dur, rowid],
    )?;
    let next = now_unix.saturating_add(delay_minutes.saturating_mul(60));
    set_next_ping_at_unix(conn, next)?;
    set_next_ping_kind(conn, NextPingKind::PlannedCheck)?;
    set_planned_check_subject(conn, &subject)?;
    set_awaiting_followup(conn, false)?;
    Ok(next)
}

/// Records another capture with the **same body** as the latest row (same timestamp semantics as [`persist_capture`]).
pub fn persist_repeat_latest<R: Rng + ?Sized>(
    conn: &Connection,
    now_unix: i64,
    rng: &mut R,
) -> Result<(), AppError> {
    let Some(text) = latest_capture_body(conn)? else {
        return Err(AppError::NoPriorCapture);
    };
    if text.trim().is_empty() {
        return Err(AppError::NoPriorCapture);
    }
    let thread_root = latest_thread_root(conn)?.unwrap_or_else(|| text.clone());
    persist_capture_with_root(conn, &text, now_unix, rng, None, thread_root)
}

/// `thread_root` groups follow-up segments so stacked duration charts stay consistent.
pub fn persist_capture<R: Rng + ?Sized>(
    conn: &Connection,
    text: &str,
    now_unix: i64,
    rng: &mut R,
    planned_duration_minutes: Option<i64>,
) -> Result<(), AppError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(AppError::EmptyCapture);
    }

    let awaiting = get_awaiting_followup(conn)?;
    let thread_root = if awaiting {
        get_planned_check_subject(conn)?
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| trimmed.to_string())
    } else {
        trimmed.to_string()
    };

    persist_capture_with_root(conn, text, now_unix, rng, planned_duration_minutes, thread_root)
}

fn persist_capture_with_root<R: Rng + ?Sized>(
    conn: &Connection,
    text: &str,
    now_unix: i64,
    rng: &mut R,
    planned_duration_minutes: Option<i64>,
    thread_root: String,
) -> Result<(), AppError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(AppError::EmptyCapture);
    }

    let awaiting = get_awaiting_followup(conn)?;

    conn.execute(
        "INSERT INTO captures (body, created_at_unix, duration_minutes, thread_root)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            trimmed,
            now_unix,
            planned_duration_minutes,
            thread_root
        ],
    )?;

    if awaiting {
        set_awaiting_followup(conn, false)?;
        match planned_duration_minutes {
            Some(d) => {
                validate_planned_duration_minutes(d)?;
                let next = now_unix.saturating_add(d.saturating_mul(60));
                set_next_ping_at_unix(conn, next)?;
                set_next_ping_kind(conn, NextPingKind::PlannedCheck)?;
                set_planned_check_subject(conn, trimmed)?;
            }
            None => {
                if should_use_standard_schedule(conn, now_unix)? {
                    schedule_random_next_ping(conn, now_unix, rng)?;
                    clear_planned_check_subject(conn)?;
                }
            }
        }
        return Ok(());
    }

    match planned_duration_minutes {
        Some(d) => {
            validate_planned_duration_minutes(d)?;
            let next = now_unix.saturating_add(d.saturating_mul(60));
            set_next_ping_at_unix(conn, next)?;
            set_next_ping_kind(conn, NextPingKind::PlannedCheck)?;
            set_planned_check_subject(conn, trimmed)?;
        }
        None => {
            if should_use_standard_schedule(conn, now_unix)? {
                schedule_random_next_ping(conn, now_unix, rng)?;
                clear_planned_check_subject(conn)?;
            }
        }
    }
    Ok(())
}

pub fn latest_thread_root(conn: &Connection) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT COALESCE(thread_root, body) FROM captures ORDER BY created_at_unix DESC LIMIT 1",
        [],
        |row| row.get::<_, String>(0),
    )
    .optional()
}

/// What the capture tab should show as the user's current activity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentActivitySnapshot {
    pub body: Option<String>,
    pub started_at_unix: Option<i64>,
    pub duration_minutes: Option<i64>,
    pub planned_end_at_unix: Option<i64>,
}

/// Resolves **Nothing** vs active tracking for the capture header.
///
/// - **Nothing** when there are no captures, or when the latest `finished = 1` timed row
///   has no newer captures after it.
/// - **Active** when there is at least one capture after that boundary (including an
///   unfinished timed task, or any quick capture logged since the last explicit Done).
pub fn query_current_activity(conn: &Connection) -> rusqlite::Result<CurrentActivitySnapshot> {
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM captures", [], |row| row.get(0))?;
    if total == 0 {
        return Ok(CurrentActivitySnapshot {
            body: None,
            started_at_unix: None,
            duration_minutes: None,
            planned_end_at_unix: None,
        });
    }

    let last_finished_at: Option<i64> = conn
        .query_row(
            "SELECT created_at_unix FROM captures
             WHERE duration_minutes IS NOT NULL AND COALESCE(finished, 0) = 1
             ORDER BY created_at_unix DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;

    let active = match last_finished_at {
        Some(ts) => conn
            .query_row(
                "SELECT body, created_at_unix, duration_minutes FROM captures
                 WHERE created_at_unix > ?1
                 ORDER BY created_at_unix DESC LIMIT 1",
                params![ts],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                    ))
                },
            )
            .optional()?,
        None => conn
            .query_row(
                "SELECT body, created_at_unix, duration_minutes FROM captures
                 ORDER BY created_at_unix DESC LIMIT 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                    ))
                },
            )
            .optional()?,
    };

    match active {
        Some((body, started, duration_minutes)) if !body.trim().is_empty() => {
            let planned_end_at_unix = duration_minutes
                .filter(|d| *d > 0)
                .map(|d| started.saturating_add(d.saturating_mul(60)));
            Ok(CurrentActivitySnapshot {
                body: Some(body),
                started_at_unix: Some(started),
                duration_minutes,
                planned_end_at_unix,
            })
        }
        _ => Ok(CurrentActivitySnapshot {
            body: None,
            started_at_unix: None,
            duration_minutes: None,
            planned_end_at_unix: None,
        }),
    }
}

/// Latest captures first. `limit` is clamped to **1..=50** for predictable UI cost.
pub fn query_recent_captures(
    conn: &Connection,
    limit: u32,
) -> rusqlite::Result<Vec<(String, i64, Option<i64>, String)>> {
    let lim = (limit as i64).clamp(1, 50);
    let mut stmt = conn.prepare(
        "SELECT body, created_at_unix, duration_minutes,
                COALESCE(thread_root, body) AS tr
         FROM captures ORDER BY created_at_unix DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![lim], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, Option<i64>>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

/// Aggregated logged minutes per activity thread (`thread_root`), since `since_unix` (inclusive).
pub fn query_activity_digest(
    conn: &Connection,
    since_unix: i64,
) -> rusqlite::Result<Vec<(String, i64, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT COALESCE(thread_root, body),
                COALESCE(SUM(duration_minutes), 0),
                COUNT(*)
         FROM captures
         WHERE created_at_unix >= ?1
         GROUP BY COALESCE(thread_root, body)
         ORDER BY 2 DESC, 1 COLLATE NOCASE ASC",
    )?;
    let rows = stmt.query_map(params![since_unix], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

/// Captures in `[since_unix, until_unix]` (inclusive), oldest first — for timeline exports.
pub fn query_captures_in_range(
    conn: &Connection,
    since_unix: i64,
    until_unix: i64,
) -> rusqlite::Result<Vec<(String, i64, Option<i64>, String)>> {
    let mut stmt = conn.prepare(
        "SELECT body, created_at_unix, duration_minutes,
                COALESCE(thread_root, body) AS tr
         FROM captures
         WHERE created_at_unix >= ?1 AND created_at_unix <= ?2
         ORDER BY created_at_unix ASC",
    )?;
    let rows = stmt.query_map(params![since_unix, until_unix], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, Option<i64>>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

/// Max quick-pick chips shown in the capture UI (distinct bodies by frequency).
pub const TOP_QUICK_PICKS_CAP: u32 = 5;

/// Distinct capture texts ordered by how often they appear (then alphabetically, case-insensitive).
pub fn query_top_capture_bodies_by_frequency(
    conn: &Connection,
    limit: u32,
) -> rusqlite::Result<Vec<(String, i64)>> {
    let lim = (limit as i64).clamp(1, i64::from(TOP_QUICK_PICKS_CAP));
    let mut stmt = conn.prepare(
        "SELECT body, COUNT(*) AS cnt FROM captures
         GROUP BY body
         ORDER BY cnt DESC, body COLLATE NOCASE ASC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![lim], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_memory;
    use crate::error::AppError;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    #[test]
    fn opens_in_memory_applies_migration() {
        let conn = open_memory().expect("open memory db");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM settings", [], |row| row.get(0))
            .expect("count settings");
        assert_eq!(count, 6);
    }

    #[test]
    fn persist_capture_rejects_empty() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(7);
        let err = persist_capture(&conn, "   ", 1_000, &mut rng, None).unwrap_err();
        assert!(matches!(err, AppError::EmptyCapture));
    }

    #[test]
    fn persist_capture_trims_and_sets_next_ping() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(42);
        let now = 10_000_i64;
        persist_capture(&conn, "  working  ", now, &mut rng, None).expect("persist");

        let body: String = conn
            .query_row("SELECT body FROM captures LIMIT 1", [], |row| row.get(0))
            .expect("body");
        assert_eq!(body, "working");

        let next = get_next_ping_at_unix(&conn)
            .expect("query")
            .expect("next set");
        assert!(next > now);
    }

    #[test]
    fn ensure_next_ping_scheduled_fills_null() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(9);
        let next = ensure_next_ping_scheduled(&conn, 5_000, &mut rng).expect("schedule");
        assert!(next > 5_000);
        assert_eq!(get_next_ping_at_unix(&conn).unwrap().unwrap(), next);
    }

    #[test]
    fn ensure_next_ping_keeps_existing_when_still_future() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(3);
        let first = ensure_next_ping_scheduled(&conn, 1_000, &mut rng).expect("a");
        assert!(first > 1_000);
        let mid = 1_000 + (first - 1_000) / 2;
        let again = ensure_next_ping_scheduled(&conn, mid, &mut rng).expect("b");
        assert_eq!(first, again);
    }

    #[test]
    fn ensure_next_ping_advances_when_overdue() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(3);
        let first = ensure_next_ping_scheduled(&conn, 1_000, &mut rng).expect("a");
        let future_now = first + 3_600;
        let rolled = ensure_next_ping_scheduled(&conn, future_now, &mut rng).expect("roll");
        assert!(rolled > future_now);
    }

    #[test]
    fn set_ping_bounds_rejects_invalid() {
        let conn = open_memory().expect("db");
        assert!(matches!(
            set_ping_min_max_minutes(&conn, 0, 60, true),
            Err(AppError::InvalidPingBounds(_))
        ));
        assert!(matches!(
            set_ping_min_max_minutes(&conn, 10, 5, true),
            Err(AppError::InvalidPingBounds(_))
        ));
        assert!(matches!(
            set_ping_min_max_minutes(&conn, 1, PING_MAX_MINUTES_CAP + 1, true),
            Err(AppError::InvalidPingBounds(_))
        ));
    }

    #[test]
    fn set_ping_bounds_and_reschedule() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(11);
        set_ping_min_max_minutes(&conn, 5, 10, true).expect("set");
        assert_eq!(ping_min_max_minutes(&conn).unwrap(), (5, 10));
        let now = 2_000_000_i64;
        reschedule_next_ping_from_now(&conn, now, &mut rng).expect("roll");
        let next = get_next_ping_at_unix(&conn).unwrap().unwrap();
        assert!(next > now);
        assert!(next <= now + 10 * 60 + 1);
        assert!(next >= now + 5 * 60);
    }

    #[test]
    fn query_recent_captures_newest_first() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(99);
        persist_capture(&conn, "first", 100, &mut rng, None).expect("a");
        persist_capture(&conn, "second", 200, &mut rng, None).expect("b");
        let list = query_recent_captures(&conn, 10).expect("list");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].0, "second");
        assert_eq!(list[1].0, "first");
    }

    #[test]
    fn ensure_next_ping_reschedules_when_stored_ping_beyond_max_interval() {
        let conn = open_memory().expect("db");
        set_ping_min_max_minutes(&conn, 1, 1, true).expect("bounds");
        let now = 5_000_000_i64;
        set_next_ping_at_unix(&conn, now + 3_600).expect("far future");
        let mut rng = StdRng::seed_from_u64(42);
        let next = ensure_next_ping_scheduled(&conn, now, &mut rng).expect("ensure");
        assert!(next <= now + 2 * 60 + 5);
        assert!(next > now);
    }

    #[test]
    fn ensure_next_ping_preserves_snooze_under_tight_bounds() {
        let conn = open_memory().expect("db");
        set_ping_min_max_minutes(&conn, 1, 1, true).expect("bounds");
        let now = 8_000_000_i64;
        snooze_next_ping(&conn, now).expect("snooze");
        let mut rng = StdRng::seed_from_u64(99);
        let next = ensure_next_ping_scheduled(&conn, now, &mut rng).expect("ensure");
        assert_eq!(next, now + SNOOZE_MINUTES * 60);
    }

    #[test]
    fn snooze_next_ping_sets_ten_minutes_ahead() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(1);
        let now = 1_000_000_i64;
        ensure_next_ping_scheduled(&conn, now, &mut rng).expect("seed schedule");
        assert!(get_next_ping_at_unix(&conn).unwrap().unwrap() > now);

        let after_snooze = snooze_next_ping(&conn, now).expect("snooze");
        assert_eq!(after_snooze, now + SNOOZE_MINUTES * 60);
        assert_eq!(
            get_next_ping_at_unix(&conn).unwrap().unwrap(),
            now + SNOOZE_MINUTES * 60
        );
    }

    #[test]
    fn query_captures_in_range_orders_oldest_first() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(99);
        persist_capture(&conn, "a", 100, &mut rng, Some(10)).expect("a");
        persist_capture(&conn, "b", 200, &mut rng, None).expect("b");
        persist_capture(&conn, "c", 50, &mut rng, None).expect("out of range");
        let rows = query_captures_in_range(&conn, 90, 210).expect("range");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, "a");
        assert_eq!(rows[1].0, "b");
    }

    #[test]
    fn query_recent_captures_respects_limit_cap() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(5);
        for i in 0..60 {
            persist_capture(&conn, &format!("x{i}"), 1_000 + i as i64, &mut rng, None).expect("p");
        }
        let list = query_recent_captures(&conn, 999).expect("list");
        assert_eq!(list.len(), 50);
    }

    #[test]
    fn latest_capture_body_newest() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(3);
        persist_capture(&conn, "older", 100, &mut rng, None).expect("a");
        persist_capture(&conn, "newer", 200, &mut rng, None).expect("b");
        assert_eq!(
            latest_capture_body(&conn).expect("q"),
            Some("newer".to_string())
        );
    }

    #[test]
    fn persist_repeat_latest_inserts_copy_and_err_when_empty() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(8);
        let err = persist_repeat_latest(&conn, 500, &mut rng).unwrap_err();
        assert!(matches!(err, AppError::NoPriorCapture));

        persist_capture(&conn, "coding", 100, &mut rng, None).expect("first");
        persist_repeat_latest(&conn, 600, &mut rng).expect("repeat");

        let rows = query_recent_captures(&conn, 5).expect("list");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, "coding");
        assert_eq!(rows[1].0, "coding");
        assert_eq!(rows[0].1, 600);
        assert_eq!(rows[1].1, 100);
    }

    #[test]
    fn query_top_capture_bodies_by_frequency_orders_by_count() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(2);
        persist_capture(&conn, "rare", 10, &mut rng, None).expect("r");
        persist_capture(&conn, "often", 20, &mut rng, None).expect("o1");
        persist_capture(&conn, "often", 30, &mut rng, None).expect("o2");
        persist_capture(&conn, "often", 40, &mut rng, None).expect("o3");
        let top = query_top_capture_bodies_by_frequency(&conn, 5).expect("top");
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].0, "often");
        assert_eq!(top[0].1, 3);
        assert_eq!(top[1].0, "rare");
        assert_eq!(top[1].1, 1);
    }

    #[test]
    fn persist_capture_planned_duration_sets_planned_check() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(1);
        let now = 1_000_000_i64;
        persist_capture(&conn, "deep work", now, &mut rng, Some(45)).expect("p");
        assert_eq!(get_next_ping_kind(&conn).unwrap(), NextPingKind::PlannedCheck);
        assert_eq!(
            get_next_ping_at_unix(&conn).unwrap().unwrap(),
            now + 45 * 60
        );
        assert_eq!(
            get_planned_check_subject(&conn).unwrap().as_deref(),
            Some("deep work")
        );
    }

    #[test]
    fn ensure_next_ping_preserves_far_planned_check() {
        let conn = open_memory().expect("db");
        set_ping_min_max_minutes(&conn, 1, 2, true).expect("bounds");
        let now = 5_000_000_i64;
        let far = now + 4 * 3600;
        set_next_ping_at_unix(&conn, far).expect("set");
        set_next_ping_kind(&conn, NextPingKind::PlannedCheck).expect("kind");
        let mut rng = StdRng::seed_from_u64(42);
        let next = ensure_next_ping_scheduled(&conn, now, &mut rng).expect("ensure");
        assert_eq!(next, far);
        assert_eq!(get_next_ping_kind(&conn).unwrap(), NextPingKind::PlannedCheck);
    }

    #[test]
    fn follow_up_keeps_thread_root_and_stacks_duration() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(21);
        persist_capture(&conn, "reading", 1000, &mut rng, Some(30)).expect("first");
        set_awaiting_followup(&conn, true).expect("ping");
        set_planned_check_subject(&conn, "reading").expect("subj");
        persist_capture(&conn, "still reading", 2000, &mut rng, Some(15)).expect("follow");
        let list = query_recent_captures(&conn, 5).expect("list");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].3, "reading");
        assert_eq!(list[0].2, Some(15));
        assert_eq!(list[1].3, "reading");
        assert_eq!(list[1].2, Some(30));
        let digest = query_activity_digest(&conn, 0).expect("digest");
        let row = digest.iter().find(|(t, _, _)| t == "reading").expect("row");
        assert_eq!(row.1, 45);
        assert_eq!(row.2, 2);
    }

    #[test]
    fn awaiting_followup_blank_minutes_uses_random_interval() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(5);
        set_ping_min_max_minutes(&conn, 10, 20, true).expect("bounds");
        set_awaiting_followup(&conn, true).expect("wait");
        set_planned_check_subject(&conn, "reading").expect("subj");
        persist_capture(&conn, "still reading", 1000, &mut rng, None).expect("save");
        assert!(!get_awaiting_followup(&conn).unwrap());
        assert_eq!(get_next_ping_kind(&conn).unwrap(), NextPingKind::Standard);
        let next = get_next_ping_at_unix(&conn).unwrap().unwrap();
        assert!(next > 1000);
        assert!(next <= 1000 + 20 * 60 + 2);
        assert!(next >= 1000 + 10 * 60 - 2);
    }

    #[test]
    fn shorten_last_capture_duration_reduces_latest_segment() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(77);
        persist_capture(&conn, "work", 1000, &mut rng, Some(30)).expect("save");
        let info = shorten_last_capture_duration(&conn, 5000, TOAST_ADJUST_MINUTES)
            .expect("shorten");
        let rows = query_recent_captures(&conn, 1).expect("list");
        assert_eq!(rows[0].2, Some(15));
        assert_eq!(info.gap_minutes, (5000 - (1000 + 15 * 60)) / 60);
    }

    #[test]
    fn shorten_last_capture_duration_floors_at_one_minute() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(78);
        persist_capture(&conn, "work", 1000, &mut rng, Some(10)).expect("save");
        shorten_last_capture_duration(&conn, 5000, TOAST_ADJUST_MINUTES).expect("shorten");
        let rows = query_recent_captures(&conn, 1).expect("list");
        assert_eq!(rows[0].2, Some(1));
    }

    #[test]
    fn shorten_last_capture_duration_err_without_duration() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(79);
        persist_capture(&conn, "work", 1000, &mut rng, None).expect("save");
        let err =
            shorten_last_capture_duration(&conn, 5000, TOAST_ADJUST_MINUTES).unwrap_err();
        assert!(matches!(err, AppError::NoAdjustableDuration));
    }

    #[test]
    fn shorten_last_capture_duration_targets_latest_row_with_minutes() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(81);
        persist_capture(&conn, "work", 1000, &mut rng, Some(30)).expect("first");
        persist_repeat_latest(&conn, 1100, &mut rng).expect("repeat");
        shorten_last_capture_duration(&conn, 5000, TOAST_ADJUST_MINUTES).expect("shorten");
        let rows = query_recent_captures(&conn, 2).expect("list");
        assert_eq!(rows[0].2, None);
        assert_eq!(rows[1].2, Some(15));
    }

    #[test]
    fn persist_gap_after_shorten_logs_gap_and_planned_check() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(82);
        let created = 1_000_i64;
        let now = created + 90 * 60;
        persist_capture(&conn, "reading", created, &mut rng, Some(60)).expect("save");
        shorten_last_capture_duration(&conn, now, TOAST_ADJUST_MINUTES).expect("shorten");
        persist_gap_after_shorten(&conn, "email", now, 30, Some(20), &mut rng).expect("gap");
        let list = query_recent_captures(&conn, 5).expect("list");
        assert_eq!(list[0].0, "email");
        assert_eq!(list[0].2, Some(30));
        assert_eq!(get_next_ping_kind(&conn).unwrap(), NextPingKind::PlannedCheck);
        assert_eq!(
            get_planned_check_subject(&conn).unwrap().as_deref(),
            Some("email")
        );
        let next = get_next_ping_at_unix(&conn).unwrap().unwrap();
        assert_eq!(next, now + 20 * 60);
    }

    #[test]
    fn apply_after_planned_check_schedules_overdue_when_enabled() {
        let conn = open_memory().expect("db");
        set_overdue_ping_min_max_minutes(&conn, 5, 10, true).expect("bounds");
        let mut rng = StdRng::seed_from_u64(88);
        let now = 3_000_000_i64;
        persist_capture(&conn, "focus", now, &mut rng, Some(30)).expect("save");
        let planned = get_next_ping_at_unix(&conn).unwrap().unwrap();
        apply_after_planned_check_ping(&conn).expect("after");
        assert!(get_awaiting_followup(&conn).unwrap());
        let mut rng2 = StdRng::seed_from_u64(89);
        let next = ensure_next_ping_scheduled(&conn, planned + 1, &mut rng2).expect("ensure");
        assert!(next > planned);
        assert!(next <= planned + 10 * 60 + 2);
        assert_eq!(get_next_ping_kind(&conn).unwrap(), NextPingKind::Overdue);
    }

    #[test]
    fn mark_latest_timed_capture_finished_allows_standard_schedule() {
        let conn = open_memory().expect("db");
        set_ping_min_max_minutes(&conn, 5, 10, true).expect("bounds");
        let mut rng = StdRng::seed_from_u64(90);
        let now = 4_000_000_i64;
        persist_capture(&conn, "task", now, &mut rng, Some(20)).expect("save");
        mark_latest_timed_capture_finished(&conn).expect("done");
        assert!(!has_unfinished_timed_capture(&conn).unwrap());
        assert!(should_use_standard_schedule(&conn, now).unwrap());
        let next = ensure_next_ping_scheduled(&conn, now, &mut rng).expect("roll");
        assert!(next > now);
        assert!(next <= now + 10 * 60 + 2);
    }

    #[test]
    fn timed_capture_blocks_random_until_finished() {
        let conn = open_memory().expect("db");
        set_ping_min_max_minutes(&conn, 1, 1, true).expect("bounds");
        let mut rng = StdRng::seed_from_u64(91);
        let now = 5_000_000_i64;
        persist_capture(&conn, "work", now, &mut rng, Some(45)).expect("save");
        assert!(!should_use_standard_schedule(&conn, now).unwrap());
        schedule_random_next_ping(&conn, now, &mut rng).expect("noop");
        assert_eq!(get_next_ping_kind(&conn).unwrap(), NextPingKind::PlannedCheck);
    }

    #[test]
    fn extend_last_capture_and_delay_ping_adds_minutes_and_schedules() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(80);
        let now = 2_000_000_i64;
        persist_capture(&conn, "reading", now - 3600, &mut rng, Some(30)).expect("save");
        set_awaiting_followup(&conn, true).expect("follow");
        let next =
            extend_last_capture_and_delay_ping(&conn, now, TOAST_ADJUST_MINUTES).expect("extend");
        assert_eq!(next, now + TOAST_ADJUST_MINUTES * 60);
        assert_eq!(get_next_ping_kind(&conn).unwrap(), NextPingKind::PlannedCheck);
        assert!(!get_awaiting_followup(&conn).unwrap());
        assert_eq!(
            get_planned_check_subject(&conn).unwrap().as_deref(),
            Some("reading")
        );
        let rows = query_recent_captures(&conn, 1).expect("list");
        assert_eq!(rows[0].2, Some(45));
    }

    #[test]
    fn query_current_activity_empty_db() {
        let conn = open_memory().expect("db");
        let snap = query_current_activity(&conn).expect("query");
        assert_eq!(snap.body, None);
        assert_eq!(snap.started_at_unix, None);
        assert_eq!(snap.duration_minutes, None);
    }

    #[test]
    fn query_current_activity_shows_latest_capture() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(201);
        persist_capture(&conn, "reading", 1_000, &mut rng, None).expect("save");
        let snap = query_current_activity(&conn).expect("query");
        assert_eq!(snap.body.as_deref(), Some("reading"));
        assert_eq!(snap.started_at_unix, Some(1_000));
        assert_eq!(snap.duration_minutes, None);
    }

    #[test]
    fn query_current_activity_nothing_after_mark_done_without_new_capture() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(202);
        persist_capture(&conn, "task", 2_000, &mut rng, Some(30)).expect("save");
        mark_latest_timed_capture_finished(&conn).expect("done");
        let snap = query_current_activity(&conn).expect("query");
        assert_eq!(snap.body, None);
    }

    #[test]
    fn query_current_activity_after_done_with_new_capture() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(203);
        persist_capture(&conn, "task", 2_000, &mut rng, Some(30)).expect("save");
        mark_latest_timed_capture_finished(&conn).expect("done");
        persist_capture(&conn, "email", 3_000, &mut rng, None).expect("save");
        let snap = query_current_activity(&conn).expect("query");
        assert_eq!(snap.body.as_deref(), Some("email"));
        assert_eq!(snap.started_at_unix, Some(3_000));
        assert_eq!(snap.duration_minutes, None);
    }

    #[test]
    fn query_current_activity_unfinished_timed_task() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(204);
        persist_capture(&conn, "deep work", 4_000, &mut rng, Some(45)).expect("save");
        let snap = query_current_activity(&conn).expect("query");
        assert_eq!(snap.body.as_deref(), Some("deep work"));
        assert_eq!(snap.started_at_unix, Some(4_000));
        assert_eq!(snap.duration_minutes, Some(45));
    }
}
