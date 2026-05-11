use crate::db::repo;
use crate::error::AppError;
use crate::AppState;
use rusqlite::Connection;
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

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureRow {
    pub body: String,
    pub created_at_unix: i64,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickPickRow {
    pub body: String,
    pub count: i64,
}

#[tauri::command]
pub fn get_scheduler_status(state: State<'_, AppState>) -> Result<SchedulerStatus, AppError> {
    let mut db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let conn = &mut *db;
    read_scheduler_status(conn)
}

#[tauri::command]
pub fn snooze_ping(state: State<'_, AppState>) -> Result<SchedulerStatus, AppError> {
    let mut db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let conn = &mut *db;
    repo::snooze_next_ping(conn, unix_now())?;
    read_scheduler_status(conn)
}

/// Saves another capture with the same text as the most recent entry (for toast/quick actions).
#[tauri::command]
pub fn repeat_last_capture(state: State<'_, AppState>) -> Result<SchedulerStatus, AppError> {
    let mut db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let conn = &mut *db;
    repo::persist_repeat_latest(conn, unix_now(), &mut rand::thread_rng())?;
    read_scheduler_status(conn)
}

#[tauri::command]
pub fn update_ping_interval(
    state: State<'_, AppState>,
    ping_min_minutes: i64,
    ping_max_minutes: i64,
) -> Result<SchedulerStatus, AppError> {
    let mut db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let conn = &mut *db;
    repo::set_ping_min_max_minutes(conn, ping_min_minutes, ping_max_minutes)?;
    repo::reschedule_next_ping_from_now(conn, unix_now(), &mut rand::thread_rng())?;
    read_scheduler_status(conn)
}

fn read_scheduler_status(conn: &Connection) -> Result<SchedulerStatus, AppError> {
    let mut rng = rand::thread_rng();
    repo::ensure_next_ping_scheduled(conn, unix_now(), &mut rng)?;
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

#[tauri::command]
pub fn list_recent_captures(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<CaptureRow>, AppError> {
    let lim = limit.unwrap_or(15).clamp(1, 50);
    let mut db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let conn = &mut *db;
    let rows = repo::query_recent_captures(conn, lim)?;
    Ok(rows
        .into_iter()
        .map(|(body, created_at_unix)| CaptureRow {
            body,
            created_at_unix,
        })
        .collect())
}

/// Top distinct capture texts by frequency (for quick-insert chips in the capture UI).
#[tauri::command]
pub fn list_top_quick_picks(state: State<'_, AppState>) -> Result<Vec<QuickPickRow>, AppError> {
    let mut db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let conn = &mut *db;
    let rows = repo::query_top_capture_bodies_by_frequency(conn, repo::TOP_QUICK_PICKS_CAP)?;
    Ok(rows
        .into_iter()
        .map(|(body, count)| QuickPickRow { body, count })
        .collect())
}
