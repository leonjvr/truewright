//! popup-auto-attach spec: a popup/new-tab opened via `window.open()` as a
//! side effect of a click attaches automatically, is listed but not made
//! active, becomes drivable once explicitly switched to, and the active
//! page falls back to the original once the popup closes itself. Skips
//! (not fails) when no browser is installed, matching Phase 0's
//! integration-test convention.

use engine::{PageInfo, Session};
use std::time::Duration;

fn fixture_url(name: &str) -> String {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{name}"));
    let mut normalized = path.to_string_lossy().replace('\\', "/");
    if !normalized.starts_with('/') {
        normalized = format!("/{normalized}");
    }
    format!("file://{normalized}")
}

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

/// `Target.attachedToTarget`/`detachedFromTarget` are delivered
/// asynchronously; `list_pages` only reflects whatever has arrived by the
/// time it's called, so a short bounded poll is needed after an action
/// that's expected to change the attached-page count (see design.md
/// Decision #2 -- no artificial wait is built into `list_pages` itself).
async fn wait_for_page_count(session: &Session, want: usize, timeout: Duration) -> Vec<PageInfo> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let pages = session.list_pages().await.expect("list_pages succeeds");
        if pages.len() == want || std::time::Instant::now() >= deadline {
            return pages;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn popup_attaches_is_listed_switchable_and_falls_back_on_close() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!(
            "skipping popup_attaches_is_listed_switchable_and_falls_back_on_close: no installed browser found"
        );
        return;
    }

    let session = Session::launch("popup-auto-attach-test", true)
        .await
        .expect("session launches");

    let snapshot = session
        .navigate(&fixture_url("popup_opener.html"))
        .await
        .expect("navigate succeeds");
    let open_ref =
        find_ref(&snapshot, "button", "Sign in with Example").expect("open-popup ref present");

    let pages_before = session.list_pages().await.expect("list_pages succeeds");
    assert_eq!(
        pages_before.len(),
        1,
        "only the primary page yet: {pages_before:?}"
    );
    assert!(pages_before[0].active);

    session.click(&open_ref).await.expect("click succeeds");

    let pages_after = wait_for_page_count(&session, 2, Duration::from_secs(5)).await;
    assert_eq!(
        pages_after.len(),
        2,
        "popup should have attached: {pages_after:?}"
    );
    let popup = pages_after
        .iter()
        .find(|p| !p.active)
        .expect("a non-active popup page exists");
    let opener = pages_after
        .iter()
        .find(|p| p.active)
        .expect("the opener page is still active");
    assert!(
        popup.url.contains("popup_target.html"),
        "unexpected popup url: {popup:?}"
    );
    assert!(
        opener.url.contains("popup_opener.html"),
        "unexpected active-page url: {opener:?}"
    );

    session
        .switch_page(&popup.page_id)
        .await
        .expect("switch succeeds");

    let popup_snapshot = session.snapshot().await.expect("snapshot after switch");
    assert!(
        popup_snapshot.contains("Sign in - Example"),
        "expected popup content after switch: {popup_snapshot}"
    );
    let approve_ref = find_ref(&popup_snapshot, "button", "Approve").expect("approve ref present");

    session.click(&approve_ref).await.expect("click succeeds");

    // The popup's own click handler calls window.close().
    let pages_final = wait_for_page_count(&session, 1, Duration::from_secs(5)).await;
    assert_eq!(
        pages_final.len(),
        1,
        "popup should have detached: {pages_final:?}"
    );
    assert!(pages_final[0].active);
    assert!(pages_final[0].url.contains("popup_opener.html"));

    let final_snapshot = session.snapshot().await.expect("snapshot after fallback");
    assert!(
        final_snapshot.contains("Sign in with Example"),
        "expected fallback to opener content: {final_snapshot}"
    );

    session.close().await.expect("close succeeds");
}

#[tokio::test]
async fn switching_to_an_unknown_page_fails_clearly() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!(
            "skipping switching_to_an_unknown_page_fails_clearly: no installed browser found"
        );
        return;
    }

    let session = Session::launch("popup-auto-attach-unknown-test", true)
        .await
        .expect("session launches");

    match session.switch_page("not-a-real-target-id").await {
        Err(engine::EngineError::UnknownPage(id)) => assert_eq!(id, "not-a-real-target-id"),
        other => panic!("expected UnknownPage, got {other:?}"),
    }

    session.close().await.expect("close succeeds");
}
