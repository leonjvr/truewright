//! agent-harness spec: screenshot routing is governed by the driver
//! role's `vision` capability flag -- a non-vision driver's screenshot is
//! interpreted by a dedicated vision role and the interpretation text
//! (not an image) lands in the driver's context; a vision-capable driver
//! gets the raw image inline instead. Both verified against a real
//! Chrome screenshot and real local HTTP stubs.

use agent::{Harness, SharedSession};
use engine::Session;
use llm::{Client, CompatClient, CredentialSource, RoleClient};
use std::collections::BTreeMap;
use std::time::Duration;

#[path = "support/mod.rs"]
mod support;
use support::llm_stub::LlmStub;
use support::tool_call_response;

fn role(base_url: String, vision: bool) -> RoleClient {
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

#[tokio::test]
async fn non_vision_driver_routes_screenshots_to_the_vision_role_as_text() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!("skipping non_vision_driver_routes_screenshots_to_the_vision_role_as_text: no installed browser found");
        return;
    }

    let driver_script = vec![
        tool_call_response(&[("c1", "screenshot", serde_json::json!({}))]),
        tool_call_response(&[(
            "c2",
            "task_complete",
            serde_json::json!({"summary": "looked at the page"}),
        )]),
    ];
    let driver_stub = LlmStub::start(driver_script).await;

    let vision_script = vec![serde_json::json!({
        "choices": [{
            "message": {"role": "assistant", "content": "The page shows a mostly blank test page."},
            "finish_reason": "stop"
        }]
    })];
    let vision_stub = LlmStub::start(vision_script).await;

    let session = Session::launch("vision-routing-test", true)
        .await
        .expect("session launches");
    let shared = SharedSession::new(session);
    let harness = Harness {
        driver: role(driver_stub.base_url(), false),
        vision: Some(role(vision_stub.base_url(), true)),
        max_steps: 10,
        step_timeout: Duration::from_secs(30),
        task_timeout: Duration::from_secs(60),
        max_retained_snapshots: 2,
    };

    let outcome = harness
        .run_task(
            &shared,
            "Take a screenshot and describe it",
            &[],
            None,
            None,
        )
        .await
        .expect("task completes");
    assert!(outcome.passed);

    // The vision stub actually received a real request with a real
    // image_url part.
    let vision_requests = vision_stub.requests().await;
    assert_eq!(vision_requests.len(), 1);
    let vision_body = vision_requests[0].to_string();
    assert!(
        vision_body.contains("image_url") && vision_body.contains("data:image/png;base64,"),
        "vision stub should receive the real screenshot: {vision_body}"
    );

    // The DRIVER's next request contains the interpretation TEXT, not an
    // image part -- it has no vision of its own.
    let driver_requests = driver_stub.requests().await;
    assert!(driver_requests.len() >= 2);
    let driver_request_2 = driver_requests[1].to_string();
    assert!(
        driver_request_2.contains("mostly blank test page"),
        "driver's next request should contain the vision interpretation text: {driver_request_2}"
    );
    assert!(
        !driver_request_2.contains("image_url"),
        "a non-vision driver must never receive an image part: {driver_request_2}"
    );

    let mut guard = shared.0.lock().await;
    if let Some(s) = guard.take() {
        s.close().await.expect("close succeeds");
    }
    driver_stub.stop().await;
    vision_stub.stop().await;
}

#[tokio::test]
async fn vision_capable_driver_gets_the_raw_image_inline() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!(
            "skipping vision_capable_driver_gets_the_raw_image_inline: no installed browser found"
        );
        return;
    }

    let driver_script = vec![
        tool_call_response(&[("c1", "screenshot", serde_json::json!({}))]),
        tool_call_response(&[(
            "c2",
            "task_complete",
            serde_json::json!({"summary": "saw it directly"}),
        )]),
    ];
    let driver_stub = LlmStub::start(driver_script).await;

    let session = Session::launch("vision-routing-inline-test", true)
        .await
        .expect("session launches");
    let shared = SharedSession::new(session);
    let harness = Harness {
        driver: role(driver_stub.base_url(), true), // driver itself has vision
        vision: None,                               // no separate vision role configured
        max_steps: 10,
        step_timeout: Duration::from_secs(30),
        task_timeout: Duration::from_secs(60),
        max_retained_snapshots: 2,
    };

    let outcome = harness
        .run_task(&shared, "Take a screenshot", &[], None, None)
        .await
        .expect("task completes");
    assert!(outcome.passed);

    let driver_requests = driver_stub.requests().await;
    assert!(driver_requests.len() >= 2);
    let driver_request_2 = driver_requests[1].to_string();
    assert!(
        driver_request_2.contains("image_url")
            && driver_request_2.contains("data:image/png;base64,"),
        "a vision-capable driver should receive the raw image inline: {driver_request_2}"
    );

    let mut guard = shared.0.lock().await;
    if let Some(s) = guard.take() {
        s.close().await.expect("close succeeds");
    }
    driver_stub.stop().await;
}

#[tokio::test]
async fn screenshot_with_no_vision_role_and_no_vision_driver_errors_clearly() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!("skipping screenshot_with_no_vision_role_and_no_vision_driver_errors_clearly: no installed browser found");
        return;
    }

    let driver_script = vec![
        tool_call_response(&[("c1", "screenshot", serde_json::json!({}))]),
        tool_call_response(&[(
            "c2",
            "task_failed",
            serde_json::json!({"reason": "cannot see screenshots"}),
        )]),
    ];
    let driver_stub = LlmStub::start(driver_script).await;

    let session = Session::launch("vision-routing-none-test", true)
        .await
        .expect("session launches");
    let shared = SharedSession::new(session);
    let harness = Harness {
        driver: role(driver_stub.base_url(), false),
        vision: None,
        max_steps: 10,
        step_timeout: Duration::from_secs(30),
        task_timeout: Duration::from_secs(60),
        max_retained_snapshots: 2,
    };

    let outcome = harness
        .run_task(&shared, "Take a screenshot", &[], None, None)
        .await
        .expect("task ends");
    assert!(!outcome.passed);

    let driver_requests = driver_stub.requests().await;
    let driver_request_2 = driver_requests[1].to_string();
    assert!(
        driver_request_2.to_lowercase().contains("no vision role"),
        "should explain no vision role is configured: {driver_request_2}"
    );

    let mut guard = shared.0.lock().await;
    if let Some(s) = guard.take() {
        s.close().await.expect("close succeeds");
    }
    driver_stub.stop().await;
}
