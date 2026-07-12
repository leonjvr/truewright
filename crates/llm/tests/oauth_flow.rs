//! oauth-subscription-auth spec: PKCE math, token exchange/refresh, and
//! the local callback listener, each verified against a real local
//! server/socket -- not just serialized-shape assertions. The real
//! `auth.openai.com` authorize/token endpoints and a real browser login
//! are explicitly out of scope here (see design.md's testing note); this
//! exercises exactly the same code paths (`exchange_code_with_flow`/
//! `refresh_with_flow`/the callback listener) a real login drives, just
//! against a stub instead of OpenAI's real servers.

use llm::{CallbackResult, OAuthFlowSpec, TokenStore};
use std::time::Duration;

#[path = "support/mod.rs"]
mod support;
use support::stub_server::StubServer;

fn fake_flow(token_url: String) -> OAuthFlowSpec {
    OAuthFlowSpec {
        id: "test-flow",
        authorize_url: "http://unused.invalid/authorize",
        token_url: Box::leak(token_url.into_boxed_str()),
        client_id: "test-client-id",
        scope: "openid",
        extra_authorize_params: &[],
        redirect_port: 0,
        redirect_port_fallback: 0,
        redirect_path: "/auth/callback",
        token_exchange_is_form_encoded: true,
    }
}

fn make_id_token(claims_json: &str) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    let header = URL_SAFE_NO_PAD.encode(b"{\"alg\":\"none\"}");
    let payload = URL_SAFE_NO_PAD.encode(claims_json.as_bytes());
    format!("{header}.{payload}.unused-signature")
}

#[tokio::test]
async fn exchange_code_hits_a_real_server_form_encoded_and_extracts_account_id() {
    let far_future_exp = 4_000_000_000u64;
    let id_token = make_id_token(&format!(
        r#"{{"exp":{far_future_exp},"https://api.openai.com/auth":{{"chatgpt_account_id":"acct_live_test"}}}}"#
    ));
    let server = StubServer::start(vec![(
        200,
        serde_json::json!({
            "id_token": id_token,
            "access_token": "at-real-http-test",
            "refresh_token": "rt-real-http-test",
        }),
    )])
    .await;
    let flow = fake_flow(format!("{}/token", server.base_url()));

    let tokens = llm::exchange_code_with_flow(
        &flow,
        "auth-code-123",
        "verifier-abc",
        "http://localhost:0/auth/callback",
    )
    .await
    .expect("exchange succeeds against the real local server");

    assert_eq!(tokens.access_token, "at-real-http-test");
    assert_eq!(tokens.refresh_token.as_deref(), Some("rt-real-http-test"));
    assert_eq!(tokens.account_id.as_deref(), Some("acct_live_test"));
    assert_eq!(tokens.expires_at_epoch_s, far_future_exp);

    let requests = server.requests().await;
    assert_eq!(requests.len(), 1);
    let ct = requests[0]
        .headers
        .get("content-type")
        .cloned()
        .unwrap_or_default();
    assert!(
        ct.contains("application/x-www-form-urlencoded"),
        "exchange must be form-encoded, got: {ct}"
    );

    server.stop().await;
}

#[tokio::test]
async fn refresh_hits_a_real_server_json_encoded() {
    let id_token = make_id_token(r#"{"exp":4000000001}"#);
    let server = StubServer::start(vec![(
        200,
        serde_json::json!({
            "id_token": id_token,
            "access_token": "at-refreshed",
            "refresh_token": "rt-rotated",
        }),
    )])
    .await;
    let flow = fake_flow(format!("{}/token", server.base_url()));

    let tokens = llm::refresh_with_flow(&flow, "old-refresh-token")
        .await
        .expect("refresh succeeds against the real local server");

    assert_eq!(tokens.access_token, "at-refreshed");
    assert_eq!(tokens.refresh_token.as_deref(), Some("rt-rotated"));

    let requests = server.requests().await;
    assert_eq!(requests.len(), 1);
    // JSON, not form-encoded -- the real asymmetry confirmed against
    // OpenAI's own Codex CLI source (see design.md).
    let ct = requests[0]
        .headers
        .get("content-type")
        .cloned()
        .unwrap_or_default();
    assert!(
        ct.contains("application/json"),
        "refresh must be JSON-encoded, got: {ct}"
    );
    assert_eq!(requests[0].body["grant_type"], "refresh_token");
    assert_eq!(requests[0].body["refresh_token"], "old-refresh-token");

    server.stop().await;
}

#[tokio::test]
async fn a_non_success_token_response_fails_clearly() {
    let server =
        StubServer::start(vec![(400, serde_json::json!({"error": "invalid_grant"}))]).await;
    let flow = fake_flow(format!("{}/token", server.base_url()));

    let err = llm::exchange_code_with_flow(
        &flow,
        "bad-code",
        "verifier",
        "http://localhost:0/auth/callback",
    )
    .await
    .expect_err("a 400 from the token endpoint is not silently swallowed");
    assert!(
        format!("{err}").contains("400"),
        "error should mention the status: {err}"
    );

    server.stop().await;
}

/// A real socket bind + a real HTTP GET simulating the browser's OAuth
/// redirect landing on the local callback listener -- no browser, no
/// external network, but a genuine `TcpListener` accepting a genuine
/// request.
#[tokio::test]
async fn callback_listener_accepts_a_real_redirect_and_extracts_code_state() {
    let listener = llm::bind_callback(0)
        .await
        .expect("binds an OS-assigned port");
    let port = listener.local_addr().expect("local addr").port();

    let accept_task = tokio::spawn(llm::accept_callback(
        listener,
        "/auth/callback",
        Duration::from_secs(5),
    ));

    // Give the listener a moment to actually be accepting before "the
    // browser" (a plain HTTP client here) hits it.
    tokio::time::sleep(Duration::from_millis(50)).await;
    let resp = reqwest::get(format!(
        "http://127.0.0.1:{port}/auth/callback?code=abc123&state=xyz789"
    ))
    .await
    .expect("simulated browser redirect reaches the real listener");
    assert!(resp.status().is_success());

    let CallbackResult { code, state } = accept_task
        .await
        .expect("task join")
        .expect("callback succeeds");
    assert_eq!(code, "abc123");
    assert_eq!(state, "xyz789");
}

#[tokio::test]
async fn callback_listener_surfaces_a_denied_consent_as_an_error() {
    let listener = llm::bind_callback(0)
        .await
        .expect("binds an OS-assigned port");
    let port = listener.local_addr().expect("local addr").port();

    let accept_task = tokio::spawn(llm::accept_callback(
        listener,
        "/auth/callback",
        Duration::from_secs(5),
    ));
    tokio::time::sleep(Duration::from_millis(50)).await;
    let _ = reqwest::get(format!(
        "http://127.0.0.1:{port}/auth/callback?error=access_denied"
    ))
    .await;

    let result = accept_task.await.expect("task join");
    assert!(
        result.is_err(),
        "a denied-consent callback must surface as an error, not a silent nothing"
    );
}

#[tokio::test]
async fn token_store_persists_what_login_would_produce() {
    let dir = std::env::temp_dir().join(format!(
        "truewright-llm-oauth-flow-test-{}",
        std::process::id()
    ));
    let store = TokenStore::new(dir.clone());

    let id_token = make_id_token(
        r#"{"exp":4000000002,"https://api.openai.com/auth":{"chatgpt_account_id":"acct_store_test"}}"#,
    );
    let server = StubServer::start(vec![(
        200,
        serde_json::json!({"id_token": id_token, "access_token": "at-store-test", "refresh_token": "rt-store-test"}),
    )])
    .await;
    let flow = fake_flow(format!("{}/token", server.base_url()));

    let tokens = llm::exchange_code_with_flow(
        &flow,
        "code",
        "verifier",
        "http://localhost:0/auth/callback",
    )
    .await
    .expect("exchange succeeds");
    store.save(flow.id, &tokens).expect("save succeeds");

    let loaded = store
        .load(flow.id)
        .expect("load succeeds")
        .expect("present");
    assert_eq!(loaded.access_token, "at-store-test");
    assert_eq!(loaded.account_id.as_deref(), Some("acct_store_test"));

    server.stop().await;
    std::fs::remove_dir_all(&dir).ok();
}
