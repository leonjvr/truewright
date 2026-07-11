//! End-to-end virtual clock test: Date.now reflects the installed virtual
//! time and doesn't move on its own; a delayed setTimeout doesn't fire
//! before its delay is advanced past; a chain of callbacks scheduled
//! within the same advance (0ms follow-up hops) all fire in one
//! browser_advance_clock call (virtual-clock spec). Skips (not fails) when
//! no browser is installed, matching the other integration tests'
//! convention.

use engine::Session;

const CLOCK_START_MS: u64 = 1_700_000_000_000;

fn fixture_url() -> String {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/clock_fixture.html");
    let mut normalized = path.to_string_lossy().replace('\\', "/");
    if !normalized.starts_with('/') {
        normalized = format!("/{normalized}");
    }
    format!("file://{normalized}")
}

/// Checks for a line rendering exactly `- text "<needle>"` (trimmed),
/// avoiding accidental partial-string matches between the fixture's
/// several distinct `<p>` values.
fn has_exact_text(snapshot: &str, needle: &str) -> bool {
    let target = format!("- text \"{needle}\"");
    snapshot.lines().any(|line| line.trim() == target)
}

#[tokio::test]
async fn virtual_clock_freezes_time_and_advances_fire_due_timers_in_order() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!(
            "skipping virtual_clock_freezes_time_and_advances_fire_due_timers_in_order: no installed browser found"
        );
        return;
    }

    let session = Session::launch("engine-test-virtual-clock", true)
        .await
        .expect("session launches");

    session
        .set_clock(CLOCK_START_MS)
        .await
        .expect("clock installs");

    let snapshot = session
        .navigate(&fixture_url())
        .await
        .expect("navigate succeeds");
    assert!(
        has_exact_text(&snapshot, &format!("now={CLOCK_START_MS}")),
        "Date.now() should reflect the installed virtual time: {snapshot}"
    );
    assert!(
        has_exact_text(&snapshot, "not yet"),
        "delayed callback should not have fired yet: {snapshot}"
    );
    assert!(
        has_exact_text(&snapshot, "0"),
        "chain should not have started yet: {snapshot}"
    );

    // Real time elapsing (this await itself takes some real wall-clock
    // time) must not move the virtual clock on its own.
    let unchanged = session.snapshot().await.expect("snapshot succeeds");
    assert!(
        has_exact_text(&unchanged, &format!("now={CLOCK_START_MS}")),
        "virtual time must not move without an explicit advance: {unchanged}"
    );

    // Advance by less than the first hop's delay (100ms) -- nothing due yet.
    session.advance_clock(50).await.expect("advance succeeds");
    let mid = session.snapshot().await.expect("snapshot succeeds");
    assert!(
        has_exact_text(&mid, "not yet"),
        "5000ms delay should still be pending after only 50ms: {mid}"
    );
    assert!(
        has_exact_text(&mid, "0"),
        "100ms chain start should still be pending after only 50ms: {mid}"
    );

    // Advance past the first hop's delay (total 100ms) -- the whole
    // 0ms-follow-up chain should complete within this single advance.
    session.advance_clock(50).await.expect("advance succeeds");
    let after_chain = session.snapshot().await.expect("snapshot succeeds");
    assert!(
        has_exact_text(&after_chain, "3"),
        "chain of 0ms-delay follow-ups scheduled within the same advance should all fire: {after_chain}"
    );
    assert!(
        has_exact_text(&after_chain, "not yet"),
        "5000ms delay should still be pending after only 100ms: {after_chain}"
    );

    // Advance the rest of the way past the 5000ms delay.
    session.advance_clock(4_900).await.expect("advance succeeds");
    let after_delay = session.snapshot().await.expect("snapshot succeeds");
    assert!(
        has_exact_text(&after_delay, "fired"),
        "5000ms delayed callback should have fired after advancing past it: {after_delay}"
    );

    session.close().await.expect("session closes");
}
