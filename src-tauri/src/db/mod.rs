//! SQLite connection and migrations.

mod migrations;
pub mod daily_reminder;
pub mod repo;
pub mod sleep_hours;

use rusqlite::Connection;
use std::path::Path;

pub fn open_memory() -> rusqlite::Result<Connection> {
    let conn = Connection::open_in_memory()?;
    migrations::apply_initial(&conn)?;
    Ok(conn)
}

/// Opens the on-disk database (used when Tauri app state is wired).
#[allow(dead_code)]
pub fn open_database(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    migrations::apply_initial(&conn)?;
    Ok(conn)
}
