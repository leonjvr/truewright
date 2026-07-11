//! End-to-end YAML runner tests: a hand-written script runs successfully;
//! a script with a failing assert stops there without running later steps;
//! a real captured trace exports to YAML and replaying it reproduces the
//! same end state (yaml-runner spec). Skips (not fails) when no browser is
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
async fn a_valid_script_runs_all_steps_successfully() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!("skipping a_valid_script_runs_all_steps_successfully: no installed browser found");
        return;
    }

    let session = Session::launch("engine-test-yaml-valid", true)
        .await
        .expect("session launches");

    let yaml = format!(
        r#"
name: signup flow
steps:
  - navigate: "{url}"
  - type:
      ref: e2
      text: "hello@example.com"
  - click: e3
  - assert:
      text: "Account created"
"#,
        url = fixture_url()
    );

    let summary = session.run_yaml(&yaml).await.expect("script should run successfully");
    assert_eq!(summary.steps_run, 4);
    assert_eq!(summary.total_steps, 4);

    session.close().await.expect("session closes");
}

#[tokio::test]
async fn a_failing_step_stops_the_run_before_later_steps() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!("skipping a_failing_step_stops_the_run_before_later_steps: no installed browser found");
        return;
    }

    let session = Session::launch("engine-test-yaml-fail", true)
        .await
        .expect("session launches");

    let yaml = format!(
        r#"
steps:
  - navigate: "{url}"
  - assert:
      text: "Account created"
  - type:
      ref: e2
      text: "should-not-run@example.com"
"#,
        url = fixture_url()
    );

    match session.run_yaml(&yaml).await {
        Err(EngineError::YamlStepFailed {
            step_number,
            total_steps,
            step_kind,
            ..
        }) => {
            assert_eq!(step_number, 2);
            assert_eq!(total_steps, 3);
            assert_eq!(step_kind, "assert");
        }
        other => panic!("expected YamlStepFailed at step 2, got {other:?}"),
    }

    // The third step (typing into e2) must not have run.
    let snapshot = session.snapshot().await.expect("snapshot succeeds");
    assert!(
        !snapshot.contains("should-not-run@example.com"),
        "a step after the failure should not have executed: {snapshot}"
    );

    session.close().await.expect("session closes");
}

#[tokio::test]
async fn a_captured_trace_exports_to_yaml_and_replays_correctly() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!("skipping a_captured_trace_exports_to_yaml_and_replays_correctly: no installed browser found");
        return;
    }

    // --- Record a real flow ---
    let recording_session = Session::launch("engine-test-yaml-export-record", true)
        .await
        .expect("session launches");
    let capture = recording_session
        .console_capture_start("yaml-export-test")
        .await
        .expect("console capture starts");

    recording_session
        .navigate(&fixture_url())
        .await
        .expect("navigate succeeds");
    recording_session
        .type_text("e2", "exported@example.com", false)
        .await
        .expect("type succeeds");
    recording_session.click("e3").await.expect("click succeeds");

    capture.stop().await.expect("console capture stops");
    recording_session.close().await.expect("session closes");

    // --- Export and replay against a fresh session ---
    let yaml = Session::export_yaml("yaml-export-test").expect("export succeeds");
    assert!(yaml.contains("navigate"), "exported YAML should contain a navigate step: {yaml}");
    assert!(yaml.contains("exported@example.com"), "exported YAML should contain the typed text: {yaml}");

    let replay_session = Session::launch("engine-test-yaml-export-replay", true)
        .await
        .expect("replay session launches");
    replay_session
        .run_yaml(&yaml)
        .await
        .expect("exported script should replay successfully");

    let snapshot = replay_session.snapshot().await.expect("snapshot succeeds");
    assert!(
        snapshot.contains("Account created"),
        "replaying the exported script should reproduce the same end state: {snapshot}"
    );

    replay_session.close().await.expect("replay session closes");
}
