//! End-to-end browser_assert test: a passing assertion succeeds, a failing
//! one returns a typed AssertionFailed error (not a panic), and both are
//! logged into an active trace as action entries with the correct pass/fail
//! outcome (browser-assert spec). Skips (not fails) when no browser is
//! installed, matching the other integration tests' convention.

use engine::{EngineError, Session};

fn fixture_url() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/form.html");
    let mut normalized = path.to_string_lossy().replace('\\', "/");
    if !normalized.starts_with('/') {
        normalized = format!("/{normalized}");
    }
    format!("file://{normalized}")
}

#[tokio::test]
async fn passing_and_failing_assertions_behave_correctly() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!("skipping passing_and_failing_assertions_behave_correctly: no installed browser found");
        return;
    }

    let session = Session::launch("engine-test-assert", true)
        .await
        .expect("session launches");
    session.navigate(&fixture_url()).await.expect("navigate succeeds");

    // Present, expected present -> pass.
    session
        .assert_text("Sign up", true)
        .await
        .expect("text known to be present should assert present successfully");

    // Absent, expected absent -> pass.
    session
        .assert_text("definitely-not-on-this-page-xyz", false)
        .await
        .expect("text known to be absent should assert absent successfully");

    // Absent, expected present -> fail.
    match session.assert_text("definitely-not-on-this-page-xyz", true).await {
        Err(EngineError::AssertionFailed { text, present, .. }) => {
            assert_eq!(text, "definitely-not-on-this-page-xyz");
            assert!(present);
        }
        other => panic!("expected AssertionFailed, got {other:?}"),
    }

    // Present, expected absent -> fail.
    match session.assert_text("Sign up", false).await {
        Err(EngineError::AssertionFailed { text, present, .. }) => {
            assert_eq!(text, "Sign up");
            assert!(!present);
        }
        other => panic!("expected AssertionFailed, got {other:?}"),
    }

    session.close().await.expect("session closes");
}

#[tokio::test]
async fn assertions_are_logged_into_the_active_trace_with_their_outcome() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!(
            "skipping assertions_are_logged_into_the_active_trace_with_their_outcome: no installed browser found"
        );
        return;
    }

    let session = Session::launch("engine-test-assert-trace", true)
        .await
        .expect("session launches");

    let capture = session
        .console_capture_start("assert-flow-test")
        .await
        .expect("console capture starts");

    session.navigate(&fixture_url()).await.expect("navigate succeeds");
    session
        .assert_text("Sign up", true)
        .await
        .expect("passing assertion succeeds");
    let _ = session.assert_text("nope-not-here", true).await; // expected to fail; outcome checked via trace below

    let summary = capture.stop().await.expect("console capture stops");

    let raw = std::fs::read_to_string(&summary.path).expect("read trace file");
    let entries: Vec<serde_json::Value> = raw
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("valid JSON line"))
        .collect();

    let assert_entries: Vec<&serde_json::Value> = entries
        .iter()
        .filter(|e| e["type"] == "action" && e["text"].as_str().unwrap_or("").starts_with("assert "))
        .collect();
    assert_eq!(assert_entries.len(), 2, "expected one action entry per assert call: {entries:#?}");
    assert!(
        assert_entries[0]["text"].as_str().unwrap().contains("pass"),
        "first assertion (passing) should be logged as pass: {:?}",
        assert_entries[0]
    );
    assert!(
        assert_entries[1]["text"].as_str().unwrap().contains("fail"),
        "second assertion (failing) should be logged as fail: {:?}",
        assert_entries[1]
    );

    session.close().await.expect("session closes");
}
