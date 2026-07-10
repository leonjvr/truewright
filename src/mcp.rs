//! `aib mcp` — runs the `browser` MCP server over stdio (mcp-server spec:
//! "Stdio MCP transport").

use mcp_server::AibTools;
use rmcp::transport::stdio;
use rmcp::ServiceExt;

pub async fn run(headless: bool) -> std::process::ExitCode {
    let service = match AibTools::new(headless).serve(stdio()).await {
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
