//! mcp-streamable-http spec: `truewright mcp --http` serves the same tools stdio
//! does, gated by a bearer token, loopback-only. Auth-layer tests need no
//! browser; the concurrent-session test skips (not fails) when no browser
//! is installed, matching Phase 0's integration-test convention.

use rmcp::model::CallToolRequestParams;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::ServiceExt;
use tokio_util::sync::CancellationToken;

const TOKEN: &str = "test-only-token-do-not-reuse";

async fn spawn_server() -> (String, CancellationToken) {
    spawn_server_with_agent(None).await
}

async fn spawn_server_with_agent(
    agent: Option<mcp_server::AgentConfig>,
) -> (String, CancellationToken) {
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
    // `.auth_header` is the raw token, not the header value -- the client
    // transport calls reqwest's `.bearer_auth()` internally, which already
    // prepends "Bearer " itself.
    StreamableHttpClientTransport::from_config(
        StreamableHttpClientTransportConfig::with_uri(url.to_string()).auth_header(TOKEN),
    )
}

#[tokio::test]
async fn request_without_a_token_is_rejected() {
    let (url, cancellation_token) = spawn_server().await;

    let resp = reqwest::Client::new()
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .body("{}")
        .send()
        .await
        .expect("request completes");
    assert_eq!(resp.status(), reqwest::StatusCode::UNAUTHORIZED);

    cancellation_token.cancel();
}

#[tokio::test]
async fn request_with_the_wrong_token_is_rejected() {
    let (url, cancellation_token) = spawn_server().await;

    let resp = reqwest::Client::new()
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .header("Authorization", "Bearer not-the-real-token")
        .body("{}")
        .send()
        .await
        .expect("request completes");
    assert_eq!(resp.status(), reqwest::StatusCode::UNAUTHORIZED);

    cancellation_token.cancel();
}

#[tokio::test]
async fn request_with_the_correct_token_completes_a_real_mcp_session() {
    let (url, cancellation_token) = spawn_server().await;

    let transport = client_for(&url);
    let client = rmcp::model::ClientInfo::default()
        .serve(transport)
        .await
        .expect("client connects and completes the initialize handshake");

    let tools = client.list_all_tools().await.expect("list_tools succeeds");
    assert!(
        tools.iter().any(|t| t.name.as_ref() == "browser_navigate"),
        "expected browser_navigate among the reported tools: {:?}",
        tools.iter().map(|t| t.name.clone()).collect::<Vec<_>>()
    );

    let _ = client.cancel().await;
    cancellation_token.cancel();
}

/// popup-auto-attach-style live check: two sessions launching around the
/// same time must not collide on the same Chrome profile directory
/// (mcp-streamable-http spec: "Independent per-session browser"). Needs a
/// real installed browser, unlike the auth-only tests above.
#[tokio::test]
async fn concurrent_sessions_get_independent_browsers() {
    if cdp::launch::discover_browsers().is_err() {
        eprintln!(
            "skipping concurrent_sessions_get_independent_browsers: no installed browser found"
        );
        return;
    }

    let (url, cancellation_token) = spawn_server().await;

    async fn connect(
        url: &str,
    ) -> rmcp::service::RunningService<rmcp::RoleClient, rmcp::model::ClientInfo> {
        rmcp::model::ClientInfo::default()
            .serve(client_for(url))
            .await
            .expect("client connects")
    }

    let (client_a, client_b) = tokio::join!(connect(&url), connect(&url));

    async fn navigate(
        client: &rmcp::service::RunningService<rmcp::RoleClient, rmcp::model::ClientInfo>,
    ) -> anyhow::Result<()> {
        let args: serde_json::Map<String, serde_json::Value> =
            serde_json::from_value(serde_json::json!({"url": "about:blank"}))?;
        client
            .call_tool(CallToolRequestParams::new("browser_navigate").with_arguments(args))
            .await?;
        Ok(())
    }

    let (nav_a, nav_b) = tokio::join!(navigate(&client_a), navigate(&client_b));
    nav_a.expect("session A navigates without a profile-directory collision");
    nav_b.expect("session B navigates without a profile-directory collision");

    let _ = client_a.cancel().await;
    let _ = client_b.cancel().await;
    cancellation_token.cancel();
}
