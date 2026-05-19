use crate::db::daily_reminder::{
    self, DailyReminderSettings, SaveDailyReminderInput, PRESET_CATALOG,
};
use crate::db::repo::{self, ShortenGapInfo};
use crate::db::sleep_hours::{self, SleepHoursSettings};
use crate::error::AppError;
use crate::export::{self, HistoryReport};
use crate::AppState;
use rusqlite::Connection;
use serde::Deserialize;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, State};
#[cfg(desktop)]
use tauri::{WebviewUrl, WebviewWindowBuilder};
#[cfg(desktop)]
use tauri::webview::PageLoadEvent;
use tauri_plugin_dialog::DialogExt;

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
    repo::mark_latest_timed_capture_finished(conn, unix_now())?;
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportHistoryInput {
    pub format: String,
    pub since_unix: i64,
    pub until_unix: i64,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportHistoryResult {
    pub saved: bool,
    pub path: Option<String>,
}

fn build_report_locked(
    state: &State<'_, AppState>,
    since_unix: i64,
    until_unix: i64,
) -> Result<HistoryReport, AppError> {
    let db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let conn = &*db;
    export::build_history_report(conn, since_unix, until_unix, unix_now())
}

/// Save history report as CSV or XLSX via native save dialog.
#[tauri::command]
pub fn export_history_file(
    app: AppHandle,
    state: State<'_, AppState>,
    input: ExportHistoryInput,
) -> Result<ExportHistoryResult, AppError> {
    let report = build_report_locked(&state, input.since_unix, input.until_unix)?;
    let (title, filter_name, ext): (&str, &str, &[&str]) = match input.format.as_str() {
        "csv" => ("Save history as CSV", "CSV", &["csv"]),
        "xlsx" => ("Save history as Excel", "Excel", &["xlsx"]),
        other => {
            return Err(AppError::Export(format!(
                "unsupported format: {other} (use csv or xlsx)"
            )))
        }
    };

    let path = app
        .dialog()
        .file()
        .set_title(title)
        .add_filter(filter_name, ext)
        .blocking_save_file();

    let Some(path) = path else {
        return Ok(ExportHistoryResult {
            saved: false,
            path: None,
        });
    };

    let path_buf: PathBuf = path.into_path().map_err(|e| AppError::Export(e.to_string()))?;
    match input.format.as_str() {
        "csv" => {
            let csv = export::report_to_csv(&report);
            std::fs::write(&path_buf, csv).map_err(|e| AppError::Export(e.to_string()))?;
        }
        "xlsx" => export::write_xlsx(&report, &path_buf)?,
        _ => unreachable!(),
    }

    Ok(ExportHistoryResult {
        saved: true,
        path: Some(path_buf.to_string_lossy().into_owned()),
    })
}

/// Print-ready HTML for the selected period (use system Print → Save as PDF).
#[tauri::command]
pub fn history_report_html(
    state: State<'_, AppState>,
    since_unix: i64,
    until_unix: i64,
) -> Result<String, AppError> {
    let report = build_report_locked(&state, since_unix, until_unix)?;
    Ok(export::report_to_html(&report))
}

const HISTORY_REPORT_WINDOW_LABEL: &str = "history-report";

/// Open a dedicated webview with the report and trigger the system print dialog.
///
/// Uses a Tauri webview instead of `window.open()` (blocked in the embedded webview).
#[tauri::command]
pub async fn history_report_print(
    app: AppHandle,
    state: State<'_, AppState>,
    since_unix: i64,
    until_unix: i64,
) -> Result<(), AppError> {
    #[cfg(not(desktop))]
    {
        let _ = (app, state, since_unix, until_unix);
        return Err(AppError::Export("PDF print is desktop-only".into()));
    }

    #[cfg(desktop)]
    {
        let report = build_report_locked(&state, since_unix, until_unix)?;
        let html = export::report_to_html(&report);

        let temp_dir = app
            .path()
            .temp_dir()
            .map_err(|e| AppError::Export(e.to_string()))?;
        let html_path = temp_dir.join(format!(
            "areyoufocused-report-{}.html",
            unix_now()
        ));
        std::fs::write(&html_path, html).map_err(|e| AppError::Export(e.to_string()))?;

        let file_url = tauri::Url::from_file_path(&html_path)
            .map_err(|_| AppError::Export("could not build report file URL".into()))?;

        if let Some(existing) = app.get_webview_window(HISTORY_REPORT_WINDOW_LABEL) {
            let _ = existing.close();
        }

        WebviewWindowBuilder::new(
            &app,
            HISTORY_REPORT_WINDOW_LABEL,
            WebviewUrl::External(file_url),
        )
        .title("AreYouFocused — History report")
        .inner_size(920.0, 720.0)
        .center()
        .on_page_load(|window, payload| {
            if payload.event() == PageLoadEvent::Finished {
                let _ = window.eval("window.print();");
            }
        })
        .build()
        .map_err(|e| AppError::Export(e.to_string()))?;

        Ok(())
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyReminderPreset {
    pub key: String,
    pub label: String,
}

#[tauri::command]
pub fn get_daily_reminder_presets() -> Vec<DailyReminderPreset> {
    PRESET_CATALOG
        .iter()
        .map(|(key, label)| DailyReminderPreset {
            key: (*key).to_string(),
            label: (*label).to_string(),
        })
        .collect()
}

#[tauri::command]
pub fn get_daily_reminder_settings(
    state: State<'_, AppState>,
) -> Result<DailyReminderSettings, AppError> {
    let db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let conn = &*db;
    Ok(daily_reminder::read_daily_reminder_settings(conn)?)
}

#[tauri::command]
pub fn set_daily_reminder_enabled(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<DailyReminderSettings, AppError> {
    let mut db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let conn = &mut *db;
    daily_reminder::set_daily_reminder_enabled(conn, enabled)?;
    Ok(daily_reminder::read_daily_reminder_settings(conn)?)
}

#[tauri::command]
pub fn save_daily_reminder(
    state: State<'_, AppState>,
    input: SaveDailyReminderInput,
) -> Result<DailyReminderSettings, AppError> {
    let mut db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let conn = &mut *db;
    daily_reminder::save_daily_reminder(conn, &input)?;
    Ok(daily_reminder::read_daily_reminder_settings(conn)?)
}

#[tauri::command]
pub fn delete_daily_reminder(
    state: State<'_, AppState>,
    id: i64,
) -> Result<DailyReminderSettings, AppError> {
    let mut db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let conn = &mut *db;
    daily_reminder::delete_daily_reminder(conn, id)?;
    Ok(daily_reminder::read_daily_reminder_settings(conn)?)
}

#[tauri::command]
pub fn get_sleep_hours_settings(
    state: State<'_, AppState>,
) -> Result<SleepHoursSettings, AppError> {
    let db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    Ok(sleep_hours::read_sleep_hours_settings(&db)?)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSleepHoursInput {
    pub enabled: bool,
    pub start_hm: String,
    pub end_hm: String,
}

#[tauri::command]
pub fn save_sleep_hours_settings(
    state: State<'_, AppState>,
    input: SaveSleepHoursInput,
) -> Result<SleepHoursSettings, AppError> {
    let mut db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let conn = &mut *db;
    let settings = sleep_hours::save_sleep_hours_settings(
        conn,
        input.enabled,
        &input.start_hm,
        &input.end_hm,
    )?;
    repo::reschedule_next_ping_for_sleep_change(conn)?;
    let mut rng = rand::thread_rng();
    repo::ensure_next_ping_scheduled(conn, unix_now(), &mut rng)?;
    Ok(settings)
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
