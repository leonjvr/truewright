//! High-level helper tying the primitives together: connect → context →
//! page → navigate → evaluate → screenshot → teardown (tasks.md 4.1). This
//! is the seam the daemon's `engine` crate will build sessions on in Phase 1.

use crate::connection::Connection;
use crate::error::{CdpError, Result};
use crate::launch::{self, BrowserKind, DiscoveredBrowser, LaunchedBrowser};
use crate::protocol::{browser, fetch, input, network, page, runtime, target};
use crate::session::{CdpEvent, EventItem, EventStream, Session};
use base64::Engine;
use std::time::Duration;

pub struct Browser {
    conn: Connection,
    session: Session,
}

impl Browser {
    pub async fn connect(ws_url: &str) -> Result<Self> {
        let conn = Connection::connect(ws_url).await?;
        let session = conn.browser_session();
        Ok(Self { conn, session })
    }

    pub async fn version(&self) -> Result<browser::GetVersionResponse> {
        self.session
            .execute::<browser::GetVersion>(Default::default())
            .await
    }

    pub async fn new_context(&self) -> Result<BrowserContext> {
        let resp = self
            .session
            .execute::<target::CreateBrowserContext>(Default::default())
            .await?;
        Ok(BrowserContext {
            conn: self.conn.clone(),
            session: self.session.clone(),
            context_id: resp.browser_context_id,
        })
    }
}

pub struct BrowserContext {
    conn: Connection,
    session: Session,
    context_id: String,
}

impl BrowserContext {
    pub async fn new_page(&self, url: &str) -> Result<Page> {
        let created = self
            .session
            .execute::<target::CreateTarget>(target::CreateTargetParams {
                url: url.to_string(),
                browser_context_id: Some(self.context_id.clone()),
            })
            .await?;

        let attached = self
            .session
            .execute::<target::AttachToTarget>(target::AttachToTargetParams {
                target_id: created.target_id.clone(),
                flatten: true,
            })
            .await?;

        let page_session = self.conn.session(attached.session_id);
        page_session
            .execute::<page::Enable>(Default::default())
            .await?;
        page_session
            .execute::<page::SetLifecycleEventsEnabled>(page::SetLifecycleEventsEnabledParams {
                enabled: true,
            })
            .await?;
        page_session
            .execute::<runtime::Enable>(Default::default())
            .await?;

        Ok(Page {
            browser_session: self.session.clone(),
            session: page_session,
            target_id: created.target_id,
        })
    }

    /// Disposes this context (browser-attach spec: "Clean teardown").
    pub async fn dispose(&self) -> Result<()> {
        self.session
            .execute::<target::DisposeBrowserContext>(target::DisposeBrowserContextParams {
                browser_context_id: self.context_id.clone(),
            })
            .await?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct Page {
    browser_session: Session,
    session: Session,
    target_id: String,
}

impl Page {
    /// Navigates and waits for the `load` lifecycle milestone, event-driven
    /// (no polling) and raced against `timeout` (cdp-client spec:
    /// "Navigation-complete semantics").
    pub async fn navigate_and_wait(&self, url: &str, timeout: Duration) -> Result<()> {
        let mut lifecycle = self.session.events::<page::LifecycleEvent>();
        let mut load_fired = self.session.events::<page::LoadEventFired>();

        self.session
            .execute::<page::Navigate>(page::NavigateParams {
                url: url.to_string(),
            })
            .await?;

        let wait = async {
            loop {
                tokio::select! {
                    ev = lifecycle.next() => {
                        if let Some(EventItem::Event(ev)) = ev {
                            if ev.name == "load" {
                                return;
                            }
                        }
                    }
                    ev = load_fired.next() => {
                        if matches!(ev, Some(EventItem::Event(_))) {
                            return;
                        }
                    }
                }
            }
        };

        tokio::time::timeout(timeout, wait)
            .await
            .map_err(|_| CdpError::Timeout(timeout))
    }

    /// Registers a script that runs before any of a page's own scripts, on
    /// every subsequent navigation (deterministic-init spec). Returns the
    /// CDP-assigned identifier.
    pub async fn add_init_script(&self, source: &str) -> Result<String> {
        let resp = self
            .session
            .execute::<page::AddScriptToEvaluateOnNewDocument>(
                page::AddScriptToEvaluateOnNewDocumentParams {
                    source: source.to_string(),
                },
            )
            .await?;
        Ok(resp.identifier)
    }

    /// The on-screen bounds of this page's native window (true-user-input
    /// spec: window discovery). A browser-level command (needs `targetId`
    /// explicitly since there's no page-session-implicit target for it).
    pub async fn window_bounds(&self) -> Result<browser::GetWindowForTargetResponse> {
        self.browser_session
            .execute::<browser::GetWindowForTarget>(browser::GetWindowForTargetParams {
                target_id: self.target_id.clone(),
            })
            .await
    }

    pub async fn evaluate(&self, expression: &str) -> Result<serde_json::Value> {
        let resp = self
            .session
            .execute::<runtime::Evaluate>(runtime::EvaluateParams::new(expression))
            .await?;
        if let Some(details) = resp.exception_details {
            return Err(CdpError::Other(format!("evaluate threw: {details}")));
        }
        Ok(resp.result.value.unwrap_or(serde_json::Value::Null))
    }

    pub async fn screenshot(&self) -> Result<Vec<u8>> {
        let resp = self
            .session
            .execute::<page::CaptureScreenshot>(Default::default())
            .await?;
        base64::engine::general_purpose::STANDARD
            .decode(resp.data)
            .map_err(|e| CdpError::Other(format!("invalid screenshot base64: {e}")))
    }

    pub async fn close(&self) -> Result<()> {
        self.browser_session
            .execute::<target::CloseTarget>(target::CloseTargetParams {
                target_id: self.target_id.clone(),
            })
            .await?;
        Ok(())
    }

    /// Left-clicks at viewport coordinates (press + release), for the
    /// `engine` crate's ref-resolved actions.
    pub async fn click_at(&self, x: f64, y: f64) -> Result<()> {
        for kind in ["mousePressed", "mouseReleased"] {
            self.session
                .execute::<input::DispatchMouseEvent>(input::DispatchMouseEventParams {
                    kind: kind.to_string(),
                    x,
                    y,
                    button: Some("left".to_string()),
                    click_count: Some(1),
                })
                .await?;
        }
        Ok(())
    }

    /// Inserts text into whatever element currently has focus.
    pub async fn insert_text(&self, text: &str) -> Result<()> {
        self.session
            .execute::<input::InsertText>(input::InsertTextParams {
                text: text.to_string(),
            })
            .await?;
        Ok(())
    }

    /// Dispatches a single `mouseMoved` event, for human-motion's curved,
    /// timed mouse paths (browser-actions / human-motion spec).
    pub async fn move_mouse_to(&self, x: f64, y: f64) -> Result<()> {
        self.session
            .execute::<input::DispatchMouseEvent>(input::DispatchMouseEventParams {
                kind: "mouseMoved".to_string(),
                x,
                y,
                button: None,
                click_count: None,
            })
            .await?;
        Ok(())
    }

    /// Dispatches a single `char` key event carrying `ch`, for human-motion's
    /// per-character typing cadence (extends `dispatch_key`'s named-key-only
    /// keyDown/keyUp pair to arbitrary Unicode characters).
    pub async fn dispatch_char(&self, ch: char) -> Result<()> {
        self.session
            .execute::<input::DispatchKeyEvent>(input::DispatchKeyEventParams {
                kind: "char".to_string(),
                key: None,
                code: None,
                windows_virtual_key_code: None,
                text: Some(ch.to_string()),
            })
            .await?;
        Ok(())
    }

    /// Dispatches a keyDown/keyUp pair for a named key.
    pub async fn dispatch_key(
        &self,
        key: &str,
        code: &str,
        windows_virtual_key_code: i64,
    ) -> Result<()> {
        for kind in ["keyDown", "keyUp"] {
            self.session
                .execute::<input::DispatchKeyEvent>(input::DispatchKeyEventParams {
                    kind: kind.to_string(),
                    key: Some(key.to_string()),
                    code: Some(code.to_string()),
                    windows_virtual_key_code: Some(windows_virtual_key_code),
                    text: None,
                })
                .await?;
        }
        Ok(())
    }

    /// Subscribes to a typed event on this page's session (browser-recording
    /// spec: rides the same bounded event-stream infrastructure as
    /// `navigate_and_wait`'s lifecycle events).
    pub fn events<E: CdpEvent>(&self) -> EventStream<E> {
        self.session.events::<E>()
    }

    /// Exposes `window.<name>(payload)` in the page; calls surface as
    /// `BindingCalled` events on this page's `events::<BindingCalled>()`
    /// stream (human-motion spec: training capture).
    pub async fn add_binding(&self, name: &str) -> Result<()> {
        self.session
            .execute::<runtime::AddBinding>(runtime::AddBindingParams {
                name: name.to_string(),
            })
            .await?;
        Ok(())
    }

    pub async fn remove_binding(&self, name: &str) -> Result<()> {
        self.session
            .execute::<runtime::RemoveBinding>(runtime::RemoveBindingParams {
                name: name.to_string(),
            })
            .await?;
        Ok(())
    }

    /// Starts passive network observation: `requestWillBeSent`/
    /// `responseReceived`/`loadingFinished` events start flowing on this
    /// page's session (network-mocking spec: "Passive network recording").
    pub async fn enable_network_capture(&self) -> Result<()> {
        self.session
            .execute::<network::Enable>(Default::default())
            .await?;
        Ok(())
    }

    /// Fetches a completed request's response body. Only valid after that
    /// request's `loadingFinished` event has fired.
    pub async fn get_response_body(&self, request_id: &str) -> Result<(String, bool)> {
        let resp = self
            .session
            .execute::<network::GetResponseBody>(network::GetResponseBodyParams {
                request_id: request_id.to_string(),
            })
            .await?;
        Ok((resp.body, resp.base64_encoded))
    }

    /// Starts intercepting every request: each one pauses (surfaced as a
    /// `RequestPaused` event) until `fulfill_request`/`fail_request` is
    /// called (network-mocking spec: "Replay from a cassette").
    pub async fn enable_request_interception(&self) -> Result<()> {
        self.session
            .execute::<fetch::Enable>(Default::default())
            .await?;
        Ok(())
    }

    pub async fn disable_request_interception(&self) -> Result<()> {
        self.session
            .execute::<fetch::Disable>(Default::default())
            .await?;
        Ok(())
    }

    /// Resolves a paused request with a substituted response.
    /// `body_base64` is the base64-encoded response body.
    pub async fn fulfill_request(
        &self,
        request_id: &str,
        status: i64,
        headers: Vec<(String, String)>,
        body_base64: Option<String>,
    ) -> Result<()> {
        self.session
            .execute::<fetch::FulfillRequest>(fetch::FulfillRequestParams {
                request_id: request_id.to_string(),
                response_code: status,
                response_headers: headers
                    .into_iter()
                    .map(|(name, value)| fetch::HeaderEntry { name, value })
                    .collect(),
                body: body_base64,
            })
            .await?;
        Ok(())
    }

    /// Resolves a paused request as a network failure (network-mocking
    /// spec: "Unmatched replay requests fail loudly").
    pub async fn fail_request(&self, request_id: &str) -> Result<()> {
        self.session
            .execute::<fetch::FailRequest>(fetch::FailRequestParams {
                request_id: request_id.to_string(),
                error_reason: "Failed".to_string(),
            })
            .await?;
        Ok(())
    }

    pub async fn start_screencast(&self, params: page::StartScreencastParams) -> Result<()> {
        self.session
            .execute::<page::StartScreencast>(params)
            .await?;
        Ok(())
    }

    pub async fn stop_screencast(&self) -> Result<()> {
        self.session
            .execute::<page::StopScreencast>(Default::default())
            .await?;
        Ok(())
    }

    pub async fn ack_screencast_frame(&self, frame_ack_id: i64) -> Result<()> {
        self.session
            .execute::<page::ScreencastFrameAck>(page::ScreencastFrameAckParams {
                session_id: frame_ack_id,
            })
            .await?;
        Ok(())
    }
}

/// Result of one attach→navigate→evaluate→screenshot→teardown cycle
/// against a single browser (doctor-cli spec: "Full-cycle self-check per
/// browser").
pub struct CycleReport {
    pub kind: BrowserKind,
    pub title: serde_json::Value,
    pub screenshot_bytes: usize,
}

/// Launches `discovered`, runs the full cycle against `https://example.com`,
/// and always tears the browser process down again — even on failure — so a
/// failed doctor step never leaves an orphaned process.
pub async fn run_full_cycle(
    discovered: &DiscoveredBrowser,
    profile_name: &str,
    headless: bool,
) -> Result<CycleReport> {
    let launched = launch::launch(discovered, profile_name, headless).await?;
    let ws_url = launched.ws_url.clone();

    let result = run_full_cycle_inner(&ws_url).await;
    teardown(launched).await;

    let (title, screenshot_len) = result?;
    Ok(CycleReport {
        kind: discovered.kind,
        title,
        screenshot_bytes: screenshot_len,
    })
}

async fn run_full_cycle_inner(ws_url: &str) -> Result<(serde_json::Value, usize)> {
    let browser = Browser::connect(ws_url).await?;
    let context = browser.new_context().await?;
    let page = context.new_page("about:blank").await?;
    page.navigate_and_wait("https://example.com", Duration::from_secs(15))
        .await?;
    let title = page.evaluate("document.title").await?;
    let screenshot = page.screenshot().await?;
    page.close().await?;
    context.dispose().await?;
    Ok((title, screenshot.len()))
}

async fn teardown(launched: LaunchedBrowser) {
    if let Err(e) = launched.shutdown().await {
        tracing::warn!(error = %e, "browser shutdown failed");
    }
}
