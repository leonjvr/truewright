//! End-to-end action trace test: while a console trace is active,
//! navigate/type/click each append a summary entry, interleaved
//! chronologically with any console output those actions provoke
//! (action-trace spec). Skips (not fails) when no browser is installed,
//! matching the other integration tests' convention.

use engine::Session;

fn fixture_url() -> String {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/form.html");
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
async fn actions_are_recorded_into_the_active_trace_in_chronological_order() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!(
            "skipping actions_are_recorded_into_the_active_trace_in_chronological_order: no installed browser found"
        );
        return;
    }

    let session = Session::launch("engine-test-action-trace", true)
        .await
        .expect("session launches");

    let capture = session
        .console_capture_start("action-trace-test")
        .await
        .expect("console capture starts");

    let snapshot = session
        .navigate(&fixture_url())
        .await
        .expect("navigate succeeds");
    let email_ref = find_ref(&snapshot, "textbox", "Email address").expect("email ref present");
    let submit_ref = find_ref(&snapshot, "button", "Create account").expect("submit ref present");

    session
        .type_text(&email_ref, "hello@example.com", false)
        .await
        .expect("type succeeds");
    session.click(&submit_ref).await.expect("click succeeds");

    let summary = capture.stop().await.expect("console capture stops");

    let raw = std::fs::read_to_string(&summary.path).expect("read trace file");
    let entries: Vec<serde_json::Value> = raw
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("valid JSON line"))
        .collect();

    let actions: Vec<&serde_json::Value> = entries
        .iter()
        .filter(|e| e["type"] == "action")
        .collect();
    assert_eq!(
        actions.len(),
        3,
        "expected exactly one action entry each for navigate/type/click: {entries:#?}"
    );

    assert!(
        actions[0]["text"].as_str().unwrap_or("").starts_with("navigate "),
        "first action should be the navigate: {:?}",
        actions[0]
    );
    assert!(
        actions[1]["text"]
            .as_str()
            .unwrap_or("")
            .starts_with(&format!("type {email_ref} ")),
        "second action should be the type into the email field: {:?}",
        actions[1]
    );
    assert!(
        actions[2]["text"]
            .as_str()
            .unwrap_or("")
            .starts_with(&format!("click {submit_ref}")),
        "third action should be the click on submit: {:?}",
        actions[2]
    );

    let t0 = actions[0]["timestamp_ms"].as_f64().unwrap();
    let t1 = actions[1]["timestamp_ms"].as_f64().unwrap();
    let t2 = actions[2]["timestamp_ms"].as_f64().unwrap();
    assert!(
        t0 <= t1 && t1 <= t2,
        "action entries should be in chronological order: {t0}, {t1}, {t2}"
    );

    session.close().await.expect("session closes");
}
