//! shadow-dom-walker spec: the walker recurses into open shadow roots and
//! walks a <slot>'s actually-projected light-DOM content, with refs and
//! clicks working the same as any other element (no coordinate changes
//! needed, unlike iframes). Skips (not fails) when no browser is
//! installed, matching Phase 0's integration-test convention.

use engine::Session;

fn fixture_url() -> String {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shadow_dom.html");
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
async fn shadow_root_and_slotted_content_are_visible_and_clickable() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!(
            "skipping shadow_root_and_slotted_content_are_visible_and_clickable: no installed browser found"
        );
        return;
    }

    let session = Session::launch("shadow-dom-test", true)
        .await
        .expect("session launches");

    let snapshot = session
        .navigate(&fixture_url())
        .await
        .expect("navigate succeeds");

    assert!(
        snapshot.contains("Card Title"),
        "slotted (projected) light-DOM content missing from snapshot: {snapshot}"
    );
    assert!(
        snapshot.contains("Card Button"),
        "shadow tree's own directly-rendered content missing from snapshot: {snapshot}"
    );

    let button_ref =
        find_ref(&snapshot, "button", "Card Button").expect("shadow button ref present");

    session
        .click(&button_ref)
        .await
        .expect("click on shadow-nested ref succeeds");

    let after_click = session.snapshot().await.expect("snapshot after click");
    assert!(
        after_click.contains("Clicked"),
        "expected the shadow tree's own click handler to have fired (proves resolve.js needs no \
         changes for shadow-tree coordinates, not just that the CDP call didn't error): {after_click}"
    );

    session.close().await.expect("close succeeds");
}
