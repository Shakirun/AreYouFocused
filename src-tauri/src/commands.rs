use crate::db::repo;
use crate::error::AppError;
use crate::AppState;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[tauri::command]
pub fn submit_capture(state: State<'_, AppState>, text: String) -> Result<(), AppError> {
    let mut db = state
        .db
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let conn = &mut *db;
    repo::persist_capture(conn, &text, unix_now(), &mut rand::thread_rng())
}
