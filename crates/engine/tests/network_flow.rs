//! End-to-end network mocking test: record real traffic against a local
//! HTTP server, then replay with the server shut down and confirm the page
//! renders identically -- proves "no live-network dependency", not just
//! that the code compiles (network-mocking spec). Skips (not fails) when no
//! browser is installed, matching the other integration tests' convention.

#[path = "support/mod.rs"]
mod support;

use engine::Session;
use std::collections::HashMap;
use std::time::Duration;
use support::http_server::{Route, TestServer};

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

fn fixture_routes() -> HashMap<String, Route> {
    let html = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/network.html"),
    )
    .expect("read network.html fixture");

    let mut routes = HashMap::new();
    routes.insert(
        "/".to_string(),
        Route {
            content_type: "text/html",
            body: html,
        },
    );
    routes.insert(
        "/api/greeting".to_string(),
        Route {
            content_type: "application/json",
            body: r#"{"message":"hello from real server"}"#.to_string(),
        },
    );
    routes
}

#[tokio::test]
async fn record_then_replay_with_server_shut_down_renders_identically() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!(
            "skipping record_then_replay_with_server_shut_down_renders_identically: no installed browser found"
        );
        return;
    }

    let server = TestServer::start(fixture_routes()).await;
    let page_url = server.url("/");

    // --- Record ---
    let session = Session::launch("engine-test-network-record", true)
        .await
        .expect("session launches");

    let recording = session
        .network_record_start("network-flow-test")
        .await
        .expect("network recording starts");

    let snapshot = session.navigate(&page_url).await.expect("navigate succeeds");
    let fetch_ref = find_ref(&snapshot, "button", "Fetch greeting").expect("fetch button ref");
    session.click(&fetch_ref).await.expect("click succeeds");
    let after_fetch = session
        .wait_for("hello from real server", Duration::from_secs(5))
        .await
        .expect("wait_for finds the fetched greeting");
    assert!(after_fetch.contains("hello from real server"));

    let summary = recording.stop().await.expect("recording stops");
    assert!(
        summary.entry_count >= 2,
        "expected at least the page load and the API call to be recorded, got {}",
        summary.entry_count
    );

    session.close().await.expect("session closes");

    // Prove replay has no live dependency: shut the real server down before
    // replaying anything against it.
    server.stop().await;

    // --- Replay ---
    let replay_session = Session::launch("engine-test-network-replay", true)
        .await
        .expect("replay session launches");

    let replay = replay_session
        .network_replay_start("network-flow-test")
        .await
        .expect("network replay starts");

    let replay_snapshot = replay_session
        .navigate(&page_url)
        .await
        .expect("navigate succeeds against the (now-dead) real server, served from the cassette instead");
    let replay_fetch_ref =
        find_ref(&replay_snapshot, "button", "Fetch greeting").expect("fetch button ref in replay");
    replay_session
        .click(&replay_fetch_ref)
        .await
        .expect("click succeeds");
    let replay_after_fetch = replay_session
        .wait_for("hello from real server", Duration::from_secs(5))
        .await
        .expect("wait_for finds the replayed greeting");
    assert!(
        replay_after_fetch.contains("hello from real server"),
        "replay should render the exact recorded content: {replay_after_fetch}"
    );

    replay.stop().await.expect("replay stops");
    replay_session.close().await.expect("replay session closes");
}

#[tokio::test]
async fn unmatched_request_during_replay_fails_loudly() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!("skipping unmatched_request_during_replay_fails_loudly: no installed browser found");
        return;
    }

    let server = TestServer::start(fixture_routes()).await;
    let page_url = server.url("/");

    let session = Session::launch("engine-test-network-record-2", true)
        .await
        .expect("session launches");
    let recording = session
        .network_record_start("network-flow-unmatched-test")
        .await
        .expect("network recording starts");
    session.navigate(&page_url).await.expect("navigate succeeds");
    // Deliberately do NOT fetch /api/missing during recording, so the
    // cassette has no entry for it.
    recording.stop().await.expect("recording stops");
    session.close().await.expect("session closes");
    server.stop().await;

    let replay_session = Session::launch("engine-test-network-replay-2", true)
        .await
        .expect("replay session launches");
    let replay = replay_session
        .network_replay_start("network-flow-unmatched-test")
        .await
        .expect("network replay starts");

    let snapshot = replay_session
        .navigate(&page_url)
        .await
        .expect("navigate succeeds from the cassette");
    let missing_ref =
        find_ref(&snapshot, "button", "Fetch missing").expect("fetch-missing button ref");
    replay_session.click(&missing_ref).await.expect("click succeeds");

    let after = replay_session
        .wait_for("ERROR", Duration::from_secs(5))
        .await
        .expect("wait_for finds the fetch error");
    assert!(
        after.contains("ERROR"),
        "an unmatched request should surface as a fetch error, not a silent passthrough: {after}"
    );

    replay.stop().await.expect("replay stops");
    replay_session.close().await.expect("replay session closes");
}
