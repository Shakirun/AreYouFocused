use crate::db::repo::{self, ShortenGapInfo};
use crate::error::AppError;
use crate::AppState;
use rusqlite::Connection;
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitCaptureInput {
    pub text: String,
    pub planned_duration_minutes: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitGapAfterShortenInput {
    pub text: String,
    pub gap_minutes: i64,
    pub planned_duration_minutes: Option<i64>,
}

/// Snapshot of scheduler row + ping interval settings for the UI.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerStatus {
    pub next_ping_at_unix: Option<i64>,
    pub ping_min_minutes: i64,
    pub ping_max_minutes: i64,
    pub random_ping_enabled: bool,
    pub overdue_ping_enabled: bool,
    pub overdue_ping_min_minutes: i64,
    pub overdue_ping_max_minutes: i64,
    pub awaiting_followup: bool,
    pub planned_check_subject: Option<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureRow {
    pub body: String,
    pub created_at_unix: i64,
    pub duration_minutes: Option<i64>,
    pub thread_root: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityDigestRow {
    pub thread_root: String,
    pub total_minutes: i64,
    pub capture_count: i64,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickPickRow {
    pub body: String,
    pub count: i64,
}

/// Active capture for the "Currently on:" header (`None` body → show "Nothing").
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentActivityStatus {
    pub body: Option<String>,
    pub started_at_unix: Option<i64>,
    pub duration_minutes: Option<i64>,
    pub planned_end_at_unix: Option<i64>,
}

#[tauri::command]
pub fn get_current_activity(
    state: State<'_, AppState>,
) -> Result<CurrentActivityStatus, AppError> {
    let db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let conn = &*db;
    let snap = repo::query_current_activity(conn)?;
    Ok(CurrentActivityStatus {
        body: snap.body,
        started_at_unix: snap.started_at_unix,
        duration_minutes: snap.duration_minutes,
        planned_end_at_unix: snap.planned_end_at_unix,
    })
}

#[tauri::command]
pub fn get_scheduler_status(state: State<'_, AppState>) -> Result<SchedulerStatus, AppError> {
    let mut db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let conn = &mut *db;
    read_scheduler_status(conn)
}

#[tauri::command]
pub fn mark_task_done(state: State<'_, AppState>) -> Result<SchedulerStatus, AppError> {
    let mut db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let conn = &mut *db;
    repo::mark_latest_timed_capture_finished(conn)?;
    read_scheduler_status(conn)
}

#[tauri::command]
pub fn snooze_ping(state: State<'_, AppState>) -> Result<SchedulerStatus, AppError> {
    let mut db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let conn = &mut *db;
    repo::snooze_next_ping(conn, unix_now())?;
    read_scheduler_status(conn)
}

#[tauri::command]
pub fn shorten_last_capture(
    state: State<'_, AppState>,
    minutes: i64,
) -> Result<ShortenGapInfo, AppError> {
    let mut db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let conn = &mut *db;
    repo::validate_planned_duration_minutes(minutes)?;
    repo::shorten_last_capture_duration(conn, unix_now(), minutes)
}

#[tauri::command]
pub fn shorten_last_capture_15(
    state: State<'_, AppState>,
) -> Result<ShortenGapInfo, AppError> {
    shorten_last_capture(state, repo::TOAST_ADJUST_MINUTES)
}

#[tauri::command]
pub fn submit_gap_after_shorten(
    state: State<'_, AppState>,
    input: SubmitGapAfterShortenInput,
) -> Result<SchedulerStatus, AppError> {
    let mut db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let conn = &mut *db;
    repo::persist_gap_after_shorten(
        conn,
        &input.text,
        unix_now(),
        input.gap_minutes,
        input.planned_duration_minutes,
        &mut rand::thread_rng(),
    )?;
    read_scheduler_status(conn)
}

#[tauri::command]
pub fn extend_last_capture(
    state: State<'_, AppState>,
    minutes: i64,
) -> Result<SchedulerStatus, AppError> {
    let mut db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let conn = &mut *db;
    repo::validate_planned_duration_minutes(minutes)?;
    repo::extend_last_capture_and_delay_ping(conn, unix_now(), minutes)?;
    read_scheduler_status(conn)
}

#[tauri::command]
pub fn extend_last_capture_15(
    state: State<'_, AppState>,
) -> Result<SchedulerStatus, AppError> {
    extend_last_capture(state, repo::TOAST_ADJUST_MINUTES)
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
    random_ping_enabled: bool,
) -> Result<SchedulerStatus, AppError> {
    let mut db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let conn = &mut *db;
    repo::set_ping_min_max_minutes(conn, ping_min_minutes, ping_max_minutes, random_ping_enabled)?;
    if random_ping_enabled {
        repo::reschedule_next_ping_from_now(conn, unix_now(), &mut rand::thread_rng())?;
    }
    read_scheduler_status(conn)
}

#[tauri::command]
pub fn update_overdue_ping_interval(
    state: State<'_, AppState>,
    overdue_ping_min_minutes: i64,
    overdue_ping_max_minutes: i64,
    overdue_ping_enabled: bool,
) -> Result<SchedulerStatus, AppError> {
    let mut db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let conn = &mut *db;
    repo::set_overdue_ping_min_max_minutes(
        conn,
        overdue_ping_min_minutes,
        overdue_ping_max_minutes,
        overdue_ping_enabled,
    )?;
    let mut rng = rand::thread_rng();
    repo::ensure_next_ping_scheduled(conn, unix_now(), &mut rng)?;
    read_scheduler_status(conn)
}

fn read_scheduler_status(conn: &Connection) -> Result<SchedulerStatus, AppError> {
    let mut rng = rand::thread_rng();
    repo::ensure_next_ping_scheduled(conn, unix_now(), &mut rng)?;
    let next_ping_at_unix = repo::get_next_ping_at_unix(conn)?;
    let (ping_min_minutes, ping_max_minutes) = repo::ping_min_max_minutes(conn)?;
    let random_ping_enabled = repo::random_ping_enabled(conn)?;
    let overdue_ping_enabled = repo::overdue_ping_enabled(conn)?;
    let (overdue_ping_min_minutes, overdue_ping_max_minutes) =
        repo::overdue_ping_min_max_minutes(conn)?;
    let awaiting_followup = repo::get_awaiting_followup(conn)?;
    let planned_check_subject = repo::get_planned_check_subject(conn)?;
    Ok(SchedulerStatus {
        next_ping_at_unix,
        ping_min_minutes,
        ping_max_minutes,
        random_ping_enabled,
        overdue_ping_enabled,
        overdue_ping_min_minutes,
        overdue_ping_max_minutes,
        awaiting_followup,
        planned_check_subject,
    })
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[tauri::command]
pub fn submit_capture(
    state: State<'_, AppState>,
    input: SubmitCaptureInput,
) -> Result<(), AppError> {
    let mut db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let conn = &mut *db;
    repo::persist_capture(
        conn,
        &input.text,
        unix_now(),
        &mut rand::thread_rng(),
        input.planned_duration_minutes,
    )
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
        .map(
            |(body, created_at_unix, duration_minutes, thread_root)| CaptureRow {
                body,
                created_at_unix,
                duration_minutes,
                thread_root,
            },
        )
        .collect())
}

#[tauri::command]
pub fn list_activity_digest(
    state: State<'_, AppState>,
    since_unix: i64,
) -> Result<Vec<ActivityDigestRow>, AppError> {
    let mut db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let conn = &mut *db;
    let rows = repo::query_activity_digest(conn, since_unix)?;
    Ok(rows
        .into_iter()
        .map(
            |(thread_root, total_minutes, capture_count)| ActivityDigestRow {
                thread_root,
                total_minutes,
                capture_count,
            },
        )
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
