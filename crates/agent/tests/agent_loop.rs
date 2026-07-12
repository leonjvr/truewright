//! agent-harness spec: the full step loop against a real Chrome session
//! and a real local HTTP LLM stub -- end to end, not an in-process mock.
//! Skips (not fails) when no browser is installed, matching this
//! project's other integration tests' convention.

use agent::{Harness, SharedSession};
use engine::Session;
use llm::{Client, CompatClient, CredentialSource, RoleClient};
use std::collections::BTreeMap;
use std::time::Duration;

#[path = "support/mod.rs"]
mod support;
use support::llm_stub::LlmStub;
use support::{text_only_response, tool_call_response};

fn fixture_url() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/form.html");
    let mut normalized = path.to_string_lossy().replace('\\', "/");
    if !normalized.starts_with('/') {
        normalized = format!("/{normalized}");
    }
    format!("file://{normalized}")
}

fn driver_role(base_url: String, vision: bool) -> RoleClient {
    RoleClient {
        client: Client::Compat(CompatClient::new(
            base_url,
            CredentialSource::Static("test-key".to_string()),
            BTreeMap::new(),
        )),
        model: "test-model".to_string(),
        vision,
    }
}

fn test_harness(driver: RoleClient, vision: Option<RoleClient>) -> Harness {
    Harness {
        driver,
        vision,
        max_steps: 10,
        step_timeout: Duration::from_secs(30),
        task_timeout: Duration::from_secs(60),
        max_retained_snapshots: 2,
    }
}

#[tokio::test]
async fn a_scripted_navigate_type_click_assert_sequence_completes_the_task() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!("skipping a_scripted_navigate_type_click_assert_sequence_completes_the_task: no installed browser found");
        return;
    }

    // form.html's DOM order is deterministic and walker.js assigns refs
    // in first-encounter order: the input is e1, the button is e2 --
    // confirmed by this crate's fixture having exactly these two
    // interactive elements and nothing else.
    let script = vec![
        tool_call_response(&[("c1", "navigate", serde_json::json!({"url": fixture_url()}))]),
        tool_call_response(&[(
            "c2",
            "type",
            serde_json::json!({"ref": "e1", "text": "Ada", "submit": false}),
        )]),
        tool_call_response(&[("c3", "click", serde_json::json!({"ref": "e2"}))]),
        tool_call_response(&[(
            "c4",
            "assert",
            serde_json::json!({"text": "Submitted: Ada"}),
        )]),
        tool_call_response(&[(
            "c5",
            "task_complete",
            serde_json::json!({"summary": "form submitted with Ada"}),
        )]),
    ];
    let stub = LlmStub::start(script).await;

    let session = Session::launch("agent-loop-test", true)
        .await
        .expect("session launches");
    let shared = SharedSession::new(session);
    let harness = test_harness(driver_role(stub.base_url(), false), None);

    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    let outcome = harness
        .run_task(
            &shared,
            "Fill in the form with the name Ada and submit it",
            &[],
            None,
            Some(tx),
        )
        .await
        .expect("task completes");

    assert!(outcome.passed, "expected task_complete, got: {outcome:?}");
    assert_eq!(outcome.summary, "form submitted with Ada");
    assert_eq!(outcome.steps_used, 5);

    let mut events = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        events.push(ev);
    }
    assert!(
        events
            .iter()
            .any(|e| matches!(e, agent::AgentEvent::Done { passed: true, .. })),
        "expected a Done(passed=true) event, got: {events:?}"
    );

    // Request #2 (sent after navigate's tool-result was appended) must
    // carry the fixture's own snapshot text -- proving the executor's
    // real snapshot flowed back into the model's context, not just that
    // the navigate call itself didn't error.
    let requests = stub.requests().await;
    assert!(
        requests.len() >= 2,
        "expected at least 2 requests, got {}",
        requests.len()
    );
    let request_2_text = requests[1].to_string();
    assert!(
        request_2_text.contains("Your name") || request_2_text.contains("textbox"),
        "request #2 should contain the fixture's own snapshot text, got: {request_2_text}"
    );

    let mut guard = shared.0.lock().await;
    if let Some(s) = guard.take() {
        s.close().await.expect("close succeeds");
    }
    stub.stop().await;
}

#[tokio::test]
async fn a_stale_ref_error_is_fed_back_and_the_model_recovers() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!("skipping a_stale_ref_error_is_fed_back_and_the_model_recovers: no installed browser found");
        return;
    }

    let script = vec![
        tool_call_response(&[("c1", "navigate", serde_json::json!({"url": fixture_url()}))]),
        // A ref that was never assigned by the walker -- must come back
        // as a tool-result error, not abort the run.
        tool_call_response(&[("c2", "click", serde_json::json!({"ref": "e999"}))]),
        tool_call_response(&[(
            "c3",
            "task_failed",
            serde_json::json!({"reason": "could not find the element"}),
        )]),
    ];
    let stub = LlmStub::start(script).await;

    let session = Session::launch("agent-loop-error-test", true)
        .await
        .expect("session launches");
    let shared = SharedSession::new(session);
    let harness = test_harness(driver_role(stub.base_url(), false), None);

    let outcome = harness
        .run_task(&shared, "Click a nonexistent element", &[], None, None)
        .await
        .expect("task ends via task_failed, not an Err");
    assert!(!outcome.passed);
    assert_eq!(outcome.summary, "could not find the element");

    // Request #3 (sent after the bad click's tool-result was appended)
    // must contain the error text, proving it was fed back to the model
    // rather than the run aborting on the spot.
    let requests = stub.requests().await;
    assert!(
        requests.len() >= 3,
        "expected at least 3 requests, got {}",
        requests.len()
    );
    let request_3_text = requests[2].to_string();
    assert!(
        request_3_text.to_lowercase().contains("error"),
        "request #3 should contain the stale-ref error text, got: {request_3_text}"
    );

    let mut guard = shared.0.lock().await;
    if let Some(s) = guard.take() {
        s.close().await.expect("close succeeds");
    }
    stub.stop().await;
}

#[tokio::test]
async fn older_snapshots_are_pruned_once_more_than_the_retained_limit_have_accumulated() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!("skipping older_snapshots_are_pruned_once_more_than_the_retained_limit_have_accumulated: no installed browser found");
        return;
    }

    // Four distinctly-marked pages navigated in sequence -- distinct
    // content per step is what lets the assertions tell an elided
    // snapshot apart from a retained one (identical snapshots wouldn't).
    let marker_url = |marker: &str| {
        format!(
            "data:text/html,<html><body><button id=\"b\">Marker-{marker}</button></body></html>"
        )
    };
    let script = vec![
        tool_call_response(&[(
            "c1",
            "navigate",
            serde_json::json!({"url": marker_url("Alpha")}),
        )]),
        tool_call_response(&[(
            "c2",
            "navigate",
            serde_json::json!({"url": marker_url("Beta")}),
        )]),
        tool_call_response(&[(
            "c3",
            "navigate",
            serde_json::json!({"url": marker_url("Gamma")}),
        )]),
        tool_call_response(&[(
            "c4",
            "task_complete",
            serde_json::json!({"summary": "done"}),
        )]),
    ];
    let stub = LlmStub::start(script).await;

    let session = Session::launch("agent-loop-pruning-test", true)
        .await
        .expect("session launches");
    let shared = SharedSession::new(session);
    let mut harness = test_harness(driver_role(stub.base_url(), false), None);
    harness.max_retained_snapshots = 2;

    let outcome = harness
        .run_task(
            &shared,
            "Navigate through three marked pages",
            &[],
            None,
            None,
        )
        .await
        .expect("task completes");
    assert!(outcome.passed);

    // Request #4 is built after Alpha/Beta/Gamma's snapshots have all
    // been appended (3 > keep=2), so the oldest (Alpha) must be elided
    // while Beta and Gamma remain. Checked against the walker's own
    // rendered snapshot line shape (`button "Marker-Alpha"`), not just
    // the bare marker string -- "Marker-Alpha" legitimately still
    // appears elsewhere in the conversation, inside the assistant's own
    // earlier tool-call *arguments* (the navigate URL itself), which
    // isn't the thing pruning is supposed to touch.
    let requests = stub.requests().await;
    assert!(
        requests.len() >= 4,
        "expected at least 4 requests, got {}",
        requests.len()
    );
    let request_4_text = requests[3].to_string();
    assert!(
        !request_4_text.contains("button \\\"Marker-Alpha\\\""),
        "oldest snapshot's rendered content should be elided: {request_4_text}"
    );
    assert!(
        request_4_text.contains("elided"),
        "an elided placeholder should be present: {request_4_text}"
    );
    assert!(
        request_4_text.contains("button \\\"Marker-Beta\\\""),
        "second-oldest snapshot should still be retained: {request_4_text}"
    );
    assert!(
        request_4_text.contains("button \\\"Marker-Gamma\\\""),
        "most recent snapshot should still be retained: {request_4_text}"
    );

    let mut guard = shared.0.lock().await;
    if let Some(s) = guard.take() {
        s.close().await.expect("close succeeds");
    }
    stub.stop().await;
}

#[tokio::test]
async fn chatter_with_no_tool_calls_is_nudged_then_fails() {
    // No browser needed: the model never produces a tool call, so
    // execute_tool is never reached.
    let script = vec![
        text_only_response("Let me think about this..."),
        text_only_response("Still thinking..."),
    ];
    let stub = LlmStub::start(script).await;

    let shared = SharedSession(std::sync::Arc::new(tokio::sync::Mutex::new(None)));
    let harness = test_harness(driver_role(stub.base_url(), false), None);

    let err = harness
        .run_task(&shared, "Do something", &[], None, None)
        .await
        .expect_err("two consecutive no-tool-call turns must fail, not hang forever");
    assert!(matches!(err, agent::AgentError::NoProgress), "got: {err:?}");

    stub.stop().await;
}

#[tokio::test]
async fn exceeding_max_steps_fails_with_a_clear_error() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!(
            "skipping exceeding_max_steps_fails_with_a_clear_error: no installed browser found"
        );
        return;
    }

    // A cheap, session-touching tool call repeated forever, never
    // task_complete/task_failed -- LlmStub repeats the last scripted
    // entry once its script is exhausted.
    let script = vec![tool_call_response(&[(
        "c",
        "snapshot",
        serde_json::json!({}),
    )])];
    let stub = LlmStub::start(script).await;

    let session = Session::launch("agent-loop-maxsteps-test", true)
        .await
        .expect("session launches");
    let shared = SharedSession::new(session);
    let mut harness = test_harness(driver_role(stub.base_url(), false), None);
    harness.max_steps = 3;

    let err = harness
        .run_task(&shared, "Loop forever", &[], None, None)
        .await
        .expect_err("must fail once the step budget is exhausted");
    assert!(
        matches!(err, agent::AgentError::MaxStepsExceeded(3)),
        "got: {err:?}"
    );

    let mut guard = shared.0.lock().await;
    if let Some(s) = guard.take() {
        s.close().await.expect("close succeeds");
    }
    stub.stop().await;
}
