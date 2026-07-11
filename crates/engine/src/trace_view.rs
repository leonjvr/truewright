//! Renders a saved console/action trace as a single, self-contained HTML
//! file (html-trace-viewer spec).

use crate::console::{load_trace, TraceEntry};
use crate::error::{EngineError, Result};
use base64::Engine;
use std::fmt::Write as _;
use std::path::PathBuf;

/// Loads `name`'s saved trace, renders it, writes `<name>.html` next to
/// the `.jsonl` trace, and returns the output path -- the one function
/// both `aib trace view` and `browser_render_trace` call into.
// EngineError is kept as one flat enum (matches cdp::CdpError's
// rationale); see the identical allow in cdp/src/launch.rs.
#[allow(clippy::result_large_err)]
pub fn render_trace_html(name: &str) -> Result<PathBuf> {
    let entries = load_trace(name)?;
    let html = render_html(&entries)?;

    let path = crate::console::trace_html_path(name)?;
    std::fs::write(&path, html)
        .map_err(|e| EngineError::Console(format!("failed to write {}: {e}", path.display())))?;

    Ok(path)
}

/// Renders a trace's entries as a self-contained HTML page: chronologically
/// sorted (never trusted to already be in order -- console/exception
/// entries arrive via an async collector racing against directly-logged
/// action/screenshot entries), color-coded by kind, screenshots embedded
/// as base64 data URIs so the file needs nothing alongside it to be
/// readable (design.md Decisions #3/#6).
#[allow(clippy::result_large_err)]
pub fn render_html(entries: &[TraceEntry]) -> Result<String> {
    let mut sorted: Vec<&TraceEntry> = entries.iter().collect();
    sorted.sort_by(|a, b| timestamp(a).partial_cmp(&timestamp(b)).unwrap());

    let start = sorted.first().map(|e| timestamp(e)).unwrap_or(0.0);

    let mut rows = String::new();
    for entry in &sorted {
        write_row(&mut rows, entry, start)?;
    }

    let mut out = String::new();
    let _ = write!(
        out,
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>aib trace</title>
<style>
  body {{ font-family: system-ui, sans-serif; background: #1e1e1e; color: #ddd; margin: 0; padding: 24px; }}
  h1 {{ font-size: 16px; font-weight: 600; color: #fff; margin: 0 0 16px; }}
  .row {{ display: flex; gap: 12px; padding: 8px 12px; border-left: 3px solid #444; margin-bottom: 4px; background: #262626; border-radius: 4px; align-items: flex-start; }}
  .t {{ color: #888; font-family: ui-monospace, monospace; font-size: 12px; width: 70px; flex-shrink: 0; padding-top: 2px; }}
  .kind {{ font-family: ui-monospace, monospace; font-size: 11px; font-weight: 700; text-transform: uppercase; padding: 2px 6px; border-radius: 3px; flex-shrink: 0; height: fit-content; }}
  .text {{ white-space: pre-wrap; word-break: break-word; font-family: ui-monospace, monospace; font-size: 13px; }}
  .row.action {{ border-left-color: #4a9eff; }} .kind.action {{ background: #1a3a5c; color: #7cc0ff; }}
  .row.console {{ border-left-color: #888; }} .kind.console {{ background: #333; color: #bbb; }}
  .row.exception {{ border-left-color: #ff5c5c; }} .kind.exception {{ background: #4a1a1a; color: #ff8a8a; }}
  .row.screenshot {{ border-left-color: #b98eff; }} .kind.screenshot {{ background: #3a2a5c; color: #d4b8ff; }}
  .row.fail {{ border-left-color: #ff5c5c; }} .row.fail .kind {{ background: #4a1a1a; color: #ff8a8a; }}
  img {{ max-width: 480px; border-radius: 4px; margin-top: 6px; display: block; border: 1px solid #444; }}
</style>
</head>
<body>
<h1>aib trace &mdash; {count} entries</h1>
{rows}
</body>
</html>
"#,
        count = sorted.len(),
    );

    Ok(out)
}

fn timestamp(entry: &TraceEntry) -> f64 {
    match entry {
        TraceEntry::Console { timestamp_ms, .. }
        | TraceEntry::Exception { timestamp_ms, .. }
        | TraceEntry::Action { timestamp_ms, .. }
        | TraceEntry::Screenshot { timestamp_ms, .. } => *timestamp_ms,
    }
}

#[allow(clippy::result_large_err)]
fn write_row(out: &mut String, entry: &TraceEntry, start: f64) -> Result<()> {
    let elapsed_ms = (timestamp(entry) - start).max(0.0) as u64;
    let elapsed = format!("+{}ms", elapsed_ms);

    match entry {
        TraceEntry::Console { level, text, .. } => {
            let _ = writeln!(
                out,
                r#"<div class="row console"><span class="t">{elapsed}</span><span class="kind console">{level}</span><span class="text">{text}</span></div>"#,
                level = html_escape(level),
                text = html_escape(text),
            );
        }
        TraceEntry::Exception { text, .. } => {
            let _ = writeln!(
                out,
                r#"<div class="row exception"><span class="t">{elapsed}</span><span class="kind exception">exception</span><span class="text">{text}</span></div>"#,
                text = html_escape(text),
            );
        }
        TraceEntry::Action { text, .. } => {
            let is_failed_assert = text.starts_with("assert ") && text.ends_with("fail");
            let row_class = if is_failed_assert { "row action fail" } else { "row action" };
            let _ = writeln!(
                out,
                r#"<div class="{row_class}"><span class="t">{elapsed}</span><span class="kind action">action</span><span class="text">{text}</span></div>"#,
                text = html_escape(text),
            );
        }
        TraceEntry::Screenshot { path, .. } => {
            let data_uri = screenshot_data_uri(path)?;
            let _ = writeln!(
                out,
                r#"<div class="row screenshot"><span class="t">{elapsed}</span><span class="kind screenshot">screenshot</span><div><img src="{data_uri}" alt="screenshot"></div></div>"#,
            );
        }
    }
    Ok(())
}

#[allow(clippy::result_large_err)]
fn screenshot_data_uri(path: &str) -> Result<String> {
    let bytes = std::fs::read(path)
        .map_err(|e| EngineError::Console(format!("failed to read screenshot {path}: {e}")))?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Ok(format!("data:image/png;base64,{encoded}"))
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_render_sorted_by_timestamp_regardless_of_input_order() {
        let entries = vec![
            TraceEntry::Action { text: "second".into(), timestamp_ms: 200.0 },
            TraceEntry::Console { level: "log".into(), text: "first".into(), timestamp_ms: 100.0 },
        ];
        let html = render_html(&entries).expect("renders");
        let first_pos = html.find("first").expect("first present");
        let second_pos = html.find("second").expect("second present");
        assert!(first_pos < second_pos, "entries should render in chronological order");
    }

    #[test]
    fn entry_kinds_get_distinct_styling_classes() {
        let entries = vec![
            TraceEntry::Console { level: "log".into(), text: "c".into(), timestamp_ms: 0.0 },
            TraceEntry::Exception { text: "e".into(), timestamp_ms: 1.0 },
            TraceEntry::Action { text: "a".into(), timestamp_ms: 2.0 },
        ];
        let html = render_html(&entries).expect("renders");
        assert!(html.contains("row console"));
        assert!(html.contains("row exception"));
        assert!(html.contains("row action"));
    }

    #[test]
    fn failed_assert_actions_get_a_distinct_fail_class() {
        let entries = vec![TraceEntry::Action {
            text: "assert text=\"X\" present=true => fail".into(),
            timestamp_ms: 0.0,
        }];
        let html = render_html(&entries).expect("renders");
        assert!(html.contains("row action fail"));
    }

    #[test]
    fn text_is_html_escaped() {
        let entries = vec![TraceEntry::Console {
            level: "log".into(),
            text: "<script>alert(1)</script>".into(),
            timestamp_ms: 0.0,
        }];
        let html = render_html(&entries).expect("renders");
        assert!(!html.contains("<script>alert"));
        assert!(html.contains("&lt;script&gt;"));
    }
}
