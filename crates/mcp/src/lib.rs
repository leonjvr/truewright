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

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RecordStartRequest {
    #[serde(default)]
    #[schemars(
        description = "Maximum recording length in milliseconds (default and hard cap: 30000)"
    )]
    pub max_duration_ms: Option<u64>,
    #[serde(default)]
    #[schemars(description = "JPEG quality 0-100 (default 60)")]
    pub quality: Option<u8>,
}

const DEFAULT_WAIT_FOR_MS: u64 = 5000;

#[derive(Clone)]
pub struct AibTools {
    session: Arc<Mutex<Option<engine::Session>>>,
    recording: Arc<Mutex<Option<engine::Recording>>>,
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
            recording: Arc::new(Mutex::new(None)),
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

    #[tool(
        description = "Start recording the page (video, up to 30s). Fails if a recording is already active."
    )]
    async fn browser_record_start(
        &self,
        Parameters(RecordStartRequest {
            max_duration_ms,
            quality,
        }): Parameters<RecordStartRequest>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_session().await?;

        let mut recording_guard = self.recording.lock().await;
        if recording_guard.is_some() {
            return Err(McpError::invalid_params(
                "a recording is already in progress; call browser_record_stop first",
                None,
            ));
        }

        let session_guard = self.session.lock().await;
        let session = session_guard.as_ref().ok_or_else(no_session_error)?;

        let mut options = engine::RecordingOptions::default();
        if let Some(ms) = max_duration_ms {
            options.max_duration = std::time::Duration::from_millis(ms);
        }
        if let Some(q) = quality {
            options.quality = q;
        }

        let recording = session
            .start_recording(options)
            .await
            .map_err(map_engine_err)?;
        *recording_guard = Some(recording);

        Ok(CallToolResult::success(vec![ContentBlock::text(
            "recording started. Call browser_record_stop to finish (auto-stops after 30s).",
        )]))
    }

    #[tool(
        description = "Stop the active recording; returns the artifact directory, frame count, duration, and a preview frame"
    )]
    async fn browser_record_stop(&self) -> Result<CallToolResult, McpError> {
        let recording = self
            .recording
            .lock()
            .await
            .take()
            .ok_or_else(|| McpError::invalid_params("no recording is in progress", None))?;

        let output = recording.stop().await.map_err(map_engine_err)?;

        let summary = format!(
            "recording stopped: {} frames over {:.0}ms, saved to {}{}",
            output.frame_count,
            output.duration_ms,
            output.dir.display(),
            output
                .gif_path
                .as_ref()
                .map(|p| format!(" (gif: {})", p.display()))
                .unwrap_or_default()
        );

        let mut content = vec![ContentBlock::text(summary)];
        if let Some(preview) = output.preview_jpeg {
            content.push(ContentBlock::image(base64_encode(&preview), "image/jpeg"));
        }
        Ok(CallToolResult::success(content))
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
                 browser_record_start(max_duration_ms?, quality?), browser_record_stop(), \
                 browser_close(). Refs come from the snapshot text, e.g. `[e6]` -> ref \"e6\". \
                 Actions do not auto-return a new snapshot; call browser_snapshot again after \
                 an action that may have changed the page. Use browser_record_start/stop to \
                 capture a short GIF of moving/animated parts instead of a single screenshot."
                    .to_string(),
            )
    }
}

fn map_engine_err(e: engine::EngineError) -> McpError {
    use engine::EngineError::*;
    match e {
        StaleRef(_) | UnknownKey(_) => McpError::invalid_params(e.to_string(), None),
        ActionTimeout { .. } | WaitTimeout { .. } | Cdp(_) | Serde(_) | Recording(_) => {
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
