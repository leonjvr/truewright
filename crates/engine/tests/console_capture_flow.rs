//! End-to-end console capture test: a fixture that logs at multiple
//! console levels and throws an uncaught exception produces a JSONL trace
//! with matching entries in chronological order (console-capture spec).
//! Skips (not fails) when no browser is installed, matching the other
//! integration tests' convention.

use engine::Session;
use std::time::Duration;

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
async fn console_and_exception_capture_produces_a_chronological_jsonl_trace() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!(
            "skipping console_and_exception_capture_produces_a_chronological_jsonl_trace: no installed browser found"
        );
        return;
    }

    let session = Session::launch("engine-test-console-capture", true)
        .await
        .expect("session launches");

    let capture = session
        .console_capture_start("console-flow-test")
        .await
        .expect("console capture starts");

    session.navigate(&fixture_url()).await.expect("navigate succeeds");
    // The fixture's uncaught exception fires from a setTimeout(0), a real
    // tick after the synchronous console.* calls -- give it a moment to
    // actually fire before stopping capture.
    tokio::time::sleep(Duration::from_millis(300)).await;

    let summary = capture.stop().await.expect("console capture stops");
    assert_eq!(summary.name, "console-flow-test");
    // 3 console calls + 1 uncaught exception + 1 navigate action entry
    // (the `navigate` call above is itself logged into this same trace, per
    // the action-trace change -- see action_trace_flow.rs for that
    // behavior's own dedicated coverage; this test cares about the
    // console/exception entries specifically, so it filters those out).
    assert_eq!(summary.entry_count, 5);

    let raw = std::fs::read_to_string(&summary.path).expect("read trace file");
    let entries: Vec<serde_json::Value> = raw
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("valid JSON line"))
        .filter(|e: &serde_json::Value| e["type"] != "action")
        .collect();
    assert_eq!(entries.len(), 4, "trace file should have one console/exception entry per call");

    assert_eq!(entries[0]["type"], "console");
    assert_eq!(entries[0]["level"], "log");
    assert_eq!(entries[0]["text"], "log message");

    assert_eq!(entries[1]["type"], "console");
    // console.warn's CDP console-api type is "warning", not "warn".
    assert_eq!(entries[1]["level"], "warning");
    assert_eq!(entries[1]["text"], "warn message");

    assert_eq!(entries[2]["type"], "console");
    assert_eq!(entries[2]["level"], "error");
    assert_eq!(entries[2]["text"], "error message");

    assert_eq!(entries[3]["type"], "exception");
    assert!(
        entries[3]["text"].as_str().unwrap_or("").contains("boom"),
        "exception entry should mention the thrown error's message: {:?}",
        entries[3]["text"]
    );

    // Chronological order: the three synchronous console calls, in the
    // order the script issued them, then the async exception last.
    let t0 = entries[0]["timestamp_ms"].as_f64().unwrap();
    let t1 = entries[1]["timestamp_ms"].as_f64().unwrap();
    let t2 = entries[2]["timestamp_ms"].as_f64().unwrap();
    let t3 = entries[3]["timestamp_ms"].as_f64().unwrap();
    assert!(t0 <= t1 && t1 <= t2 && t2 <= t3, "entries should be in chronological order: {t0}, {t1}, {t2}, {t3}");

    session.close().await.expect("session closes");
}
