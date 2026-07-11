//! cross-origin-oopif spec: a cross-*site* iframe (real separate registrable
//! domain, not an opaque `data:`/`srcdoc` origin) that Chrome isolates into
//! its own OOPIF target renders its real content in the snapshot, with
//! namespaced refs, and clicking one of those refs actually lands on the
//! right on-screen coordinates in the OOPIF's own process. Skips (not
//! fails) when no browser is installed, matching this project's other
//! integration tests' convention.

use engine::Session;
use std::collections::HashMap;

#[path = "support/mod.rs"]
mod support;
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

#[tokio::test]
async fn a_cross_site_oopif_iframe_shows_real_content_and_is_clickable() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!(
            "skipping a_cross_site_oopif_iframe_shows_real_content_and_is_clickable: no installed browser found"
        );
        return;
    }

    let inner_html = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/oopif_inner.html"),
    )
    .expect("read oopif_inner.html fixture");
    let top_html_template = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/oopif_top.html"),
    )
    .expect("read oopif_top.html fixture");

    let server = TestServer::start_with(move |port| {
        let inner_url = format!("http://b.localhost:{port}/inner");
        let top_html = top_html_template.replace("__INNER_URL__", &inner_url);
        let mut routes = HashMap::new();
        routes.insert(
            "/".to_string(),
            Route {
                content_type: "text/html",
                body: top_html,
            },
        );
        routes.insert(
            "/inner".to_string(),
            Route {
                content_type: "text/html",
                body: inner_html,
            },
        );
        routes
    })
    .await;

    let session = Session::launch_with_flags(
        "cross-origin-oopif-test",
        true,
        cdp::launch::BrowserPreference::Auto,
        &["--site-per-process"],
    )
    .await
    .expect("session launches");

    let snapshot = session
        .navigate(&server.url_on("a.localhost", "/"))
        .await
        .expect("navigate succeeds");

    assert!(
        snapshot.contains("Inner Button"),
        "OOPIF content missing from snapshot -- expected real content, not a placeholder: {snapshot}"
    );

    let button_ref =
        find_ref(&snapshot, "button", "Inner Button").expect("inner button ref present");
    assert!(
        button_ref.contains(':'),
        "OOPIF-sourced ref should be namespaced (e.g. f1:e2), got: {button_ref}"
    );

    session
        .click(&button_ref)
        .await
        .expect("click on OOPIF-nested ref succeeds");

    let after_click = session.snapshot().await.expect("snapshot after click");
    assert!(
        after_click.contains("Clicked"),
        "expected the OOPIF's own click handler to have fired (proves the click landed on the \
         right on-screen coordinates in the OOPIF's own process, not just that the CDP call \
         didn't error): {after_click}"
    );

    session.close().await.expect("close succeeds");
    server.stop().await;
}
