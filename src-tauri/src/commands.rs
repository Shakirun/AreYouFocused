use crate::db::repo;
use crate::error::AppError;
use crate::AppState;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

/// Snapshot of scheduler row + ping interval settings for the UI.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerStatus {
    pub next_ping_at_unix: Option<i64>,
    pub ping_min_minutes: i64,
    pub ping_max_minutes: i64,
}

#[tauri::command]
pub fn get_scheduler_status(state: State<'_, AppState>) -> Result<SchedulerStatus, AppError> {
    let mut db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let conn = &mut *db;
    let next_ping_at_unix = repo::get_next_ping_at_unix(conn)?;
    let (ping_min_minutes, ping_max_minutes) = repo::ping_min_max_minutes(conn)?;
    Ok(SchedulerStatus {
        next_ping_at_unix,
        ping_min_minutes,
        ping_max_minutes,
    })
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[tauri::command]
pub fn submit_capture(state: State<'_, AppState>, text: String) -> Result<(), AppError> {
    let mut db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let conn = &mut *db;
    repo::persist_capture(conn, &text, unix_now(), &mut rand::thread_rng())
}
