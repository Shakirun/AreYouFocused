use rusqlite::Connection;

const INITIAL_SQL: &str = include_str!("../../migrations/001_initial.sql");

pub fn apply_initial(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(INITIAL_SQL)?;
    migrate_scheduler_followup(conn)?;
    migrate_captures_analytics(conn)?;
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
