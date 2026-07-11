use crate::session::Command;
use serde::{Deserialize, Serialize};

pub struct GetVersion;
impl Command for GetVersion {
    const METHOD: &'static str = "Browser.getVersion";
    type Params = super::EmptyParams;
    type Response = GetVersionResponse;
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetVersionResponse {
    pub protocol_version: String,
    pub product: String,
    pub revision: String,
    pub user_agent: String,
    pub js_version: String,
}

/// Reports the on-screen bounds of the native window hosting a target
/// (true-user-input spec: used to disambiguate a browser process's OS
/// windows when it owns more than one -- e.g. every headed session's
/// leftover initial-launch window alongside the isolated context's actual
/// window; see design.md addendum).
pub struct GetWindowForTarget;
impl Command for GetWindowForTarget {
    const METHOD: &'static str = "Browser.getWindowForTarget";
    type Params = GetWindowForTargetParams;
    type Response = GetWindowForTargetResponse;
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetWindowForTargetParams {
    pub target_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetWindowForTargetResponse {
    pub window_id: i64,
    pub bounds: Bounds,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bounds {
    #[serde(default)]
    pub left: Option<i64>,
    #[serde(default)]
    pub top: Option<i64>,
    #[serde(default)]
    pub width: Option<i64>,
    #[serde(default)]
    pub height: Option<i64>,
    #[serde(default)]
    pub window_state: Option<String>,
}
