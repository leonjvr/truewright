use crate::session::Command;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub struct Enable;
impl Command for Enable {
    const METHOD: &'static str = "Runtime.enable";
    type Params = super::EmptyParams;
    type Response = super::EmptyResponse;
}

pub struct Evaluate;
impl Command for Evaluate {
    const METHOD: &'static str = "Runtime.evaluate";
    type Params = EvaluateParams;
    type Response = EvaluateResponse;
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluateParams {
    pub expression: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_by_value: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub await_promise: Option<bool>,
}

impl EvaluateParams {
    pub fn new(expression: impl Into<String>) -> Self {
        Self {
            expression: expression.into(),
            return_by_value: Some(true),
            await_promise: Some(true),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluateResponse {
    pub result: RemoteObject,
    #[serde(default)]
    pub exception_details: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteObject {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub subtype: Option<String>,
    #[serde(default)]
    pub value: Option<Value>,
    #[serde(default)]
    pub description: Option<String>,
}
