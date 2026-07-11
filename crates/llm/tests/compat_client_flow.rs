//! llm-providers spec: `CompatClient` against a real local HTTP server (not
//! just serialized-shape assertions) -- a real socket round trip, real
//! Authorization header, real retry-on-5xx behavior.

use llm::{ChatRequest, CompatClient, CredentialSource, LlmError, Message};
use std::collections::BTreeMap;

#[path = "support/mod.rs"]
mod support;
use support::stub_server::StubServer;

fn sample_request() -> ChatRequest {
    ChatRequest {
        model: "test-model".to_string(),
        messages: vec![Message::system("be helpful"), Message::user("hello")],
        tools: vec![],
    }
}

fn ok_response(text: &str) -> serde_json::Value {
    serde_json::json!({
        "choices": [{
            "message": {"role": "assistant", "content": text},
            "finish_reason": "stop"
        }]
    })
}

#[tokio::test]
async fn real_request_reaches_the_server_with_auth_header_and_body() {
    let server = StubServer::start(vec![(200, ok_response("pong"))]).await;
    let client = CompatClient::new(
        server.base_url(),
        CredentialSource::Static("test-key-123".to_string()),
        BTreeMap::new(),
    );

    let resp = client
        .complete(&sample_request())
        .await
        .expect("request succeeds");
    assert_eq!(resp.message.text(), "pong");

    let requests = server.requests().await;
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].headers.get("authorization").map(String::as_str),
        Some("Bearer test-key-123")
    );
    assert_eq!(requests[0].body["model"], "test-model");
    assert_eq!(requests[0].body["messages"][1]["content"], "hello");

    server.stop().await;
}

#[tokio::test]
async fn transient_server_error_is_retried_and_then_succeeds() {
    let server = StubServer::start(vec![
        (500, serde_json::json!({"error": "transient"})),
        (200, ok_response("recovered")),
    ])
    .await;
    let client = CompatClient::new(
        server.base_url(),
        CredentialSource::Static("test-key".to_string()),
        BTreeMap::new(),
    );

    let resp = client
        .complete(&sample_request())
        .await
        .expect("retry recovers");
    assert_eq!(resp.message.text(), "recovered");
    assert_eq!(
        server.requests().await.len(),
        2,
        "one failed attempt, one retry"
    );

    server.stop().await;
}

#[tokio::test]
async fn non_retryable_client_error_surfaces_immediately() {
    let server = StubServer::start(vec![(400, serde_json::json!({"error": "bad request"}))]).await;
    let client = CompatClient::new(
        server.base_url(),
        CredentialSource::Static("test-key".to_string()),
        BTreeMap::new(),
    );

    let err = client
        .complete(&sample_request())
        .await
        .expect_err("400 is not retried");
    assert!(
        matches!(err, LlmError::HttpStatus { status: 400, .. }),
        "got: {err:?}"
    );
    assert_eq!(
        server.requests().await.len(),
        1,
        "no retry on a non-transient 4xx"
    );

    server.stop().await;
}
