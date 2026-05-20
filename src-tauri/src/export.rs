//! History report building and file export (CSV, XLSX, HTML for print).

use crate::db::repo;
use crate::error::AppError;
use chrono::{Local, NaiveDate, TimeZone, Timelike};
use std::collections::BTreeMap;
use rusqlite::Connection;
use rust_xlsxwriter::{Chart, ChartType, Format, Workbook};
use std::path::Path;

/// Teal / orange palette aligned with app exports (print-safe solids).
const ACTIVITY_COLORS: &[&str] = &[
    "#0d9488", "#14b8a6", "#2dd4bf", "#0f766e", "#f97316", "#fb923c", "#ea580c", "#fdba74",
    "#2563eb", "#6366f1",
];

fn activity_color(activity: &str) -> &'static str {
    let mut h: u64 = 0;
    for b in activity.bytes() {
        h = h.wrapping_mul(31).wrapping_add(u64::from(b));
    }
    ACTIVITY_COLORS[(h as usize) % ACTIVITY_COLORS.len()]
}

fn format_unix_local(unix: i64) -> String {
    match Local.timestamp_opt(unix, 0) {
        chrono::LocalResult::Single(dt) => dt.format("%d/%m/%Y %H:%M").to_string(),
        chrono::LocalResult::Ambiguous(dt, _) => dt.format("%d/%m/%Y %H:%M").to_string(),
        chrono::LocalResult::None => unix.to_string(),
    }
}

#[derive(Debug, Clone)]
pub struct OverallRow {
    pub activity: String,
    pub total_minutes: i64,
    pub capture_count: i64,
    pub percent: f64,
}

#[derive(Debug, Clone)]
pub struct TimelineRow {
    pub activity: String,
    pub body: String,
    pub started_at_unix: i64,
    pub end_at_unix: i64,
    pub duration_minutes: Option<i64>,
}

/// Timeline rows for one local calendar day (header + chronological segments).
#[derive(Debug, Clone)]
pub struct TimelineDaySection {
    pub header: String,
    pub rows: Vec<TimelineRow>,
}

#[derive(Debug, Clone)]
pub struct HistoryReport {
    pub since_unix: i64,
    pub until_unix: i64,
    pub generated_at_unix: i64,
    pub overall: Vec<OverallRow>,
    /// Flat timeline (newest day first; within each day oldest → newest).
    pub timeline: Vec<TimelineRow>,
    /// Same data grouped under local day headers for export layouts.
    pub timeline_by_day: Vec<TimelineDaySection>,
}

fn local_date(unix: i64) -> NaiveDate {
    match Local.timestamp_opt(unix, 0) {
        chrono::LocalResult::Single(dt) => dt.date_naive(),
        chrono::LocalResult::Ambiguous(dt, _) => dt.date_naive(),
        chrono::LocalResult::None => NaiveDate::from_ymd_opt(1970, 1, 1).unwrap_or_default(),
    }
}

fn format_day_header(date: NaiveDate) -> String {
    date.format("%A, %d %B %Y").to_string()
}

fn local_minutes_since_midnight(unix: i64) -> i32 {
    match Local.timestamp_opt(unix, 0) {
        chrono::LocalResult::Single(dt) => (dt.hour() as i32) * 60 + dt.minute() as i32,
        chrono::LocalResult::Ambiguous(dt, _) => (dt.hour() as i32) * 60 + dt.minute() as i32,
        chrono::LocalResult::None => 0,
    }
}

fn format_hm(minutes: i32) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

/// Wall-clock end minute for layout (minimum 15 min width for point-in-time captures).
fn segment_end_minutes(row: &TimelineRow) -> i32 {
    let start = local_minutes_since_midnight(row.started_at_unix);
    let mut end = if row.end_at_unix > row.started_at_unix {
        local_minutes_since_midnight(row.end_at_unix)
    } else if let Some(d) = row.duration_minutes {
        start + d as i32
    } else {
        start
    };
    // Same-day axis: segment crossing midnight caps at 24:00.
    if end < start {
        end = 24 * 60;
    }
    end.max(start + 15).min(24 * 60)
}

fn format_time_range(row: &TimelineRow) -> String {
    let start = format_hm(local_minutes_since_midnight(row.started_at_unix));
    let end = format_hm(segment_end_minutes(row));
    format!("{start}–{end}")
}

fn timeline_display_text(row: &TimelineRow) -> &str {
    let body = row.body.trim();
    if body.is_empty() {
        row.activity.trim()
    } else {
        body
    }
}

fn timeline_detail_differs(row: &TimelineRow) -> bool {
    let body = row.body.trim();
    !body.is_empty() && body != row.activity.trim()
}

/// Newest calendar day first; within each day segments are oldest → newest.
fn group_timeline_by_day(rows: Vec<TimelineRow>) -> (Vec<TimelineRow>, Vec<TimelineDaySection>) {
    let mut by_day: BTreeMap<NaiveDate, Vec<TimelineRow>> = BTreeMap::new();
    for row in rows {
        by_day.entry(local_date(row.started_at_unix)).or_default().push(row);
    }

    let mut flat = Vec::new();
    let mut sections = Vec::new();
    for (date, day_rows) in by_day.into_iter().rev() {
        flat.extend(day_rows.iter().cloned());
        sections.push(TimelineDaySection {
            header: format_day_header(date),
            rows: day_rows,
        });
    }
    (flat, sections)
}

pub fn build_history_report(
    conn: &Connection,
    since_unix: i64,
    until_unix: i64,
    generated_at_unix: i64,
) -> Result<HistoryReport, AppError> {
    let since = since_unix.min(until_unix);
    let until = since_unix.max(until_unix);

    let digest = repo::query_activity_digest(conn, since)?;
    let total: i64 = digest.iter().map(|(_, m, _)| m).sum();
    let overall: Vec<OverallRow> = digest
        .into_iter()
        .map(|(activity, total_minutes, capture_count)| {
            let percent = if total > 0 {
                (total_minutes as f64 / total as f64) * 100.0
            } else {
                0.0
            };
            OverallRow {
                activity,
                total_minutes,
                capture_count,
                percent,
            }
        })
        .collect();

    let captures = repo::query_captures_in_range(conn, since, until)?;
    let raw_timeline: Vec<TimelineRow> = captures
        .into_iter()
        .map(|(body, started_at_unix, duration_minutes, activity)| {
            let end_at_unix = duration_minutes
                .map(|d| started_at_unix.saturating_add(d.saturating_mul(60)))
                .unwrap_or(started_at_unix);
            TimelineRow {
                activity,
                body,
                started_at_unix,
                end_at_unix,
                duration_minutes,
            }
        })
        .collect();
    let (timeline, timeline_by_day) = group_timeline_by_day(raw_timeline);

    Ok(HistoryReport {
        since_unix: since,
        until_unix: until,
        generated_at_unix,
        overall,
        timeline,
        timeline_by_day,
    })
}

fn write_timeline_row_csv(out: &mut String, row: &TimelineRow) {
    let dur = row
        .duration_minutes
        .map(|d| d.to_string())
        .unwrap_or_default();
    out.push_str(&format!(
        "{},{},{},{}\n",
        csv_escape(timeline_display_text(row)),
        csv_escape(&format_unix_local(row.started_at_unix)),
        csv_escape(&format_unix_local(row.end_at_unix)),
        dur
    ));
}

fn csv_escape(field: &str) -> String {
    if field.contains(['"', ',', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

pub fn report_to_csv(report: &HistoryReport) -> String {
    let mut out = String::new();
    out.push_str("# AreYouFocused history export\n");
    out.push_str(&format!(
        "# Period: {} .. {}\n",
        format_unix_local(report.since_unix),
        format_unix_local(report.until_unix)
    ));
    out.push_str(&format!(
        "# Generated: {}\n\n",
        format_unix_local(report.generated_at_unix)
    ));

    out.push_str("SECTION,OVERALL\n");
    out.push_str("activity,total_minutes,percent,capture_count\n");
    for row in &report.overall {
        out.push_str(&format!(
            "{},{},{:.1},{}\n",
            csv_escape(&row.activity),
            row.total_minutes,
            row.percent,
            row.capture_count
        ));
    }

    out.push_str("\nSECTION,TIMELINE\n");
    out.push_str("activity,start,end,duration_minutes\n");
    for day in &report.timeline_by_day {
        out.push_str(&format!("DAY,{}\n", csv_escape(&day.header)));
        for row in &day.rows {
            write_timeline_row_csv(&mut out, row);
        }
    }
    out
}

pub fn write_xlsx(report: &HistoryReport, path: &Path) -> Result<(), AppError> {
    write_xlsx_inner(report, path).map_err(|e| AppError::Export(e.to_string()))
}

fn write_xlsx_inner(
    report: &HistoryReport,
    path: &Path,
) -> Result<(), rust_xlsxwriter::XlsxError> {
    let mut workbook = Workbook::new();
    let bold = Format::new().set_bold();

    let overall_sheet = workbook.add_worksheet();
    overall_sheet.set_name("Overall")?;
    overall_sheet.write_string_with_format(0, 0, "Activity", &bold)?;
    overall_sheet.write_string_with_format(0, 1, "Total minutes", &bold)?;
    overall_sheet.write_string_with_format(0, 2, "Percent", &bold)?;
    overall_sheet.write_string_with_format(0, 3, "Saves", &bold)?;
    for (i, row) in report.overall.iter().enumerate() {
        let r = (i + 1) as u32;
        overall_sheet.write_string(r, 0, &row.activity)?;
        overall_sheet.write_number(r, 1, row.total_minutes as f64)?;
        overall_sheet.write_number(r, 2, row.percent)?;
        overall_sheet.write_number(r, 3, row.capture_count as f64)?;
    }
    overall_sheet.autofit();

    if !report.overall.is_empty() {
        let last = report.overall.len() as u32;
        let cat_range = format!("Overall!$A$2:$A${}", last + 1);
        let val_range = format!("Overall!$B$2:$B${}", last + 1);
        let mut chart = Chart::new(ChartType::Bar);
        chart
            .add_series()
            .set_categories(&cat_range)
            .set_values(&val_range);
        chart.title().set_name("Minutes by activity");
        chart.x_axis().set_name("Activity");
        chart.y_axis().set_name("Minutes");
        overall_sheet.insert_chart_with_offset(last + 2, 0, &chart, 20, 8)?;
    }

    let timeline_sheet = workbook.add_worksheet();
    timeline_sheet.set_name("Timeline")?;
    timeline_sheet.write_string_with_format(0, 0, "Activity", &bold)?;
    timeline_sheet.write_string_with_format(0, 1, "Start", &bold)?;
    timeline_sheet.write_string_with_format(0, 2, "End", &bold)?;
    timeline_sheet.write_string_with_format(0, 3, "Duration (min)", &bold)?;
    let day_header_fmt = Format::new().set_bold().set_background_color("#E0F2F1");
    let mut r: u32 = 1;
    for day in &report.timeline_by_day {
        timeline_sheet.write_string_with_format(r, 0, &day.header, &day_header_fmt)?;
        r += 1;
        for row in &day.rows {
            timeline_sheet.write_string(r, 0, timeline_display_text(row))?;
            timeline_sheet.write_string(r, 1, &format_unix_local(row.started_at_unix))?;
            timeline_sheet.write_string(r, 2, &format_unix_local(row.end_at_unix))?;
            if let Some(d) = row.duration_minutes {
                timeline_sheet.write_number(r, 3, d as f64)?;
            }
            r += 1;
        }
    }
    timeline_sheet.autofit();

    write_timeline_visual_sheet(&mut workbook, report)?;

    workbook.save(path)?;
    Ok(())
}

/// Hour-grid “calendar strip” per day (colored cells = activity blocks).
fn write_timeline_visual_sheet(
    workbook: &mut Workbook,
    report: &HistoryReport,
) -> Result<(), rust_xlsxwriter::XlsxError> {
    let sheet = workbook.add_worksheet();
    sheet.set_name("Timeline visual")?;

    let bold = Format::new().set_bold();
    let day_fmt = Format::new().set_bold().set_background_color("#CCFBF1");
    let header_fmt = Format::new().set_bold().set_background_color("#F0FDFA");

    sheet.write_string_with_format(0, 0, "Day agenda (vertical list)", &bold)?;
    sheet.write_string_with_format(
        1,
        0,
        "Chronological segments per local day — time range, activity, and duration.",
        &Format::new().set_font_color("#475569"),
    )?;

    let mut row: u32 = 3;
    for day in &report.timeline_by_day {
        if day.rows.is_empty() {
            continue;
        }
        sheet.write_string_with_format(row, 0, &day.header, &day_fmt)?;
        row += 1;
        sheet.write_string_with_format(row, 0, "Time", &header_fmt)?;
        sheet.write_string_with_format(row, 1, "Activity", &header_fmt)?;
        sheet.write_string_with_format(row, 2, "Min", &header_fmt)?;
        row += 1;

        for seg in &day.rows {
            let stripe = Format::new().set_background_color(activity_color(&seg.activity));
            let time_range = format_time_range(seg);
            sheet.write_string_with_format(row, 0, &time_range, &stripe)?;
            sheet.write_string_with_format(row, 1, timeline_display_text(seg), &stripe)?;
            if let Some(d) = seg.duration_minutes {
                sheet.write_number(row, 2, d as f64)?;
            }
            row += 1;
        }
        row += 1;
    }

    sheet.set_column_width(0, 14)?;
    sheet.set_column_width(1, 48)?;
    sheet.set_column_width(2, 8)?;
    Ok(())
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn render_day_timeline_html(day: &TimelineDaySection) -> String {
    if day.rows.is_empty() {
        return String::new();
    }

    let mut items = String::new();
    for row in &day.rows {
        let color = activity_color(&row.activity);
        let time_range = format_time_range(row);
        let dur_badge = row
            .duration_minutes
            .map(|d| format!(r#"<span class="agenda-dur">{d} min</span>"#))
            .unwrap_or_default();
        let thread_note = if timeline_detail_differs(row) {
            format!(
                r#"<p class="agenda-thread">Grouped as {}</p>"#,
                html_escape(&row.activity)
            )
        } else {
            String::new()
        };
        items.push_str(&format!(
            r#"<li class="agenda-item">
  <span class="agenda-stripe" style="background:{color}" aria-hidden="true"></span>
  <div class="agenda-time-col">
    <span class="agenda-time">{time}</span>
    {dur}
  </div>
  <div class="agenda-content">
    <span class="agenda-activity">{act}</span>
    {thread_note}
  </div>
</li>
"#,
            color = color,
            time = time_range,
            dur = dur_badge,
            act = html_escape(timeline_display_text(row)),
            thread_note = thread_note,
        ));
    }

    format!(
        r#"<section class="day-viz">
  <h3 class="day-viz-title">{header}</h3>
  <ol class="day-agenda">{items}</ol>
</section>
"#,
        header = html_escape(&day.header),
        items = items,
    )
}

fn render_overall_bar_chart_svg(overall: &[OverallRow]) -> String {
    if overall.is_empty() {
        return String::new();
    }
    let max_min = overall
        .iter()
        .map(|r| r.total_minutes)
        .max()
        .unwrap_or(1)
        .max(1) as f64;
    let bar_h = 22i32;
    let gap = 10i32;
    let label_w = 120i32;
    let chart_w = 320i32;
    let pad = 12i32;
    let height = overall.len() as i32 * (bar_h + gap) + pad * 2;
    let width = label_w + chart_w + pad * 2;

    let mut bars = String::new();
    for (i, row) in overall.iter().enumerate() {
        let y = pad + i as i32 * (bar_h + gap);
        let w = (row.total_minutes as f64 / max_min * chart_w as f64).max(2.0);
        let color = activity_color(&row.activity);
        let label = if row.activity.len() > 16 {
            format!("{}…", &row.activity[..15])
        } else {
            row.activity.clone()
        };
        bars.push_str(&format!(
            r#"<text x="{pad}" y="{ty}" class="chart-label">{label}</text>
<rect x="{bx}" y="{y}" width="{w:.1}" height="{bar_h}" rx="3" fill="{color}"/>
<text x="{tx}" y="{ty}" class="chart-value">{mins}m</text>
"#,
            pad = pad,
            ty = y + bar_h - 6,
            label = html_escape(&label),
            bx = label_w + pad,
            y = y,
            w = w,
            bar_h = bar_h,
            color = color,
            tx = label_w + pad + w as i32 + 6,
            mins = row.total_minutes,
        ));
    }

    format!(
        r#"<svg class="overall-chart" viewBox="0 0 {width} {height}" width="100%" height="{height}" role="img" aria-label="Minutes by activity">
  {bars}
</svg>
"#,
        width = width,
        height = height,
        bars = bars,
    )
}

pub fn report_to_html(report: &HistoryReport) -> String {
    let mut overall_rows = String::new();
    for row in &report.overall {
        overall_rows.push_str(&format!(
            "<tr><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{:.1}%</td><td class=\"num\">{}</td></tr>\n",
            html_escape(&row.activity),
            row.total_minutes,
            row.percent,
            row.capture_count
        ));
    }
    let overall_chart = render_overall_bar_chart_svg(&report.overall);
    let mut day_viz = String::new();
    for day in &report.timeline_by_day {
        day_viz.push_str(&render_day_timeline_html(day));
    }
    let mut legend = String::new();
    for row in &report.overall {
        legend.push_str(&format!(
            r#"<span class="legend-item"><span class="swatch" style="background:{}"></span>{}</span>"#,
            activity_color(&row.activity),
            html_escape(&row.activity)
        ));
    }

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8"/>
<title>AreYouFocused — History report</title>
<style>
  body {{ font-family: "Segoe UI", system-ui, sans-serif; color: #134e4a; margin: 1.5rem; background: #fff; }}
  h1 {{ font-size: 1.25rem; margin-bottom: 0.25rem; color: #0f766e; }}
  .meta {{ font-size: 0.85rem; color: #475569; margin-bottom: 1.5rem; }}
  h2 {{ font-size: 1rem; margin-top: 1.5rem; border-bottom: 2px solid #99f6e4; padding-bottom: 0.25rem; color: #0f766e; }}
  h3.day-viz-title {{ font-size: 0.95rem; margin: 0 0 0.5rem; color: #115e59; }}
  .hint {{ font-size: 0.8rem; color: #64748b; margin: 0.25rem 0 0.75rem; }}
  .legend {{ display: flex; flex-wrap: wrap; gap: 0.5rem 1rem; margin: 0.5rem 0 1rem; font-size: 0.75rem; }}
  .legend-item {{ display: inline-flex; align-items: center; gap: 0.35rem; }}
  .swatch {{ width: 10px; height: 10px; border-radius: 2px; display: inline-block; }}
  .overall-chart {{ max-width: 520px; margin: 0.5rem 0 1rem; }}
  .chart-label {{ font-size: 11px; fill: #134e4a; }}
  .chart-value {{ font-size: 10px; fill: #64748b; }}
  .day-viz {{ margin: 1.25rem 0 1.5rem; padding: 0.75rem 0.75rem 0.5rem; border: 1px solid #ccfbf1; border-radius: 8px; background: #f8fffe; break-inside: avoid; }}
  .day-agenda {{ list-style: none; margin: 0; padding: 0; }}
  .agenda-item {{ display: grid; grid-template-columns: 4px 5.5rem 1fr; gap: 0 0.75rem; align-items: start; padding: 0.55rem 0; border-bottom: 1px solid #e2e8f0; }}
  .agenda-item:last-child {{ border-bottom: none; }}
  .agenda-stripe {{ display: block; width: 4px; min-height: 2.5rem; border-radius: 2px; align-self: stretch; }}
  .agenda-time-col {{ font-variant-numeric: tabular-nums; }}
  .agenda-time {{ display: block; font-size: 0.8rem; font-weight: 600; color: #0f766e; white-space: nowrap; }}
  .agenda-dur {{ display: inline-block; margin-top: 0.2rem; font-size: 0.68rem; font-weight: 600; color: #ea580c; background: #ffedd5; padding: 0.1rem 0.35rem; border-radius: 4px; }}
  .agenda-content {{ min-width: 0; }}
  .agenda-activity {{ display: block; font-size: 0.88rem; font-weight: 600; color: #134e4a; word-wrap: break-word; overflow-wrap: anywhere; }}
  .agenda-thread {{ margin: 0.25rem 0 0; font-size: 0.75rem; color: #64748b; line-height: 1.35; word-wrap: break-word; overflow-wrap: anywhere; }}
  table {{ width: 100%; border-collapse: collapse; font-size: 0.85rem; margin-top: 0.5rem; }}
  th, td {{ text-align: left; padding: 0.35rem 0.5rem; border-bottom: 1px solid #e2e8f0; vertical-align: top; }}
  th {{ background: #f0fdfa; }}
  tr.day td {{ background: #ccfbf1; font-weight: 600; padding-top: 0.75rem; }}
  .num {{ text-align: right; font-variant-numeric: tabular-nums; }}
  @media print {{
    body {{ margin: 0.75rem; }}
    .day-viz {{ page-break-inside: avoid; }}
    .agenda-stripe, .agenda-dur {{ -webkit-print-color-adjust: exact; print-color-adjust: exact; }}
  }}
</style>
</head>
<body>
<h1>AreYouFocused — History report</h1>
<p class="meta">Period: {since} … {until} · Generated: {gen}</p>
<h2>Overall — time by activity</h2>
{overall_chart}
<div class="legend">{legend}</div>
<table>
<thead><tr><th>Activity</th><th class="num">Minutes</th><th class="num">%</th><th class="num">Saves</th></tr></thead>
<tbody>
{overall_rows}
</tbody>
</table>
<h2>Day timeline — when &amp; what</h2>
<p class="hint">Each day is a vertical agenda: local start–end time, what you logged, and duration. Follow-up segments that roll up under an earlier activity show a grouped-as note.</p>
{day_viz}
</body>
</html>"#,
        since = html_escape(&format_unix_local(report.since_unix)),
        until = html_escape(&format_unix_local(report.until_unix)),
        gen = html_escape(&format_unix_local(report.generated_at_unix)),
        overall_chart = overall_chart,
        legend = legend,
        day_viz = day_viz,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_memory;
    use crate::db::repo::persist_capture;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    #[test]
    fn csv_includes_overall_header() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(1);
        repo::persist_capture(&conn, "work", 1_000, &mut rng, Some(30)).expect("p");
        let report = build_history_report(&conn, 0, 9_999, 9_999).expect("report");
        let csv = report_to_csv(&report);
        assert!(csv.contains("SECTION,OVERALL"));
        assert!(csv.contains("work,30"));
        assert!(csv.contains("activity,start,end,duration_minutes"));
        assert!(!csv.contains("activity,body,start,end,duration_minutes"));
        assert!(!csv.contains("start_unix"));
    }

    #[test]
    fn format_unix_local_uses_day_month_year() {
        // 2026-05-19 16:30:00 UTC — local string must not be raw unix.
        let formatted = format_unix_local(1_748_000_000);
        assert!(!formatted.chars().all(|c| c.is_ascii_digit()));
        assert!(formatted.contains('/'));
    }

    #[test]
    fn timeline_export_groups_by_day_newest_first() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(7);
        let older_day = 1_700_000_000_i64;
        let newer_day = older_day + 86_400 * 2;
        persist_capture(&conn, "first day", older_day, &mut rng, Some(10)).expect("a");
        persist_capture(&conn, "second day", newer_day, &mut rng, None).expect("b");
        let report = build_history_report(&conn, 0, newer_day + 1, newer_day + 1).expect("report");
        assert_eq!(report.timeline.len(), 2);
        assert_eq!(report.timeline[0].body, "second day");
        assert_eq!(report.timeline[1].body, "first day");
        assert_eq!(report.timeline_by_day.len(), 2);
        assert_eq!(report.timeline_by_day[0].rows[0].body, "second day");
        assert_eq!(report.timeline_by_day[1].rows[0].body, "first day");
        let csv = report_to_csv(&report);
        assert!(csv.contains("DAY,"));
        let html = report_to_html(&report);
        assert!(html.contains("day-agenda"));
        assert!(html.contains("day-viz"));
        assert!(html.contains("overall-chart"));
        assert!(!html.contains("seg-block"));
    }

    #[test]
    fn segment_end_minutes_adds_duration_not_hours() {
        use chrono::{Local, TimeZone};

        // 2026-05-19 16:33 local + 15 min → 16:48 (not 31:33 from hour+duration bug).
        let start_dt = Local
            .with_ymd_and_hms(2026, 5, 19, 16, 33, 0)
            .single()
            .expect("valid local datetime");
        let start_unix = start_dt.timestamp();
        let row = TimelineRow {
            activity: "work".into(),
            body: "test".into(),
            started_at_unix: start_unix,
            end_at_unix: start_unix + 15 * 60,
            duration_minutes: Some(15),
        };

        assert_eq!(local_minutes_since_midnight(row.started_at_unix), 16 * 60 + 33);
        assert_eq!(segment_end_minutes(&row), 16 * 60 + 48);
        assert_eq!(format_hm(segment_end_minutes(&row)), "16:48");

        let html = render_day_timeline_html(&TimelineDaySection {
            header: "Tuesday, 19 May 2026".into(),
            rows: vec![row],
        });
        assert!(html.contains("16:33–16:48"), "html agenda: {html}");
        assert!(html.contains("day-agenda"));
        assert!(html.contains("agenda-item"));
    }

    #[test]
    fn timeline_export_uses_capture_text_not_duplicate_columns() {
        let row = TimelineRow {
            activity: "reading".into(),
            body: "still reading".into(),
            started_at_unix: 2000,
            end_at_unix: 2900,
            duration_minutes: Some(15),
        };
        assert_eq!(timeline_display_text(&row), "still reading");
        assert!(timeline_detail_differs(&row));

        let report = HistoryReport {
            since_unix: 0,
            until_unix: 3000,
            generated_at_unix: 3000,
            overall: vec![],
            timeline: vec![row.clone()],
            timeline_by_day: vec![TimelineDaySection {
                header: "Monday, 01 January 2024".into(),
                rows: vec![row],
            }],
        };
        let csv = report_to_csv(&report);
        assert!(csv.contains("still reading,"));
        assert!(!csv.contains("reading,still reading,"));
        let html = report_to_html(&report);
        assert!(html.contains("still reading"));
        assert!(html.contains("Grouped as reading"));
    }

    #[test]
    fn timeline_export_hides_group_note_when_text_matches_activity() {
        let row = TimelineRow {
            activity: "work".into(),
            body: "work".into(),
            started_at_unix: 1000,
            end_at_unix: 2800,
            duration_minutes: Some(30),
        };
        assert!(!timeline_detail_differs(&row));
        let html = render_day_timeline_html(&TimelineDaySection {
            header: "Monday, 01 January 2024".into(),
            rows: vec![row],
        });
        assert!(html.contains("work"));
        assert!(!html.contains("Grouped as"));
    }

    #[test]
    fn xlsx_includes_visual_sheet_and_chart() {
        let conn = open_memory().expect("db");
        let mut rng = StdRng::seed_from_u64(99);
        repo::persist_capture(&conn, "deep work", 1_700_010_000, &mut rng, Some(45)).expect("p");
        let report = build_history_report(&conn, 0, 1_800_000_000, 1_800_000_000).expect("report");
        let dir = std::env::temp_dir();
        let path = dir.join(format!("ayf_export_test_{}.xlsx", std::process::id()));
        write_xlsx(&report, &path).expect("xlsx");
        assert!(path.exists());
        let _ = std::fs::remove_file(&path);
    }
}
