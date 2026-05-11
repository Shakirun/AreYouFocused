use rusqlite::Connection;

const INITIAL_SQL: &str = include_str!("../../migrations/001_initial.sql");

pub fn apply_initial(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(INITIAL_SQL)
}
