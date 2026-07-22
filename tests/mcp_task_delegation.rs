//! mcp-task-delegation spec: `browser_run_task` and
//! `browser_screenshot(interpret: true)` against a real Chrome session and
//! a real local LLM stub, over a real streamable-HTTP MCP client -- same
//! discipline as `tests/mcp_http_flow.rs`. Skips (not fails) when no
//! browser is installed, matching this project's other integration tests.

use agent::Harness;
use llm::{Client, CompatClient, CredentialSource, RoleClient};
use mcp_server::AgentConfig;
use rmcp::model::CallToolRequestParams;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::ServiceExt;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[path = "support/mod.rs"]
mod support;
use support::llm_stub::LlmStub;
use support::{text_only_response, tool_call_response};

const TOKEN: &str = "test-only-token-do-not-reuse";

fn fixture_url() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("crates/agent/tests/fixtures/form.html");
    let mut normalized = path.to_string_lossy().replace('\\', "/");
    if !normalized.starts_with('/') {
        normalized = format!("/{normalized}");
    }
    format!("file://{normalized}")
}

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

fn agent_config(driver: RoleClient, vision: Option<RoleClient>) -> AgentConfig {
    AgentConfig {
        harness: Arc::new(Harness {
            driver,
            vision,
            max_steps: 10,
            step_timeout: Duration::from_secs(30),
            task_timeout: Duration::from_secs(60),
            max_retained_snapshots: 2,
        }),
        skill_dirs: Vec::new(),
    }
}

async fn spawn_server(agent: Option<AgentConfig>) -> (String, CancellationToken) {
    let cancellation_token = CancellationToken::new();
    let app = truewright::mcp::router(
        true,
        cdp::launch::BrowserPreference::Auto,
        Vec::new(),
        TOKEN.to_string(),
        cancellation_token.clone(),
        agent,
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind succeeds");
    let addr = listener.local_addr().expect("bound listener has an addr");

    tokio::spawn({
        let cancellation_token = cancellation_token.clone();
        async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async move { cancellation_token.cancelled_owned().await })
                .await;
        }
    });

    (format!("http://{addr}/mcp"), cancellation_token)
}

fn client_for(url: &str) -> StreamableHttpClientTransport<reqwest::Client> {
    StreamableHttpClientTransport::from_config(
        StreamableHttpClientTransportConfig::with_uri(url.to_string()).auth_header(TOKEN),
    )
}

fn args(value: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    serde_json::from_value(value).expect("valid arguments object")
}

#[tokio::test]
async fn browser_run_task_completes_a_delegated_task_over_the_shared_session() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!(
            "skipping browser_run_task_completes_a_delegated_task_over_the_shared_session: \
             no installed browser found"
        );
        return;
    }

    // Same deterministic ref assignment as crates/agent's own fixture-based
    // tests: the input is e1, the button is e2.
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
            "task_complete",
            serde_json::json!({"summary": "form submitted with Ada"}),
        )]),
    ];
    let stub = LlmStub::start(script).await;
    let agent = agent_config(role(stub.base_url(), false), None);

    let (url, cancellation_token) = spawn_server(Some(agent)).await;
    let client = rmcp::model::ClientInfo::default()
        .serve(client_for(&url))
        .await
        .expect("client connects");

    let result = client
        .call_tool(
            CallToolRequestParams::new("browser_run_task").with_arguments(args(
                serde_json::json!({"task": "fill in and submit the form"}),
            )),
        )
        .await
        .expect("browser_run_task completes as a real MCP call");

    assert_ne!(
        result.is_error,
        Some(true),
        "expected a passing outcome, got: {result:?}"
    );
    let text: String = result
        .content
        .iter()
        .filter_map(|block| match block {
            rmcp::model::ContentBlock::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        text.contains("PASS"),
        "expected PASS in transcript: {text:?}"
    );

    // The session survives the delegated task -- confirmed by driving it
    // further with an ordinary browser_* tool call afterward.
    let snapshot = client
        .call_tool(CallToolRequestParams::new("browser_snapshot"))
        .await
        .expect("browser_snapshot succeeds on the same session after browser_run_task");
    assert_ne!(snapshot.is_error, Some(true));

    let _ = client.cancel().await;
    stub.stop().await;
    cancellation_token.cancel();
}

#[tokio::test]
async fn browser_run_task_fails_clearly_when_no_driver_is_configured() {
    let (url, cancellation_token) = spawn_server(None).await;
    let client = rmcp::model::ClientInfo::default()
        .serve(client_for(&url))
        .await
        .expect("client connects");

    let err = client
        .call_tool(
            CallToolRequestParams::new("browser_run_task")
                .with_arguments(args(serde_json::json!({"task": "anything"}))),
        )
        .await
        .expect_err("browser_run_task fails outright with no driver configured");
    let message = err.to_string();
    assert!(
        message.contains("driver") || message.contains("roles.driver"),
        "expected a clear no-driver-configured error, got: {message:?}"
    );

    let _ = client.cancel().await;
    cancellation_token.cancel();
}

#[tokio::test]
async fn browser_screenshot_interpret_returns_vision_text_not_an_image() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!(
            "skipping browser_screenshot_interpret_returns_vision_text_not_an_image: \
             no installed browser found"
        );
        return;
    }

    let driver_stub = LlmStub::start(vec![]).await;
    let vision_stub = LlmStub::start(vec![text_only_response(
        "a blank page with no visible content",
    )])
    .await;
    let agent = agent_config(
        role(driver_stub.base_url(), false),
        Some(role(vision_stub.base_url(), true)),
    );

    let (url, cancellation_token) = spawn_server(Some(agent)).await;
    let client = rmcp::model::ClientInfo::default()
        .serve(client_for(&url))
        .await
        .expect("client connects");

    client
        .call_tool(
            CallToolRequestParams::new("browser_navigate")
                .with_arguments(args(serde_json::json!({"url": "about:blank"}))),
        )
        .await
        .expect("navigate succeeds");

    let result = client
        .call_tool(
            CallToolRequestParams::new("browser_screenshot")
                .with_arguments(args(serde_json::json!({"interpret": true}))),
        )
        .await
        .expect("browser_screenshot(interpret: true) succeeds");

    assert_ne!(result.is_error, Some(true));
    assert_eq!(
        result.content.len(),
        1,
        "expected exactly one content block: {:?}",
        result.content
    );
    match &result.content[0] {
        rmcp::model::ContentBlock::Text(t) => {
            assert!(t.text.contains("blank page"), "unexpected text: {}", t.text);
        }
        other => panic!("expected a text block, not {other:?}"),
    }

    let _ = client.cancel().await;
    driver_stub.stop().await;
    vision_stub.stop().await;
    cancellation_token.cancel();
}
