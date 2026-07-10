use crate::session::Command;
use serde::{Deserialize, Serialize};

pub struct CreateBrowserContext;
impl Command for CreateBrowserContext {
    const METHOD: &'static str = "Target.createBrowserContext";
    type Params = CreateBrowserContextParams;
    type Response = CreateBrowserContextResponse;
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBrowserContextParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dispose_on_detach: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBrowserContextResponse {
    pub browser_context_id: String,
}

pub struct DisposeBrowserContext;
impl Command for DisposeBrowserContext {
    const METHOD: &'static str = "Target.disposeBrowserContext";
    type Params = DisposeBrowserContextParams;
    type Response = super::EmptyResponse;
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisposeBrowserContextParams {
    pub browser_context_id: String,
}

pub struct CreateTarget;
impl Command for CreateTarget {
    const METHOD: &'static str = "Target.createTarget";
    type Params = CreateTargetParams;
    type Response = CreateTargetResponse;
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTargetParams {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_context_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTargetResponse {
    pub target_id: String,
}

pub struct AttachToTarget;
impl Command for AttachToTarget {
    const METHOD: &'static str = "Target.attachToTarget";
    type Params = AttachToTargetParams;
    type Response = AttachToTargetResponse;
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachToTargetParams {
    pub target_id: String,
    pub flatten: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachToTargetResponse {
    pub session_id: String,
}

pub struct CloseTarget;
impl Command for CloseTarget {
    const METHOD: &'static str = "Target.closeTarget";
    type Params = CloseTargetParams;
    type Response = CloseTargetResponse;
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseTargetParams {
    pub target_id: String,
}

#[derive(Debug, Deserialize)]
pub struct CloseTargetResponse {
    pub success: bool,
}
