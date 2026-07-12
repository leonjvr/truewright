//! oauth-subscription-auth spec: `ResponsesClient` against a real local
//! SSE-emitting server -- the Codex/ChatGPT backend this client talks to
//! is effectively SSE-only (see design.md), so this verifies the
//! aggregation-to-a-single-`ChatResponse` logic against a genuine byte
//! stream, not just a hand-constructed `serde_json::Value` fed directly
//! to the parser.
//!
//! A small dedicated SSE server, not the shared `support::stub_server`:
//! that one always serves `application/json`; SSE needs a different
//! content type and a body shaped as `event:`/`data:` blocks, which is
//! specific enough to this one client that duplicating the shared
//! server's plumbing for it isn't worth it.

use llm::{ChatRequest, CredentialSource, Message, ResponsesClient};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Binds an OS-assigned port, accepts exactly one connection, drains the
/// request, and writes back `sse_body` as a `text/event-stream` response.
/// Returns the base URL to send the request to.
async fn spawn_sse_server(sse_body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind sse stub");
    let port = listener.local_addr().expect("local addr").port();

    tokio::spawn(async move {
        let Ok((mut stream, _)) = listener.accept().await else {
            return;
        };
        let mut buf = [0u8; 4096];
        // Drain whatever the client sent (headers + JSON body) without
        // needing to parse it -- this stub always returns the same
        // scripted SSE body regardless of request content.
        let _ = tokio::time::timeout(std::time::Duration::from_millis(200), stream.read(&mut buf))
            .await;

        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{sse_body}",
            sse_body.len()
        );
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.shutdown().await;
    });

    format!("http://127.0.0.1:{port}")
}

#[tokio::test]
async fn aggregates_a_real_sse_stream_into_one_text_response() {
    let sse_body = concat!(
        "event: response.output_text.delta\n",
        "data: {\"delta\": \"partial, ignored\"}\n\n",
        "event: response.completed\n",
        "data: {\"response\": {\"output\": [{\"type\": \"message\", \"content\": [{\"type\": \"output_text\", \"text\": \"final answer\"}]}], \"usage\": {\"input_tokens\": 3, \"output_tokens\": 2, \"total_tokens\": 5}}}\n\n",
    );
    let base_url = spawn_sse_server(sse_body).await;

    let client = ResponsesClient::new(base_url, CredentialSource::Static("test-token".to_string()));
    let req = ChatRequest {
        model: "test-model".to_string(),
        messages: vec![Message::user("hello")],
        tools: vec![],
    };

    let resp = client
        .complete(&req)
        .await
        .expect("aggregates the real SSE stream");
    assert_eq!(resp.message.text(), "final answer");
    assert_eq!(resp.usage.unwrap().total_tokens, 5);
}

#[tokio::test]
async fn aggregates_a_real_sse_stream_with_a_tool_call() {
    let sse_body = concat!(
        "event: response.completed\n",
        "data: {\"response\": {\"output\": [{\"type\": \"function_call\", \"call_id\": \"call_9\", \"name\": \"navigate\", \"arguments\": \"{\\\"url\\\":\\\"https://example.com\\\"}\"}]}}\n\n",
    );
    let base_url = spawn_sse_server(sse_body).await;

    let client = ResponsesClient::new(base_url, CredentialSource::Static("test-token".to_string()));
    let req = ChatRequest {
        model: "test-model".to_string(),
        messages: vec![Message::user("go to example.com")],
        tools: vec![],
    };

    let resp = client
        .complete(&req)
        .await
        .expect("aggregates a tool-call SSE stream");
    assert_eq!(resp.message.tool_calls.len(), 1);
    assert_eq!(resp.message.tool_calls[0].name, "navigate");
    assert_eq!(resp.message.tool_calls[0].id, "call_9");
}

#[tokio::test]
async fn account_id_is_sent_as_a_header_when_present() {
    // Reuses the SSE server but this test's point is the *request* the
    // client sends, so a permissive server response is enough -- what
    // matters is proving the header actually goes out over a real socket.
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("local addr").port();
    let (tx, rx) = tokio::sync::oneshot::channel();

    tokio::spawn(async move {
        let Ok((mut stream, _)) = listener.accept().await else {
            return;
        };
        let mut buf = vec![0u8; 8192];
        let n = tokio::time::timeout(std::time::Duration::from_millis(200), stream.read(&mut buf))
            .await
            .ok()
            .and_then(|r| r.ok())
            .unwrap_or(0);
        let request_text = String::from_utf8_lossy(&buf[..n]).to_string();
        let _ = tx.send(request_text);

        let sse_body = "event: response.completed\ndata: {\"response\": {\"output\": []}}\n\n";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{sse_body}",
            sse_body.len()
        );
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.shutdown().await;
    });

    let dir = std::env::temp_dir().join(format!(
        "aib-llm-responses-account-id-test-{}",
        std::process::id()
    ));
    let store = std::sync::Arc::new(llm::TokenStore::new(dir.clone()));
    store
        .save(
            "chatgpt",
            &llm::StoredTokens {
                access_token: "at-test".to_string(),
                refresh_token: None,
                id_token: None,
                expires_at_epoch_s: 4_000_000_000,
                account_id: Some("acct_header_test".to_string()),
            },
        )
        .expect("save succeeds");
    let credential = CredentialSource::OAuth {
        provider: "chatgpt".to_string(),
        flow_id: "chatgpt".to_string(),
        store,
    };

    let client = ResponsesClient::new(format!("http://127.0.0.1:{port}"), credential);
    let req = ChatRequest {
        model: "test-model".to_string(),
        messages: vec![Message::user("hi")],
        tools: vec![],
    };
    client.complete(&req).await.expect("request completes");

    let sent = rx.await.expect("captured the real request");
    assert!(
        sent.to_lowercase()
            .contains("chatgpt-account-id: acct_header_test"),
        "expected the account-id header in the real request, got:\n{sent}"
    );

    std::fs::remove_dir_all(&dir).ok();
}
