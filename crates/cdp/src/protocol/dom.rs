//! `DOM` domain -- used only for cross-origin-OOPIF frame-to-element
//! correlation (cross-origin-oopif spec: `DOM.getFrameOwner` finds which
//! `<iframe>` element in the parent owns a given cross-process frame,
//! `DOM.resolveNode` turns that into a live `Runtime` object handle).

use super::EmptyResponse;
use crate::session::Command;
use serde::{Deserialize, Serialize};

pub struct Enable;
impl Command for Enable {
    const METHOD: &'static str = "DOM.enable";
    type Params = super::EmptyParams;
    type Response = EmptyResponse;
}

pub struct GetFrameOwner;
impl Command for GetFrameOwner {
    const METHOD: &'static str = "DOM.getFrameOwner";
    type Params = GetFrameOwnerParams;
    type Response = GetFrameOwnerResponse;
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetFrameOwnerParams {
    pub frame_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetFrameOwnerResponse {
    pub backend_node_id: i64,
}

pub struct ResolveNode;
impl Command for ResolveNode {
    const METHOD: &'static str = "DOM.resolveNode";
    type Params = ResolveNodeParams;
    type Response = ResolveNodeResponse;
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveNodeParams {
    pub backend_node_id: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveNodeResponse {
    pub object: super::runtime::RemoteObject,
}
