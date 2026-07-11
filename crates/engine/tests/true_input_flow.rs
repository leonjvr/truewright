//! true-user-input spec: real Windows `SendInput` dispatch. The live-dispatch
//! test moves the actual OS mouse cursor and briefly takes real keyboard
//! focus -- run only with the machine's owner watching (see PROPOSAL.md's
//! live-testing convention, same as human-motion-trained's physical-human
//! demo). Skips (not fails) when no browser is installed, matching Phase 0's
//! integration-test convention.

use engine::Session;

fn fixture_url() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/form.html");
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

/// Real, live `SendInput` dispatch against a real headed window: types into
/// a field and clicks a button via the actual OS input pipeline, then
/// confirms the page saw it -- the only way to confirm real dispatch
/// actually lands correctly (unit tests cover the coordinate math, not the
/// real Win32 calls).
#[tokio::test]
#[cfg(windows)]
async fn true_input_clicks_and_types_via_real_os_dispatch() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!(
            "skipping true_input_clicks_and_types_via_real_os_dispatch: no installed browser found"
        );
        return;
    }

    let session = Session::launch("true-input-test", false)
        .await
        .expect("headed session launches");

    let snapshot = session
        .navigate(&fixture_url())
        .await
        .expect("navigate succeeds");
    let email_ref = find_ref(&snapshot, "textbox", "Email address").expect("email ref present");
    let submit_ref = find_ref(&snapshot, "button", "Create account").expect("submit ref present");

    session
        .type_text_with(&email_ref, "true-input@example.com", false, None, true)
        .await
        .expect("real OS type dispatch succeeds");

    let after_type = session.snapshot().await.expect("snapshot after type");
    assert!(
        after_type.contains("value=\"true-input@example.com\""),
        "typed value missing after real OS dispatch: {after_type}"
    );

    session
        .click_with(&submit_ref, None, true)
        .await
        .expect("real OS click dispatch succeeds");

    let after_submit = session
        .wait_for("Account created", std::time::Duration::from_secs(5))
        .await
        .expect("wait_for finds the post-click text");
    assert!(after_submit.contains("Account created"));

    session.close().await.expect("close succeeds");
}

/// A headless session has no real OS window to receive `SendInput` events;
/// `true_input` rejects cleanly rather than silently falling back to CDP
/// dispatch (true-user-input spec: "Headless and non-Windows rejection").
/// Doesn't touch the real OS input pipeline -- safe to run unattended.
#[tokio::test]
async fn true_input_rejects_headless_session() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!("skipping true_input_rejects_headless_session: no installed browser found");
        return;
    }

    let session = Session::launch("true-input-headless-test", true)
        .await
        .expect("headless session launches");

    let snapshot = session
        .navigate(&fixture_url())
        .await
        .expect("navigate succeeds");
    let submit_ref = find_ref(&snapshot, "button", "Create account").expect("submit ref present");

    match session.click_with(&submit_ref, None, true).await {
        Err(engine::EngineError::TrueInputUnsupported(_)) => {}
        other => panic!("expected TrueInputUnsupported, got {other:?}"),
    }

    session.close().await.expect("close succeeds");
}
