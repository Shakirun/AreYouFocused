//! Data access helpers.

use crate::domain::ping_plan;
use crate::error::AppError;
use rand::Rng;
use rusqlite::{params, Connection};

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

/// Returns a `next_ping_at` strictly after `now_unix`.
///
/// If the row is missing **or** still stores a time in the past (stale DB after
/// reinstall, clock change, or long downtime), rolls a new instant and persists it.
/// A stale timestamp used to make the scheduler sleep 0s and spam notifications.
pub fn ensure_next_ping_scheduled<R: Rng + ?Sized>(
    conn: &Connection,
    now_unix: i64,
    rng: &mut R,
) -> Result<i64, AppError> {
    match get_next_ping_at_unix(conn)? {
        Some(t) if t > now_unix => Ok(t),
        Some(_) | None => {
            let (min_m, max_m) = ping_min_max_minutes(conn)?;
            let next = ping_plan::next_ping_after(now_unix, min_m, max_m, rng);
            set_next_ping_at_unix(conn, next)?;
            Ok(next)
        }
    }
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

/// Upper bound for `ping_max_minutes` (one week). Keeps scheduling math predictable.
pub const PING_MAX_MINUTES_CAP: i64 = 10_080;

/// Fixed snooze delay (minutes). Next ping is set to `now + this many minutes`, persisted in `scheduler_state`.
pub const SNOOZE_MINUTES: i64 = 10;

/// Pushes the next ping to **now + [SNOOZE_MINUTES]**, overwriting any earlier scheduled time.
pub fn snooze_next_ping(conn: &Connection, now_unix: i64) -> rusqlite::Result<i64> {
    let next = now_unix.saturating_add(SNOOZE_MINUTES * 60);
    set_next_ping_at_unix(conn, next)?;
    Ok(next)
}

pub fn set_ping_min_max_minutes(conn: &Connection, min_m: i64, max_m: i64) -> Result<(), AppError> {
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
    Ok(())
}

pub fn persist_capture<R: Rng + ?Sized>(
    conn: &Connection,
    text: &str,
    now_unix: i64,
    rng: &mut R,
) -> Result<(), AppError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(AppError::EmptyCapture);
    }

    let (min_m, max_m) = ping_min_max_minutes(conn)?;
    conn.execute(
        "INSERT INTO captures (body, created_at_unix) VALUES (?1, ?2)",
        params![trimmed, now_unix],
    )?;
    let next = ping_plan::next_ping_after(now_unix, min_m, max_m, rng);
    set_next_ping_at_unix(conn, next)?;
    Ok(())
}

/// Latest captures first. `limit` is clamped to **1..=50** for predictable UI cost.
pub fn query_recent_captures(
    conn: &Connection,
    limit: u32,
) -> rusqlite::Result<Vec<(String, i64)>> {
    let lim = (limit as i64).clamp(1, 50);
    let mut stmt = conn.prepare(
        "SELECT body, created_at_unix FROM captures ORDER BY created_at_unix DESC LIMIT ?1",
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
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    #[test]
    fn opens_in_memory_applies_migration() {
        let conn = open_memory().expect("open memory db");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM settings", [], |row| row.get(0))
            .expect("count settings");
        assert_eq!(count, 2);
    }

    #[test]
    fn persist_capture_rejects_empty() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(7);
        let err = persist_capture(&conn, "   ", 1_000, &mut rng).unwrap_err();
        assert!(matches!(err, AppError::EmptyCapture));
    }

    #[test]
    fn persist_capture_trims_and_sets_next_ping() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(42);
        let now = 10_000_i64;
        persist_capture(&conn, "  working  ", now, &mut rng).expect("persist");

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
            set_ping_min_max_minutes(&conn, 0, 60),
            Err(AppError::InvalidPingBounds(_))
        ));
        assert!(matches!(
            set_ping_min_max_minutes(&conn, 10, 5),
            Err(AppError::InvalidPingBounds(_))
        ));
        assert!(matches!(
            set_ping_min_max_minutes(&conn, 1, PING_MAX_MINUTES_CAP + 1),
            Err(AppError::InvalidPingBounds(_))
        ));
    }

    #[test]
    fn set_ping_bounds_and_reschedule() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(11);
        set_ping_min_max_minutes(&conn, 5, 10).expect("set");
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
        persist_capture(&conn, "first", 100, &mut rng).expect("a");
        persist_capture(&conn, "second", 200, &mut rng).expect("b");
        let list = query_recent_captures(&conn, 10).expect("list");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].0, "second");
        assert_eq!(list[1].0, "first");
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
    fn query_recent_captures_respects_limit_cap() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(5);
        for i in 0..60 {
            persist_capture(&conn, &format!("x{i}"), 1_000 + i as i64, &mut rng).expect("p");
        }
        let list = query_recent_captures(&conn, 999).expect("list");
        assert_eq!(list.len(), 50);
    }
}
