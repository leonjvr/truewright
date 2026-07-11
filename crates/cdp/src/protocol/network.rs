//! Passive network observation (network-mocking spec: "Passive network
//! recording to a named cassette"). `Network.enable` + these three events
//! give method+URL (`RequestWillBeSent`), status+headers
//! (`ResponseReceived`), and body availability (`LoadingFinished`,
//! after which `GetResponseBody` can be called) for the same `requestId`.

use crate::session::{CdpEvent, Command};
use serde::{Deserialize, Serialize};

pub struct Enable;
impl Command for Enable {
    const METHOD: &'static str = "Network.enable";
    type Params = super::EmptyParams;
    type Response = super::EmptyResponse;
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestWillBeSent {
    pub request_id: String,
    pub request: RequestInfo,
}
impl CdpEvent for RequestWillBeSent {
    const METHOD: &'static str = "Network.requestWillBeSent";
}

#[derive(Debug, Clone, Deserialize)]
pub struct RequestInfo {
    pub url: String,
    pub method: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseReceived {
    pub request_id: String,
    pub response: ResponseInfo,
}
impl CdpEvent for ResponseReceived {
    const METHOD: &'static str = "Network.responseReceived";
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponseInfo {
    pub status: i64,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadingFinished {
    pub request_id: String,
}
impl CdpEvent for LoadingFinished {
    const METHOD: &'static str = "Network.loadingFinished";
}

pub struct GetResponseBody;
impl Command for GetResponseBody {
    const METHOD: &'static str = "Network.getResponseBody";
    type Params = GetResponseBodyParams;
    type Response = GetResponseBodyResponse;
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetResponseBodyParams {
    pub request_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetResponseBodyResponse {
    pub body: String,
    pub base64_encoded: bool,
}
