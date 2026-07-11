use crate::session::{CdpEvent, Command};
use serde::{Deserialize, Serialize};

pub struct Enable;
impl Command for Enable {
    const METHOD: &'static str = "Page.enable";
    type Params = super::EmptyParams;
    type Response = super::EmptyResponse;
}

pub struct SetLifecycleEventsEnabled;
impl Command for SetLifecycleEventsEnabled {
    const METHOD: &'static str = "Page.setLifecycleEventsEnabled";
    type Params = SetLifecycleEventsEnabledParams;
    type Response = super::EmptyResponse;
}

#[derive(Debug, Serialize)]
pub struct SetLifecycleEventsEnabledParams {
    pub enabled: bool,
}

/// Registers a script that runs before any of a page's own scripts, on
/// every subsequent navigation -- unlike `Runtime.evaluate`, which only
/// runs after a page has already loaded (deterministic-init spec: "Init
/// scripts run before a page's own scripts").
pub struct AddScriptToEvaluateOnNewDocument;
impl Command for AddScriptToEvaluateOnNewDocument {
    const METHOD: &'static str = "Page.addScriptToEvaluateOnNewDocument";
    type Params = AddScriptToEvaluateOnNewDocumentParams;
    type Response = AddScriptToEvaluateOnNewDocumentResponse;
}

#[derive(Debug, Serialize)]
pub struct AddScriptToEvaluateOnNewDocumentParams {
    pub source: String,
}

#[derive(Debug, Deserialize)]
pub struct AddScriptToEvaluateOnNewDocumentResponse {
    pub identifier: String,
}

pub struct Navigate;
impl Command for Navigate {
    const METHOD: &'static str = "Page.navigate";
    type Params = NavigateParams;
    type Response = NavigateResponse;
}

#[derive(Debug, Serialize)]
pub struct NavigateParams {
    pub url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigateResponse {
    pub frame_id: String,
    #[serde(default)]
    pub loader_id: Option<String>,
    #[serde(default, rename = "errorText")]
    pub error_text: Option<String>,
}

pub struct CaptureScreenshot;
impl Command for CaptureScreenshot {
    const METHOD: &'static str = "Page.captureScreenshot";
    type Params = CaptureScreenshotParams;
    type Response = CaptureScreenshotResponse;
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureScreenshotParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CaptureScreenshotResponse {
    /// Base64-encoded image bytes.
    pub data: String,
}

/// Fired for each named lifecycle milestone (`init`, `load`,
/// `networkAlmostIdle`, ...). Used for event-driven navigation waiting
/// instead of polling (cdp-client spec: "Navigation-complete semantics").
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleEvent {
    pub frame_id: String,
    #[serde(default)]
    pub loader_id: Option<String>,
    pub name: String,
    pub timestamp: f64,
}
impl CdpEvent for LifecycleEvent {
    const METHOD: &'static str = "Page.lifecycleEvent";
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoadEventFired {
    pub timestamp: f64,
}
impl CdpEvent for LoadEventFired {
    const METHOD: &'static str = "Page.loadEventFired";
}

pub struct StartScreencast;
impl Command for StartScreencast {
    const METHOD: &'static str = "Page.startScreencast";
    type Params = StartScreencastParams;
    type Response = super::EmptyResponse;
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartScreencastParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_width: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_height: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub every_nth_frame: Option<i64>,
}

pub struct StopScreencast;
impl Command for StopScreencast {
    const METHOD: &'static str = "Page.stopScreencast";
    type Params = super::EmptyParams;
    type Response = super::EmptyResponse;
}

pub struct ScreencastFrameAck;
impl Command for ScreencastFrameAck {
    const METHOD: &'static str = "Page.screencastFrameAck";
    type Params = ScreencastFrameAckParams;
    type Response = super::EmptyResponse;
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreencastFrameAckParams {
    /// The frame's own `frame_ack_id` (browser-recording spec: distinct
    /// from the CDP session id used for command/event routing).
    pub session_id: i64,
}

/// One captured frame (browser-recording spec: "Screencast-based frame
/// capture"). `frame_ack_id` is CDP's own per-frame sequence number, used
/// only to ack this specific frame — unrelated to the CDP `sessionId` used
/// to route commands/events to this page.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreencastFrame {
    /// Base64-encoded JPEG bytes.
    pub data: String,
    pub metadata: ScreencastFrameMetadata,
    #[serde(rename = "sessionId")]
    pub frame_ack_id: i64,
}
impl CdpEvent for ScreencastFrame {
    const METHOD: &'static str = "Page.screencastFrame";
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScreencastFrameMetadata {
    pub timestamp: f64,
}
