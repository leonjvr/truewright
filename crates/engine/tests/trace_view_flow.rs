//! html-trace-viewer spec: a trace captured with console/exception/action
//! entries plus a `browser_screenshot` call taken while it was active
//! renders to a self-contained HTML file with everything visible,
//! including the screenshot embedded inline. Skips (not fails) when no
//! browser is installed, matching the other integration tests'
//! convention.

use engine::Session;

fn fixture_url() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/console_fixture.html");
    let mut normalized = path.to_string_lossy().replace('\\', "/");
    if !normalized.starts_with('/') {
        normalized = format!("/{normalized}");
    }
    format!("file://{normalized}")
}

#[tokio::test]
async fn a_captured_trace_with_a_screenshot_renders_to_readable_html() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!(
            "skipping a_captured_trace_with_a_screenshot_renders_to_readable_html: no installed browser found"
        );
        return;
    }

    let session = Session::launch("engine-test-trace-view", true)
        .await
        .expect("session launches");

    let capture = session
        .console_capture_start("trace-view-flow-test")
        .await
        .expect("console capture starts");

    session.navigate(&fixture_url()).await.expect("navigate succeeds");
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let screenshot_bytes = session.screenshot().await.expect("screenshot succeeds");
    assert!(!screenshot_bytes.is_empty(), "screenshot should not be empty");

    let summary = capture.stop().await.expect("console capture stops");

    let raw = std::fs::read_to_string(&summary.path).expect("read trace file");
    let has_screenshot_entry = raw.lines().any(|l| l.contains("\"type\":\"screenshot\""));
    assert!(
        has_screenshot_entry,
        "trace should contain a screenshot entry: {raw}"
    );

    let html_path = engine::render_trace_html("trace-view-flow-test").expect("renders to HTML");
    assert!(html_path.exists(), "rendered HTML file should exist");

    let html = std::fs::read_to_string(&html_path).expect("read rendered HTML");
    assert!(html.contains("log message"), "console entry missing: {html}");
    assert!(html.contains("boom"), "exception entry missing: {html}");
    assert!(html.contains("navigate"), "action entry missing: {html}");
    assert!(
        html.contains("data:image/png;base64,"),
        "screenshot should be embedded inline as a data URI: {html}"
    );
    assert!(html.contains("row screenshot"), "screenshot row styling missing: {html}");

    session.close().await.expect("session closes");
}
