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
