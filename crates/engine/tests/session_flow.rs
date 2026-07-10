//! End-to-end engine test against a local HTML fixture: navigate, snapshot,
//! type, click, wait_for, screenshot, plus stale-ref/unknown-key error
//! paths. Skips (not fails) when no browser is installed, matching Phase
//! 0's integration-test convention.

use engine::{EngineError, Session};
use std::time::Duration;

fn fixture_url() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/form.html");
    let mut normalized = path.to_string_lossy().replace('\\', "/");
    if !normalized.starts_with('/') {
        normalized = format!("/{normalized}");
    }
    format!("file://{normalized}")
}

/// Finds the ref on the (single) line whose role prefix and accessible name
/// both match, so this doesn't accidentally grab an ancestor landmark whose
/// name happens to contain the same substring.
fn find_ref(snapshot: &str, role: &str, name_substr: &str) -> Option<String> {
    for line in snapshot.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with(&format!("- {role}")) {
            continue;
        }
        if !line.contains(name_substr) {
            continue;
        }
        let start = line.rfind('[')?;
        let end = line.rfind(']')?;
        if end > start {
            return Some(line[start + 1..end].to_string());
        }
    }
    None
}

#[tokio::test]
async fn navigate_snapshot_type_click_wait_screenshot() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!(
            "skipping navigate_snapshot_type_click_wait_screenshot: no installed browser found"
        );
        return;
    }

    let session = Session::launch("engine-test-3", true)
        .await
        .expect("session launches");

    let snapshot = session
        .navigate(&fixture_url())
        .await
        .expect("navigate succeeds");
    assert!(
        snapshot.contains("textbox"),
        "snapshot missing textbox: {snapshot}"
    );
    assert!(
        snapshot.contains("Email address"),
        "snapshot missing label: {snapshot}"
    );
    assert!(
        snapshot.contains("Create account"),
        "snapshot missing button: {snapshot}"
    );
    assert!(
        !snapshot.to_lowercase().contains("decorative"),
        "decorative wrapper leaked into snapshot: {snapshot}"
    );

    let email_ref = find_ref(&snapshot, "textbox", "Email address").expect("email ref present");
    let submit_ref = find_ref(&snapshot, "button", "Create account").expect("submit ref present");

    session
        .type_text(&email_ref, "hello@example.com", false)
        .await
        .expect("type succeeds");

    let after_type = session.snapshot().await.expect("snapshot after type");
    assert!(
        after_type.contains("value=\"hello@example.com\""),
        "typed value missing: {after_type}"
    );

    session.click(&submit_ref).await.expect("click succeeds");

    let after_submit = session
        .wait_for("Account created", Duration::from_secs(5))
        .await
        .expect("wait_for finds the post-click text");
    assert!(after_submit.contains("Account created"));

    let png = session.screenshot().await.expect("screenshot succeeds");
    assert!(!png.is_empty(), "screenshot should not be empty");

    // Stale ref: a ref that was never assigned by the walker.
    match session.click("e999").await {
        Err(EngineError::StaleRef(r#ref)) => assert_eq!(r#ref, "e999"),
        other => panic!("expected StaleRef, got {other:?}"),
    }

    match session.press("F13").await {
        Err(EngineError::UnknownKey(key)) => assert_eq!(key, "F13"),
        other => panic!("expected UnknownKey, got {other:?}"),
    }

    session.close().await.expect("close succeeds");
}
