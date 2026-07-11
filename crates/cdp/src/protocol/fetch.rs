//! Active request interception (network-mocking spec: "Replay from a
//! cassette with no live-network dependency"). Unlike `Network` (passive
//! observation), enabling `Fetch` pauses every matching request until this
//! engine explicitly resolves it via `FulfillRequest` or `FailRequest`.

use crate::session::{CdpEvent, Command};
use serde::{Deserialize, Serialize};

pub struct Enable;
impl Command for Enable {
    const METHOD: &'static str = "Fetch.enable";
    type Params = super::EmptyParams;
    type Response = super::EmptyResponse;
}

pub struct Disable;
impl Command for Disable {
    const METHOD: &'static str = "Fetch.disable";
    type Params = super::EmptyParams;
    type Response = super::EmptyResponse;
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestPaused {
    pub request_id: String,
    pub request: PausedRequest,
}
impl CdpEvent for RequestPaused {
    const METHOD: &'static str = "Fetch.requestPaused";
}

#[derive(Debug, Clone, Deserialize)]
pub struct PausedRequest {
    pub url: String,
    pub method: String,
}

pub struct FulfillRequest;
impl Command for FulfillRequest {
    const METHOD: &'static str = "Fetch.fulfillRequest";
    type Params = FulfillRequestParams;
    type Response = super::EmptyResponse;
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FulfillRequestParams {
    pub request_id: String,
    pub response_code: i64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub response_headers: Vec<HeaderEntry>,
    /// Base64-encoded response body.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HeaderEntry {
    pub name: String,
    pub value: String,
}

pub struct FailRequest;
impl Command for FailRequest {
    const METHOD: &'static str = "Fetch.failRequest";
    type Params = FailRequestParams;
    type Response = super::EmptyResponse;
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FailRequestParams {
    pub request_id: String,
    pub error_reason: String,
}
