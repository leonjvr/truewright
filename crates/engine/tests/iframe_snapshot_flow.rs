//! same-origin-iframes spec: the walker recurses into same-origin iframe
//! content, cross-origin iframes render as an explicit not-inspectable
//! leaf, and refs resolved from inside a same-origin iframe click at the
//! correct top-level-page coordinates. Skips (not fails) when no browser
//! is installed, matching Phase 0's integration-test convention.

use engine::Session;

fn fixture_url() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/iframes.html");
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

#[tokio::test]
async fn same_origin_iframe_content_is_visible_and_clickable() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!(
            "skipping same_origin_iframe_content_is_visible_and_clickable: no installed browser found"
        );
        return;
    }

    let session = Session::launch("iframe-snapshot-test", true)
        .await
        .expect("session launches");

    let snapshot = session
        .navigate(&fixture_url())
        .await
        .expect("navigate succeeds");

    assert!(
        snapshot.contains("Inner Button"),
        "same-origin iframe content missing from snapshot: {snapshot}"
    );
    assert!(
        snapshot.contains("cross-origin iframe (not inspectable)"),
        "cross-origin iframe should be surfaced as an explicit boundary: {snapshot}"
    );
    assert!(
        snapshot.contains("- iframe"),
        "expected an iframe role node in the snapshot: {snapshot}"
    );

    let button_ref =
        find_ref(&snapshot, "button", "Inner Button").expect("inner button ref present");

    session
        .click(&button_ref)
        .await
        .expect("click on iframe-nested ref succeeds");

    let after_click = session.snapshot().await.expect("snapshot after click");
    assert!(
        after_click.contains("Clicked"),
        "expected the iframe's own click handler to have fired (proves the click landed on the \
         right on-screen coordinates, not just that the CDP call didn't error): {after_click}"
    );

    session.close().await.expect("close succeeds");
}
