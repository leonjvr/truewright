use crate::session::Command;
use serde::Serialize;

pub struct DispatchMouseEvent;
impl Command for DispatchMouseEvent {
    const METHOD: &'static str = "Input.dispatchMouseEvent";
    type Params = DispatchMouseEventParams;
    type Response = super::EmptyResponse;
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchMouseEventParams {
    #[serde(rename = "type")]
    pub kind: String,
    pub x: f64,
    pub y: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub button: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub click_count: Option<i64>,
}

pub struct DispatchKeyEvent;
impl Command for DispatchKeyEvent {
    const METHOD: &'static str = "Input.dispatchKeyEvent";
    type Params = DispatchKeyEventParams;
    type Response = super::EmptyResponse;
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchKeyEventParams {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub windows_virtual_key_code: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

pub struct InsertText;
impl Command for InsertText {
    const METHOD: &'static str = "Input.insertText";
    type Params = InsertTextParams;
    type Response = super::EmptyResponse;
}

#[derive(Debug, Serialize)]
pub struct InsertTextParams {
    pub text: String,
}
