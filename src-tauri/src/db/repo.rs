//! Data access helpers.

use crate::domain::ping_plan;
use crate::error::AppError;
use rand::Rng;
use rusqlite::{params, Connection};

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
    conn.execute(
        "UPDATE scheduler_state SET next_ping_at_unix = ?1 WHERE id = 1",
        params![next],
    )?;
    Ok(())
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

        let next: i64 = conn
            .query_row(
                "SELECT next_ping_at_unix FROM scheduler_state WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .expect("next");
        assert!(next > now);
    }
}
