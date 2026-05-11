//! Data access helpers. Connection ownership stays with `AppState` (later).

#[cfg(test)]
mod tests {
    #[test]
    fn opens_in_memory_applies_migration() {
        let conn = crate::db::open_memory().expect("open memory db");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM settings", [], |row| row.get(0))
            .expect("count settings");
        assert_eq!(count, 2);
    }
}
