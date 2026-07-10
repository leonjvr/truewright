//! The `browser` MCP server (mcp-server spec): a stdio tool surface over
//! one lazily-created `engine::Session`.

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo};
use rmcp::{schemars, tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct NavigateRequest {
    #[schemars(description = "URL to navigate to")]
    pub url: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RefRequest {
    #[schemars(description = "Element ref from a snapshot, e.g. \"e6\"")]
    pub r#ref: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TypeRequest {
    #[schemars(description = "Element ref from a snapshot, e.g. \"e6\"")]
    pub r#ref: String,
    #[schemars(description = "Text to insert")]
    pub text: String,
    #[serde(default)]
    #[schemars(description = "Press Enter after inserting the text")]
    pub submit: bool,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct PressRequest {
    #[schemars(description = "One of: Enter, Tab, Escape, ArrowDown, ArrowUp, Backspace")]
    pub key: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct WaitForRequest {
    #[schemars(description = "Substring to wait for in the page snapshot")]
    pub text: String,
    #[serde(default)]
    #[schemars(description = "Timeout in milliseconds (default 5000)")]
    pub timeout_ms: Option<u64>,
}

const DEFAULT_WAIT_FOR_MS: u64 = 5000;

#[derive(Clone)]
pub struct AibTools {
    session: Arc<Mutex<Option<engine::Session>>>,
    headless: bool,
    browser_pref: cdp::launch::BrowserPreference,
    // Read by the `#[tool_handler]`-generated `ServerHandler` impl to route
    // incoming `tools/call` requests to the methods below.
    #[allow(dead_code)]
    tool_router: ToolRouter<AibTools>,
}

#[tool_router]
impl AibTools {
    pub fn new(headless: bool) -> Self {
        Self::with_browser_pref(headless, cdp::launch::BrowserPreference::Auto)
    }

    pub fn with_browser_pref(headless: bool, browser_pref: cdp::launch::BrowserPreference) -> Self {
        Self {
            session: Arc::new(Mutex::new(None)),
            headless,
            browser_pref,
            tool_router: Self::tool_router(),
        }
    }

    async fn ensure_session(&self) -> Result<(), McpError> {
        let mut guard = self.session.lock().await;
        if guard.is_none() {
            let session = engine::Session::launch_with("aib-mcp", self.headless, self.browser_pref)
                .await
                .map_err(map_engine_err)?;
            *guard = Some(session);
        }
        Ok(())
    }

    #[tool(description = "Navigate to a URL and return a snapshot of the resulting page")]
    async fn browser_navigate(
        &self,
        Parameters(NavigateRequest { url }): Parameters<NavigateRequest>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_session().await?;
        let guard = self.session.lock().await;
        let session = guard.as_ref().ok_or_else(no_session_error)?;
        let text = session.navigate(&url).await.map_err(map_engine_err)?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[tool(description = "Get a fresh accessibility-style snapshot of the current page")]
    async fn browser_snapshot(&self) -> Result<CallToolResult, McpError> {
        self.ensure_session().await?;
        let guard = self.session.lock().await;
        let session = guard.as_ref().ok_or_else(no_session_error)?;
        let text = session.snapshot().await.map_err(map_engine_err)?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[tool(description = "Click an element by its ref from the last snapshot")]
    async fn browser_click(
        &self,
        Parameters(RefRequest { r#ref }): Parameters<RefRequest>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_session().await?;
        let guard = self.session.lock().await;
        let session = guard.as_ref().ok_or_else(no_session_error)?;
        session.click(&r#ref).await.map_err(map_engine_err)?;
        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "clicked {ref}. Call browser_snapshot to see any resulting changes."
        ))]))
    }

    #[tool(
        description = "Click an element by ref, then type text into it (optionally submit with Enter)"
    )]
    async fn browser_type(
        &self,
        Parameters(TypeRequest {
            r#ref,
            text,
            submit,
        }): Parameters<TypeRequest>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_session().await?;
        let guard = self.session.lock().await;
        let session = guard.as_ref().ok_or_else(no_session_error)?;
        session
            .type_text(&r#ref, &text, submit)
            .await
            .map_err(map_engine_err)?;
        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "typed into {ref}. Call browser_snapshot to see any resulting changes."
        ))]))
    }

    #[tool(description = "Press a named key: Enter, Tab, Escape, ArrowDown, ArrowUp, Backspace")]
    async fn browser_press(
        &self,
        Parameters(PressRequest { key }): Parameters<PressRequest>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_session().await?;
        let guard = self.session.lock().await;
        let session = guard.as_ref().ok_or_else(no_session_error)?;
        session.press(&key).await.map_err(map_engine_err)?;
        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "pressed {key}"
        ))]))
    }

    #[tool(
        description = "Wait for a substring to appear in the page snapshot; returns the snapshot once found"
    )]
    async fn browser_wait_for(
        &self,
        Parameters(WaitForRequest { text, timeout_ms }): Parameters<WaitForRequest>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_session().await?;
        let guard = self.session.lock().await;
        let session = guard.as_ref().ok_or_else(no_session_error)?;
        let timeout = std::time::Duration::from_millis(timeout_ms.unwrap_or(DEFAULT_WAIT_FOR_MS));
        let snapshot = session
            .wait_for(&text, timeout)
            .await
            .map_err(map_engine_err)?;
        Ok(CallToolResult::success(vec![ContentBlock::text(snapshot)]))
    }

    #[tool(description = "Capture a screenshot of the current page")]
    async fn browser_screenshot(&self) -> Result<CallToolResult, McpError> {
        self.ensure_session().await?;
        let guard = self.session.lock().await;
        let session = guard.as_ref().ok_or_else(no_session_error)?;
        let bytes = session.screenshot().await.map_err(map_engine_err)?;
        let data = base64_encode(&bytes);
        Ok(CallToolResult::success(vec![ContentBlock::image(
            data,
            "image/png",
        )]))
    }

    #[tool(description = "Close the browser session")]
    async fn browser_close(&self) -> Result<CallToolResult, McpError> {
        let mut guard = self.session.lock().await;
        if let Some(session) = guard.take() {
            session.close().await.map_err(map_engine_err)?;
            Ok(CallToolResult::success(vec![ContentBlock::text(
                "session closed",
            )]))
        } else {
            Ok(CallToolResult::success(vec![ContentBlock::text(
                "no session was open",
            )]))
        }
    }
}

#[tool_handler]
impl ServerHandler for AibTools {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_instructions(
                "Drives a real Chrome/Edge browser. Tools: browser_navigate(url), \
                 browser_snapshot(), browser_click(ref), browser_type(ref, text, submit?), \
                 browser_press(key), browser_wait_for(text, timeout_ms?), browser_screenshot(), \
                 browser_close(). Refs come from the snapshot text, e.g. `[e6]` -> ref \"e6\". \
                 Actions do not auto-return a new snapshot; call browser_snapshot again after \
                 an action that may have changed the page."
                    .to_string(),
            )
    }
}

fn map_engine_err(e: engine::EngineError) -> McpError {
    use engine::EngineError::*;
    match e {
        StaleRef(_) | UnknownKey(_) => McpError::invalid_params(e.to_string(), None),
        ActionTimeout { .. } | WaitTimeout { .. } | Cdp(_) | Serde(_) => {
            McpError::internal_error(e.to_string(), None)
        }
    }
}

fn no_session_error() -> McpError {
    McpError::internal_error("no active browser session", None)
}

fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}
