//! History report building and file export (CSV, XLSX, HTML for print).

use crate::db::repo;
use crate::error::AppError;
use rusqlite::Connection;
use rust_xlsxwriter::{Format, Workbook};
use std::path::Path;

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

#[derive(Debug, Clone)]
pub struct HistoryReport {
    pub since_unix: i64,
    pub until_unix: i64,
    pub generated_at_unix: i64,
    pub overall: Vec<OverallRow>,
    pub timeline: Vec<TimelineRow>,
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
    let timeline: Vec<TimelineRow> = captures
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

    Ok(HistoryReport {
        since_unix: since,
        until_unix: until,
        generated_at_unix,
        overall,
        timeline,
    })
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
        "# Period unix: {} .. {}\n",
        report.since_unix, report.until_unix
    ));
    out.push_str(&format!(
        "# Generated unix: {}\n\n",
        report.generated_at_unix
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
    out.push_str("activity,body,start_unix,end_unix,duration_minutes\n");
    for row in &report.timeline {
        let dur = row
            .duration_minutes
            .map(|d| d.to_string())
            .unwrap_or_else(|| String::new());
        out.push_str(&format!(
            "{},{},{},{},{}\n",
            csv_escape(&row.activity),
            csv_escape(&row.body),
            row.started_at_unix,
            row.end_at_unix,
            dur
        ));
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

    let timeline_sheet = workbook.add_worksheet();
    timeline_sheet.set_name("Timeline")?;
    timeline_sheet.write_string_with_format(0, 0, "Activity", &bold)?;
    timeline_sheet.write_string_with_format(0, 1, "Capture text", &bold)?;
    timeline_sheet.write_string_with_format(0, 2, "Start (unix)", &bold)?;
    timeline_sheet.write_string_with_format(0, 3, "End (unix)", &bold)?;
    timeline_sheet.write_string_with_format(0, 4, "Duration (min)", &bold)?;
    for (i, row) in report.timeline.iter().enumerate() {
        let r = (i + 1) as u32;
        timeline_sheet.write_string(r, 0, &row.activity)?;
        timeline_sheet.write_string(r, 1, &row.body)?;
        timeline_sheet.write_number(r, 2, row.started_at_unix as f64)?;
        timeline_sheet.write_number(r, 3, row.end_at_unix as f64)?;
        if let Some(d) = row.duration_minutes {
            timeline_sheet.write_number(r, 4, d as f64)?;
        }
    }

    workbook.save(path)?;
    Ok(())
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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
    let mut timeline_rows = String::new();
    for row in &report.timeline {
        let dur = row
            .duration_minutes
            .map(|d| d.to_string())
            .unwrap_or_else(|| "—".to_string());
        timeline_rows.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td></tr>\n",
            html_escape(&row.activity),
            html_escape(&row.body),
            row.started_at_unix,
            row.end_at_unix,
            dur
        ));
    }

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8"/>
<title>AreYouFocused — History report</title>
<style>
  body {{ font-family: system-ui, sans-serif; color: #134e4a; margin: 1.5rem; }}
  h1 {{ font-size: 1.25rem; margin-bottom: 0.25rem; }}
  .meta {{ font-size: 0.85rem; color: #475569; margin-bottom: 1.5rem; }}
  h2 {{ font-size: 1rem; margin-top: 1.5rem; border-bottom: 1px solid #99f6e4; padding-bottom: 0.25rem; }}
  table {{ width: 100%; border-collapse: collapse; font-size: 0.85rem; margin-top: 0.5rem; }}
  th, td {{ text-align: left; padding: 0.35rem 0.5rem; border-bottom: 1px solid #e2e8f0; vertical-align: top; }}
  th {{ background: #f0fdfa; }}
  .num {{ text-align: right; font-variant-numeric: tabular-nums; }}
  @media print {{ body {{ margin: 0.75rem; }} }}
</style>
</head>
<body>
<h1>AreYouFocused — History report</h1>
<p class="meta">Period (unix): {since} … {until} · Generated (unix): {gen}</p>
<h2>Overall — time by activity</h2>
<table>
<thead><tr><th>Activity</th><th class="num">Minutes</th><th class="num">%</th><th class="num">Saves</th></tr></thead>
<tbody>
{overall_rows}
</tbody>
</table>
<h2>Timeline — what happened when</h2>
<table>
<thead><tr><th>Activity</th><th>Capture</th><th class="num">Start</th><th class="num">End</th><th class="num">Min</th></tr></thead>
<tbody>
{timeline_rows}
</tbody>
</table>
</body>
</html>"#,
        since = report.since_unix,
        until = report.until_unix,
        gen = report.generated_at_unix,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_memory;
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
    }
}
