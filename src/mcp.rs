//! `aib mcp` — runs the `browser` MCP server over stdio (mcp-server spec:
//! "Stdio MCP transport") or, with `--http`, over a loopback-only,
//! bearer-token-authenticated HTTP listener (mcp-streamable-http spec).

use axum::extract::{Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use mcp_server::{AgentConfig, AibTools};
use rmcp::transport::stdio;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::ServiceExt;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

pub async fn run(
    headless: bool,
    browser_pref: cdp::launch::BrowserPreference,
    agent: Option<AgentConfig>,
) -> std::process::ExitCode {
    let service = match AibTools::with_browser_pref(headless, browser_pref)
        .with_agent(agent)
        .serve(stdio())
        .await
    {
        Ok(service) => service,
        Err(e) => {
            tracing::error!(error = %e, "failed to start MCP server");
            return std::process::ExitCode::FAILURE;
        }
    };

    if let Err(e) = service.waiting().await {
        tracing::error!(error = %e, "MCP server exited with an error");
        return std::process::ExitCode::FAILURE;
    }

    std::process::ExitCode::SUCCESS
}

/// Builds the streamable-HTTP router: the MCP endpoint at `/mcp`, gated by
/// a bearer-token middleware layer (mcp-streamable-http spec: "Bearer-token
/// authentication"). Each new MCP session gets its own `AibTools` with a
/// uniquely-suffixed profile directory (spec: "Independent per-session
/// browser") -- never a cloned/shared instance, which would put every
/// session on the same browser (design.md Decision #2).
pub fn router(
    headless: bool,
    browser_pref: cdp::launch::BrowserPreference,
    token: String,
    cancellation_token: CancellationToken,
    agent: Option<AgentConfig>,
) -> axum::Router {
    let config = StreamableHttpServerConfig::default().with_cancellation_token(cancellation_token);
    let factory = move || {
        let suffix: u64 = rand::random();
        Ok(AibTools::with_profile_name(
            headless,
            browser_pref,
            format!("aib-mcp-http-{suffix:016x}"),
        )
        .with_agent(agent.clone()))
    };
    let service: StreamableHttpService<AibTools, LocalSessionManager> =
        StreamableHttpService::new(factory, Default::default(), config);

    let token: Arc<str> = Arc::from(token);
    axum::Router::new()
        .nest_service("/mcp", service)
        .layer(middleware::from_fn_with_state(token, require_bearer_token))
}

async fn require_bearer_token(
    State(expected): State<Arc<str>>,
    req: Request,
    next: Next,
) -> Response {
    let authorized = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .is_some_and(|token| token == expected.as_ref());

    if authorized {
        next.run(req).await
    } else {
        (StatusCode::UNAUTHORIZED, "unauthorized").into_response()
    }
}

fn generate_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub async fn run_http(
    headless: bool,
    browser_pref: cdp::launch::BrowserPreference,
    port: u16,
    token: Option<String>,
    agent: Option<AgentConfig>,
) -> std::process::ExitCode {
    let token = token.unwrap_or_else(generate_token);

    let listener = match TcpListener::bind(("127.0.0.1", port)).await {
        Ok(listener) => listener,
        Err(e) => {
            tracing::error!(error = %e, port, "failed to bind streamable-HTTP listener");
            return std::process::ExitCode::FAILURE;
        }
    };
    let addr = listener
        .local_addr()
        .expect("a bound TcpListener always has a local address");

    eprintln!("aib mcp: listening on http://{addr}/mcp");
    eprintln!("aib mcp: bearer token: {token}");

    let cancellation_token = CancellationToken::new();
    let app = router(
        headless,
        browser_pref,
        token,
        cancellation_token.clone(),
        agent,
    );

    tokio::spawn({
        let cancellation_token = cancellation_token.clone();
        async move {
            let _ = tokio::signal::ctrl_c().await;
            cancellation_token.cancel();
        }
    });

    match axum::serve(listener, app)
        .with_graceful_shutdown(async move { cancellation_token.cancelled_owned().await })
        .await
    {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(error = %e, "streamable-HTTP server exited with an error");
            std::process::ExitCode::FAILURE
        }
    }
}
