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
pub struct AddInitScriptRequest {
    #[schemars(
        description = "JavaScript to run before any of the page's own scripts, on every subsequent navigation in this session. Register before browser_navigate -- it only affects loads that happen after registration."
    )]
    pub source: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SeedRandomnessRequest {
    #[schemars(
        description = "Seed for a deterministic Math.random() override, registered as an init script (register before browser_navigate). Same seed -> identical Math.random() sequence across navigations."
    )]
    pub seed: u64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SetClockRequest {
    #[schemars(
        description = "Epoch milliseconds to freeze the virtual clock at. Register before browser_navigate -- it only affects loads that happen after registration. Time only moves via browser_advance_clock."
    )]
    pub time_ms: u64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AdvanceClockRequest {
    #[schemars(
        description = "Milliseconds to advance the installed virtual clock by. Fires every due setTimeout/setInterval/requestAnimationFrame callback in chronological order, including callbacks newly scheduled within the same advance. Requires browser_set_clock to have been called (and browser_navigate since)."
    )]
    pub ms: u64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RefRequest {
    #[schemars(description = "Element ref from a snapshot, e.g. \"e6\"")]
    pub r#ref: String,
    #[serde(default)]
    #[schemars(
        description = "Move the mouse along a synthesized human-like curved path before clicking, instead of teleporting to the target (default false)"
    )]
    pub human_like: bool,
    #[serde(default)]
    #[schemars(
        description = "Persona for human_like motion: one of careful, average, fast (default average). Mutually exclusive with trained_profile."
    )]
    pub persona: Option<String>,
    #[serde(default)]
    #[schemars(
        description = "Name of a profile trained via browser_train_start/stop; replays that human's fitted timing instead of a synthetic persona. Mutually exclusive with persona. Fails clearly if the name was never trained."
    )]
    pub trained_profile: Option<String>,
    #[serde(default)]
    #[schemars(description = "Fixed seed for reproducible human_like motion (default: random)")]
    pub seed: Option<u64>,
    #[serde(default)]
    #[schemars(
        description = "Dispatch via real Windows OS-level input (SendInput) instead of CDP -- moves the actual system cursor and clicks for real, not just inside the browser tab. Windows-only, headed sessions only (default false)."
    )]
    pub true_input: bool,
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
    #[serde(default)]
    #[schemars(
        description = "Move to the field with a human-like curved path and type character-by-character with human-like pauses, instead of an instant click + bulk insert (default false)"
    )]
    pub human_like: bool,
    #[serde(default)]
    #[schemars(
        description = "Persona for human_like motion: one of careful, average, fast (default average). Mutually exclusive with trained_profile."
    )]
    pub persona: Option<String>,
    #[serde(default)]
    #[schemars(
        description = "Name of a profile trained via browser_train_start/stop; replays that human's fitted timing instead of a synthetic persona. Mutually exclusive with persona. Fails clearly if the name was never trained."
    )]
    pub trained_profile: Option<String>,
    #[serde(default)]
    #[schemars(description = "Fixed seed for reproducible human_like motion (default: random)")]
    pub seed: Option<u64>,
    #[serde(default)]
    #[schemars(
        description = "Dispatch via real Windows OS-level input (SendInput) instead of CDP -- moves the actual system cursor and types for real, not just inside the browser tab. Windows-only, headed sessions only (default false)."
    )]
    pub true_input: bool,
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
pub struct AssertRequest {
    #[schemars(description = "Substring to check for in the current page snapshot")]
    pub text: String,
    #[serde(default = "default_true")]
    #[schemars(
        description = "true (default): assert the text is present. false: assert it is absent."
    )]
    pub present: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RunYamlRequest {
    #[schemars(
        description = "YAML script source: {name?, steps: [{navigate: url}, {click: ref}, {type: {ref, text, submit?}}, {press: key}, {wait_for: {text, timeout_ms?}}, {assert: {text, present?}}]}"
    )]
    pub source: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ExportYamlRequest {
    #[schemars(description = "Name of a trace saved via browser_console_start/stop to export")]
    pub name: String,
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

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TrainStartRequest {
    #[schemars(
        description = "Name to save the trained profile under; browser_train_stop persists it, and browser_click/browser_type's trained_profile selects it later"
    )]
    pub name: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct NetworkNameRequest {
    #[schemars(description = "Name to save/load the network cassette under")]
    pub name: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ConsoleStartRequest {
    #[schemars(description = "Name to save the JSONL console/exception trace under")]
    pub name: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SwitchPageRequest {
    #[schemars(
        description = "Page id from browser_list_pages -- subsequent actions (browser_click/browser_type/browser_snapshot/etc.) target this page instead"
    )]
    pub page_id: String,
}

#[derive(Clone)]
pub struct AibTools {
    session: Arc<Mutex<Option<engine::Session>>>,
    recording: Arc<Mutex<Option<engine::Recording>>>,
    training: Arc<Mutex<Option<engine::Training>>>,
    network_recording: Arc<Mutex<Option<engine::NetworkRecording>>>,
    network_replay: Arc<Mutex<Option<engine::NetworkReplay>>>,
    console_capture: Arc<Mutex<Option<engine::ConsoleCapture>>>,
    headless: bool,
    browser_pref: cdp::launch::BrowserPreference,
    /// The `Session::launch` profile-directory name. Fixed at `"aib-mcp"`
    /// for stdio (one process, one profile, never a collision risk);
    /// streamable-HTTP mode gives each session its own randomly-suffixed
    /// name instead, since multiple concurrent sessions could otherwise
    /// collide on the same Chrome user-data directory (mcp-streamable-http
    /// spec: "Independent per-session browser").
    profile_name: String,
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
        Self::with_profile_name(headless, browser_pref, "aib-mcp".to_string())
    }

    /// As `with_browser_pref`, but with an explicit profile-directory name
    /// instead of the fixed `"aib-mcp"` default -- used by streamable-HTTP
    /// mode to give each session its own isolated profile.
    pub fn with_profile_name(
        headless: bool,
        browser_pref: cdp::launch::BrowserPreference,
        profile_name: String,
    ) -> Self {
        Self {
            session: Arc::new(Mutex::new(None)),
            recording: Arc::new(Mutex::new(None)),
            training: Arc::new(Mutex::new(None)),
            network_recording: Arc::new(Mutex::new(None)),
            network_replay: Arc::new(Mutex::new(None)),
            console_capture: Arc::new(Mutex::new(None)),
            headless,
            browser_pref,
            profile_name,
            tool_router: Self::tool_router(),
        }
    }

    async fn ensure_session(&self) -> Result<(), McpError> {
        let mut guard = self.session.lock().await;
        if guard.is_none() {
            let session =
                engine::Session::launch_with(&self.profile_name, self.headless, self.browser_pref)
                    .await
                    .map_err(map_engine_err)?;
            *guard = Some(session);
        }
        Ok(())
    }

    #[tool(
        description = "Register JS that runs before any of the page's own scripts, on every subsequent navigation. Call before browser_navigate for it to take effect."
    )]
    async fn browser_add_init_script(
        &self,
        Parameters(AddInitScriptRequest { source }): Parameters<AddInitScriptRequest>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_session().await?;
        let guard = self.session.lock().await;
        let session = guard.as_ref().ok_or_else(no_session_error)?;
        session
            .add_init_script(&source)
            .await
            .map_err(map_engine_err)?;
        Ok(CallToolResult::success(vec![ContentBlock::text(
            "init script registered; call browser_navigate for it to take effect.",
        )]))
    }

    #[tool(
        description = "Override Math.random with a deterministic PRNG seeded from the given value, for reproducible test runs. Call before browser_navigate for it to take effect."
    )]
    async fn browser_seed_randomness(
        &self,
        Parameters(SeedRandomnessRequest { seed }): Parameters<SeedRandomnessRequest>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_session().await?;
        let guard = self.session.lock().await;
        let session = guard.as_ref().ok_or_else(no_session_error)?;
        session
            .seed_randomness(seed)
            .await
            .map_err(map_engine_err)?;
        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "Math.random seeded with {seed}; call browser_navigate for it to take effect."
        ))]))
    }

    #[tool(
        description = "Install a virtual clock frozen at the given epoch time. Time only moves via browser_advance_clock. Call before browser_navigate for it to take effect."
    )]
    async fn browser_set_clock(
        &self,
        Parameters(SetClockRequest { time_ms }): Parameters<SetClockRequest>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_session().await?;
        let guard = self.session.lock().await;
        let session = guard.as_ref().ok_or_else(no_session_error)?;
        session.set_clock(time_ms).await.map_err(map_engine_err)?;
        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "virtual clock set to {time_ms}; call browser_navigate for it to take effect."
        ))]))
    }

    #[tool(
        description = "Advance the installed virtual clock, firing every due timer/interval/animation-frame callback in order. Requires browser_set_clock (and a browser_navigate since) first."
    )]
    async fn browser_advance_clock(
        &self,
        Parameters(AdvanceClockRequest { ms }): Parameters<AdvanceClockRequest>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_session().await?;
        let guard = self.session.lock().await;
        let session = guard.as_ref().ok_or_else(no_session_error)?;
        session.advance_clock(ms).await.map_err(map_engine_err)?;
        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "virtual clock advanced by {ms}ms."
        ))]))
    }

    #[tool(
        description = "Start capturing the page's console.* output and uncaught exceptions to a named JSONL trace. Fails if a console capture is already active."
    )]
    async fn browser_console_start(
        &self,
        Parameters(ConsoleStartRequest { name }): Parameters<ConsoleStartRequest>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_session().await?;

        let mut console_guard = self.console_capture.lock().await;
        if console_guard.is_some() {
            return Err(McpError::invalid_params(
                "a console capture is already in progress; call browser_console_stop first",
                None,
            ));
        }

        let session_guard = self.session.lock().await;
        let session = session_guard.as_ref().ok_or_else(no_session_error)?;

        let capture = session
            .console_capture_start(&name)
            .await
            .map_err(map_engine_err)?;
        *console_guard = Some(capture);

        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "console capture started for {name:?}. Call browser_console_stop when done \
             (auto-stops after 5 minutes)."
        ))]))
    }

    #[tool(description = "Stop the active console capture and save the JSONL trace")]
    async fn browser_console_stop(&self) -> Result<CallToolResult, McpError> {
        let capture =
            self.console_capture.lock().await.take().ok_or_else(|| {
                McpError::invalid_params("no console capture is in progress", None)
            })?;

        let summary = capture.stop().await.map_err(map_engine_err)?;

        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "console trace {:?} saved: {} entr{} to {}",
            summary.name,
            summary.entry_count,
            if summary.entry_count == 1 { "y" } else { "ies" },
            summary.path.display()
        ))]))
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

    #[tool(
        description = "Click an element by its ref from the last snapshot. Set human_like to move the mouse along a curved path first, like a real user. Set true_input to dispatch via real Windows OS-level input instead of CDP (moves the actual system cursor)."
    )]
    async fn browser_click(
        &self,
        Parameters(RefRequest {
            r#ref,
            human_like,
            persona,
            trained_profile,
            seed,
            true_input,
        }): Parameters<RefRequest>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_session().await?;
        let guard = self.session.lock().await;
        let session = guard.as_ref().ok_or_else(no_session_error)?;
        let human = build_human_like(human_like, persona, trained_profile, seed)?;
        let used_seed = session
            .click_with(&r#ref, human, true_input)
            .await
            .map_err(map_engine_err)?;
        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "clicked {ref}{}. Call browser_snapshot to see any resulting changes.",
            seed_suffix(used_seed)
        ))]))
    }

    #[tool(
        description = "Click an element by ref, then type text into it (optionally submit with Enter). Set human_like to move to it and type character-by-character with human-like pauses. Set true_input to dispatch via real Windows OS-level input instead of CDP (moves the actual system cursor and takes real keyboard focus)."
    )]
    async fn browser_type(
        &self,
        Parameters(TypeRequest {
            r#ref,
            text,
            submit,
            human_like,
            persona,
            trained_profile,
            seed,
            true_input,
        }): Parameters<TypeRequest>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_session().await?;
        let guard = self.session.lock().await;
        let session = guard.as_ref().ok_or_else(no_session_error)?;
        let human = build_human_like(human_like, persona, trained_profile, seed)?;
        let used_seed = session
            .type_text_with(&r#ref, &text, submit, human, true_input)
            .await
            .map_err(map_engine_err)?;
        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "typed into {ref}{}. Call browser_snapshot to see any resulting changes.",
            seed_suffix(used_seed)
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

    #[tool(
        description = "Immediately assert whether a substring is present (or absent) in the current page snapshot -- no polling, unlike browser_wait_for. Fails as a tool-level error result (not a protocol error) if the assertion doesn't hold."
    )]
    async fn browser_assert(
        &self,
        Parameters(AssertRequest { text, present }): Parameters<AssertRequest>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_session().await?;
        let guard = self.session.lock().await;
        let session = guard.as_ref().ok_or_else(no_session_error)?;

        match session.assert_text(&text, present).await {
            Ok(()) => Ok(CallToolResult::success(vec![ContentBlock::text(format!(
                "assertion passed: text {text:?} is {}present",
                if present { "" } else { "not " }
            ))])),
            Err(engine::EngineError::AssertionFailed {
                text,
                present,
                snapshot_excerpt,
            }) => Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                "assertion failed: expected text {text:?} to be {}present, but it was {}.\ncurrent snapshot (truncated):\n{snapshot_excerpt}",
                if present { "" } else { "not " },
                if present { "not found" } else { "found" }
            ))])),
            Err(e) => Err(map_engine_err(e)),
        }
    }

    #[tool(
        description = "Parse and run a YAML script's steps (navigate/click/type/press/wait_for/assert) against the current session, in order. Stops at the first failing step and reports which one."
    )]
    async fn browser_run_yaml(
        &self,
        Parameters(RunYamlRequest { source }): Parameters<RunYamlRequest>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_session().await?;
        let guard = self.session.lock().await;
        let session = guard.as_ref().ok_or_else(no_session_error)?;

        match session.run_yaml(&source).await {
            Ok(summary) => Ok(CallToolResult::success(vec![ContentBlock::text(format!(
                "script completed: {}/{} steps ran successfully",
                summary.steps_run, summary.total_steps
            ))])),
            Err(e @ engine::EngineError::YamlStepFailed { .. }) => {
                Ok(CallToolResult::error(vec![ContentBlock::text(
                    e.to_string(),
                )]))
            }
            Err(e) => Err(map_engine_err(e)),
        }
    }

    #[tool(
        description = "Export a saved console/action trace's actions (navigate/click/type/press/assert) as a runnable YAML script -- 'record once, get a script'. Console/exception entries are not included."
    )]
    async fn browser_export_yaml(
        &self,
        Parameters(ExportYamlRequest { name }): Parameters<ExportYamlRequest>,
    ) -> Result<CallToolResult, McpError> {
        let yaml = engine::Session::export_yaml(&name).map_err(map_engine_err)?;
        Ok(CallToolResult::success(vec![ContentBlock::text(yaml)]))
    }

    #[tool(
        description = "Render a saved trace (from browser_console_start/stop) as a self-contained HTML file, and return its path. Doesn't return the HTML itself -- open the path to view it."
    )]
    async fn browser_render_trace(
        &self,
        Parameters(ExportYamlRequest { name }): Parameters<ExportYamlRequest>,
    ) -> Result<CallToolResult, McpError> {
        let path = engine::render_trace_html(&name).map_err(map_engine_err)?;
        Ok(CallToolResult::success(vec![ContentBlock::text(
            path.display().to_string(),
        )]))
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

    #[tool(
        description = "Start capturing real, trusted mouse/keyboard input from the human physically using the browser window, for later human-like replay. Fails if a training session is already active."
    )]
    async fn browser_train_start(
        &self,
        Parameters(TrainStartRequest { name }): Parameters<TrainStartRequest>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_session().await?;

        let mut training_guard = self.training.lock().await;
        if training_guard.is_some() {
            return Err(McpError::invalid_params(
                "a training session is already in progress; call browser_train_stop first",
                None,
            ));
        }

        let session_guard = self.session.lock().await;
        let session = session_guard.as_ref().ok_or_else(no_session_error)?;

        let training = session.train_start(&name).await.map_err(map_engine_err)?;
        *training_guard = Some(training);

        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "training started for {name:?}: physically click and type on the page now. \
             Call browser_train_stop when done (auto-stops after 5 minutes)."
        ))]))
    }

    #[tool(
        description = "Stop the active training session and fit/save a persona from what was captured. Fails if too little was captured to fit."
    )]
    async fn browser_train_stop(&self) -> Result<CallToolResult, McpError> {
        let training =
            self.training.lock().await.take().ok_or_else(|| {
                McpError::invalid_params("no training session is in progress", None)
            })?;

        let stored = training.stop().await.map_err(map_engine_err)?;

        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "training saved as {:?}: fitted from {} mouse movement(s) and {} keystroke(s). \
             Use trained_profile: {:?} on browser_click/browser_type to replay it.",
            stored.name, stored.movements_captured, stored.keystrokes_captured, stored.name
        ))]))
    }

    #[tool(
        description = "Start passively recording real network traffic to a named cassette. Fails if a recording or replay is already active."
    )]
    async fn browser_network_record_start(
        &self,
        Parameters(NetworkNameRequest { name }): Parameters<NetworkNameRequest>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_session().await?;

        let mut network_recording_guard = self.network_recording.lock().await;
        if network_recording_guard.is_some() {
            return Err(McpError::invalid_params(
                "a network recording is already in progress; call browser_network_record_stop first",
                None,
            ));
        }
        if self.network_replay.lock().await.is_some() {
            return Err(McpError::invalid_params(
                "a network replay is active; call browser_network_replay_stop first",
                None,
            ));
        }

        let session_guard = self.session.lock().await;
        let session = session_guard.as_ref().ok_or_else(no_session_error)?;

        let network_recording = session
            .network_record_start(&name)
            .await
            .map_err(map_engine_err)?;
        *network_recording_guard = Some(network_recording);

        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "network recording started for {name:?}. Call browser_network_record_stop when done \
             (auto-stops after 5 minutes)."
        ))]))
    }

    #[tool(description = "Stop the active network recording and save the cassette")]
    async fn browser_network_record_stop(&self) -> Result<CallToolResult, McpError> {
        let network_recording =
            self.network_recording.lock().await.take().ok_or_else(|| {
                McpError::invalid_params("no network recording is in progress", None)
            })?;

        let summary = network_recording.stop().await.map_err(map_engine_err)?;

        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "cassette {:?} saved: {} request(s) recorded to {}",
            summary.name,
            summary.entry_count,
            summary.path.display()
        ))]))
    }

    #[tool(
        description = "Start replaying network traffic from a named cassette: every request is intercepted and fulfilled from the recording, with no live-network dependency. A request with no matching cassette entry fails loudly. Fails if a recording or replay is already active, or if the cassette doesn't exist."
    )]
    async fn browser_network_replay_start(
        &self,
        Parameters(NetworkNameRequest { name }): Parameters<NetworkNameRequest>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_session().await?;

        let mut network_replay_guard = self.network_replay.lock().await;
        if network_replay_guard.is_some() {
            return Err(McpError::invalid_params(
                "a network replay is already in progress; call browser_network_replay_stop first",
                None,
            ));
        }
        if self.network_recording.lock().await.is_some() {
            return Err(McpError::invalid_params(
                "a network recording is active; call browser_network_record_stop first",
                None,
            ));
        }

        let session_guard = self.session.lock().await;
        let session = session_guard.as_ref().ok_or_else(no_session_error)?;

        let network_replay = session
            .network_replay_start(&name)
            .await
            .map_err(map_engine_err)?;
        *network_replay_guard = Some(network_replay);

        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "network replay started from cassette {name:?}. Call browser_network_replay_stop to \
             return to normal network behavior."
        ))]))
    }

    #[tool(description = "Stop network replay and return to normal (live) network behavior")]
    async fn browser_network_replay_stop(&self) -> Result<CallToolResult, McpError> {
        let network_replay =
            self.network_replay.lock().await.take().ok_or_else(|| {
                McpError::invalid_params("no network replay is in progress", None)
            })?;

        network_replay.stop().await.map_err(map_engine_err)?;

        Ok(CallToolResult::success(vec![ContentBlock::text(
            "network replay stopped; requests now reach the live network again.",
        )]))
    }

    #[tool(
        description = "List every currently-attached page (the original page plus any popup/new tab opened as a side effect of interacting with it), showing which one is active"
    )]
    async fn browser_list_pages(&self) -> Result<CallToolResult, McpError> {
        self.ensure_session().await?;
        let guard = self.session.lock().await;
        let session = guard.as_ref().ok_or_else(no_session_error)?;
        let pages = session.list_pages().await.map_err(map_engine_err)?;
        let text = pages
            .iter()
            .map(|p| {
                format!(
                    "{}[{}] {} -- {}",
                    if p.active { "* " } else { "  " },
                    p.page_id,
                    p.url,
                    p.title
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    #[tool(
        description = "Switch which attached page subsequent actions (browser_click/browser_type/browser_snapshot/etc.) target. Use browser_list_pages to find the page_id, e.g. after clicking something that opens a popup or new tab."
    )]
    async fn browser_switch_page(
        &self,
        Parameters(SwitchPageRequest { page_id }): Parameters<SwitchPageRequest>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_session().await?;
        let guard = self.session.lock().await;
        let session = guard.as_ref().ok_or_else(no_session_error)?;
        session
            .switch_page(&page_id)
            .await
            .map_err(map_engine_err)?;
        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "switched to page {page_id}. Call browser_snapshot to see it."
        ))]))
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
                 browser_snapshot(), browser_click(ref, human_like?, persona?, trained_profile?, seed?, true_input?), \
                 browser_type(ref, text, submit?, human_like?, persona?, trained_profile?, seed?, true_input?), \
                 browser_press(key), browser_wait_for(text, timeout_ms?), browser_screenshot(), \
                 browser_record_start(max_duration_ms?, quality?), browser_record_stop(), \
                 browser_train_start(name), browser_train_stop(), \
                 browser_network_record_start(name), browser_network_record_stop(), \
                 browser_network_replay_start(name), browser_network_replay_stop(), \
                 browser_add_init_script(source), browser_seed_randomness(seed), \
                 browser_set_clock(time_ms), browser_advance_clock(ms), \
                 browser_console_start(name), browser_console_stop(), \
                 browser_assert(text, present?), \
                 browser_run_yaml(source), browser_export_yaml(name), \
                 browser_render_trace(name), \
                 browser_list_pages(), browser_switch_page(page_id), \
                 browser_close(). Refs come from the snapshot text, e.g. `[e6]` -> ref \"e6\". \
                 Actions do not auto-return a new snapshot; call browser_snapshot again after an \
                 action that may have changed the page. Use browser_record_start/stop to capture a \
                 short GIF of moving/animated parts instead of a single screenshot. Set human_like: \
                 true on browser_click/browser_type to move the mouse along a curved path and type \
                 character-by-character with human-like pauses instead of instant dispatch, e.g. \
                 for testing an application's bot-detection; persona is one of careful/average/fast \
                 (default average), and seed reproduces the exact same motion/timing on a later \
                 call. For an even more realistic profile, browser_train_start(name) captures a \
                 real human physically using the browser (call browser_train_stop when done), then \
                 pass trained_profile: name (instead of persona) on browser_click/browser_type to \
                 replay that human's fitted timing with fresh variability each call -- requesting a \
                 name that was never trained fails with a clear error rather than a silent fallback. \
                 Use browser_network_record_start(name)/stop to capture real network traffic to a \
                 named cassette, then browser_network_replay_start(name)/stop to replay a later run \
                 entirely from that cassette with no live-backend dependency -- a request with no \
                 matching recorded response fails loudly rather than silently reaching the real \
                 network, so an incomplete recording is obvious immediately. Use \
                 browser_add_init_script(source) to run JS before a page's own scripts (not just \
                 before an agent action, unlike snapshot/evaluate-based reads) -- register it, then \
                 call browser_navigate for it to take effect. browser_seed_randomness(seed) is the \
                 same mechanism pre-built to override Math.random with a deterministic PRNG, so an \
                 app's own random IDs/variants/animations become reproducible across runs with the \
                 same seed. browser_set_clock(time_ms) installs a virtual clock frozen at that epoch \
                 time (also register before browser_navigate) -- Date/performance.now/setTimeout/ \
                 setInterval/requestAnimationFrame all read from it, and time never moves on its own; \
                 call browser_advance_clock(ms) at any point afterward to move it forward and fire \
                 every due callback in order, e.g. to make a session-timeout warning or a debounced \
                 handler fire instantly and deterministically instead of waiting for real time or \
                 skipping the behavior. Use browser_console_start(name)/stop to capture the page's \
                 console.log/warn/error output and any uncaught exceptions to a named JSONL trace -- \
                 useful for finding out why a test failed or an app behaved unexpectedly (actions \
                 taken while a trace is active are logged into the same trace too). \
                 browser_assert(text, present?) checks the current snapshot immediately (no polling, \
                 unlike browser_wait_for) and fails as a real tool-call failure you should treat as a \
                 test failure, not a protocol error -- present defaults to true (assert the text is \
                 there); set it to false to assert the text is absent. browser_run_yaml(source) runs a \
                 YAML script's steps (navigate/click/type/press/wait_for/assert, one per line like \
                 `- click: e6` or `- type: {ref: e6, text: \"...\"}`) against the current session, \
                 stopping at the first failing step; browser_export_yaml(name) turns an already-saved \
                 trace's actions back into a runnable script of that same format -- record a flow once \
                 with browser_console_start/stop, then export and re-run it as a repeatable test. \
                 browser_render_trace(name) renders that same saved trace as a self-contained HTML file \
                 (returns its path, not the HTML itself) -- console/exception/action entries in \
                 chronological order, plus any browser_screenshot calls taken while the trace was \
                 active, shown inline. A \
                 popup or new tab opened as a side effect of an action (e.g. \"Sign in with Google\") \
                 attaches automatically but does NOT become active on its own -- call \
                 browser_list_pages() to see it (marked with its page_id) and browser_switch_page(page_id) \
                 to start driving it; if that page later closes itself (e.g. after an OAuth redirect \
                 completes), the active page falls back to the original one automatically. Set \
                 true_input: true on browser_click/browser_type to dispatch via real Windows OS-level \
                 input (SendInput) instead of CDP -- the actual system mouse cursor moves and clicks for \
                 real, and real keystrokes are sent, unlike every other mode here which is still \
                 CDP-synthesized even though Chrome marks it isTrusted; Windows-only, headed sessions \
                 only, rejected with a clear error otherwise."
                    .to_string(),
            )
    }
}

/// Resolves the `human_like`/`persona`/`trained_profile`/`seed` request
/// fields into an `engine::HumanLike`, or `None` for the default
/// instant-dispatch path. Specifying `persona` or `trained_profile` implies
/// human-like mode -- otherwise a caller who set `trained_profile` but left
/// `human_like` at its default `false` would silently get instant dispatch,
/// which defeats the point of naming a profile at all. Rejects an unknown
/// persona name or an untrained profile name up front, before any action is
/// taken (human-motion spec: "Persona presets" / "Untrained profile fails
/// clearly" — never a silent fallback).
fn build_human_like(
    human_like: bool,
    persona: Option<String>,
    trained_profile: Option<String>,
    seed: Option<u64>,
) -> Result<Option<engine::HumanLike>, McpError> {
    if !human_like && persona.is_none() && trained_profile.is_none() {
        return Ok(None);
    }
    let persona =
        engine::Session::persona_or_trained(persona.as_deref(), trained_profile.as_deref())
            .map_err(map_engine_err)?;
    Ok(Some(engine::HumanLike { persona, seed }))
}

fn seed_suffix(seed: Option<u64>) -> String {
    match seed {
        Some(seed) => format!(" (human-like, seed={seed})"),
        None => String::new(),
    }
}

fn map_engine_err(e: engine::EngineError) -> McpError {
    use engine::EngineError::*;
    match e {
        StaleRef(_)
        | UnknownKey(_)
        | UnknownPersona(_)
        | UntrainedProfile(_)
        | AmbiguousPersona
        | UnknownCassette(_)
        | Clock(_)
        | YamlRunner(_)
        | TrueInputUnsupported(_)
        | UnknownPage(_) => McpError::invalid_params(e.to_string(), None),
        ActionTimeout { .. }
        | WaitTimeout { .. }
        | Cdp(_)
        | Serde(_)
        | Recording(_)
        | Training(_)
        | Network(_)
        | Console(_)
        | TrueInput(_) => McpError::internal_error(e.to_string(), None),
        // Normally handled directly in browser_assert/browser_run_yaml as a
        // CallToolResult::error (a tool-level failure, not a protocol error);
        // these arms only fire if that error ever reaches this generic path
        // some other way.
        AssertionFailed { .. } => McpError::invalid_params(e.to_string(), None),
        YamlStepFailed { .. } => McpError::invalid_params(e.to_string(), None),
    }
}

fn no_session_error() -> McpError {
    McpError::internal_error("no active browser session", None)
}

fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}
