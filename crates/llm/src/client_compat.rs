//! OpenAI-compatible `/chat/completions` client (llm-providers spec:
//! "OpenAI-compatible chat completions client"). Every provider this change
//! targets (DeepSeek, MiniMax, GLM, Grok, and OpenAI itself via an API key)
//! speaks this exact wire shape -- one client, driven by per-provider
//! base_url/credential/headers rather than per-provider code.

use crate::auth::CredentialSource;
use crate::error::{LlmError, Result};
use crate::types::{ChatRequest, ChatResponse, FinishReason, Message, Part, Role, ToolCall, Usage};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Duration;

pub struct CompatClient {
    http: reqwest::Client,
    base_url: String,
    credential: CredentialSource,
    headers: BTreeMap<String, String>,
}

impl CompatClient {
    pub fn new(
        base_url: String,
        credential: CredentialSource,
        headers: BTreeMap<String, String>,
    ) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url,
            credential,
            headers,
        }
    }

    /// Posts `req` to `{base_url}/chat/completions`, retrying transient
    /// failures (HTTP 429 or 5xx) up to twice more with exponential
    /// backoff. A non-transient failure (4xx other than 429, or a retry
    /// budget exhausted) surfaces as a typed error rather than being
    /// silently swallowed.
    pub async fn complete(&self, req: &ChatRequest) -> Result<ChatResponse> {
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let wire_req = WireChatRequest::from_neutral(req);

        let mut attempt: u32 = 0;
        loop {
            attempt += 1;
            let bearer = self.credential.bearer().await?;
            let mut builder = self.http.post(&url).bearer_auth(bearer).json(&wire_req);
            for (name, value) in &self.headers {
                builder = builder.header(name.as_str(), value.as_str());
            }

            let resp = builder.send().await.map_err(|source| LlmError::Http {
                url: url.clone(),
                source,
            })?;
            let status = resp.status();

            if status.is_success() {
                let wire_resp: WireChatResponse =
                    resp.json()
                        .await
                        .map_err(|source| LlmError::ResponseParse {
                            url: url.clone(),
                            source,
                        })?;
                return Ok(wire_resp.into_neutral());
            }

            let retryable = status.as_u16() == 429 || status.is_server_error();
            if retryable && attempt < 3 {
                let backoff_ms = 500u64 * (1u64 << (attempt - 1));
                tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                continue;
            }

            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::HttpStatus {
                url,
                status: status.as_u16(),
                body,
            });
        }
    }
}

// ---- wire format (private -- OpenAI's specific JSON shape, not exposed) ----

#[derive(Serialize)]
struct WireChatRequest {
    model: String,
    messages: Vec<WireMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<WireToolDef>,
}

#[derive(Serialize)]
struct WireMessage {
    role: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<WireContent>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tool_calls: Vec<WireToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum WireContent {
    Text(String),
    Parts(Vec<WireContentPart>),
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WireContentPart {
    Text { text: String },
    ImageUrl { image_url: WireImageUrl },
}

#[derive(Serialize)]
struct WireImageUrl {
    url: String,
}

#[derive(Serialize)]
struct WireToolCall {
    id: String,
    #[serde(rename = "type")]
    kind: &'static str,
    function: WireFunctionCall,
}

#[derive(Serialize)]
struct WireFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Serialize)]
struct WireToolDef {
    #[serde(rename = "type")]
    kind: &'static str,
    function: WireFunctionDef,
}

#[derive(Serialize)]
struct WireFunctionDef {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Deserialize)]
struct WireChatResponse {
    choices: Vec<WireChoice>,
    #[serde(default)]
    usage: Option<WireUsage>,
}

#[derive(Deserialize)]
struct WireChoice {
    message: WireRespMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct WireRespMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<WireRespToolCall>,
}

#[derive(Deserialize)]
struct WireRespToolCall {
    id: String,
    function: WireRespFunctionCall,
}

#[derive(Deserialize)]
struct WireRespFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct WireUsage {
    #[serde(default)]
    prompt_tokens: u64,
    #[serde(default)]
    completion_tokens: u64,
    #[serde(default)]
    total_tokens: u64,
}

fn role_str(role: Role) -> &'static str {
    match role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Tool => "tool",
    }
}

fn content_to_wire(parts: &[Part]) -> Option<WireContent> {
    if parts.is_empty() {
        return None;
    }
    let has_image = parts.iter().any(|p| !matches!(p, Part::Text(_)));
    if !has_image {
        let joined: String = parts
            .iter()
            .map(|p| match p {
                Part::Text(t) => t.as_str(),
                _ => unreachable!("checked above"),
            })
            .collect();
        return Some(WireContent::Text(joined));
    }
    let wire_parts = parts
        .iter()
        .map(|p| match p {
            Part::Text(t) => WireContentPart::Text { text: t.clone() },
            Part::ImagePngB64(b64) => WireContentPart::ImageUrl {
                image_url: WireImageUrl {
                    url: format!("data:image/png;base64,{b64}"),
                },
            },
            Part::ImageJpegB64(b64) => WireContentPart::ImageUrl {
                image_url: WireImageUrl {
                    url: format!("data:image/jpeg;base64,{b64}"),
                },
            },
        })
        .collect();
    Some(WireContent::Parts(wire_parts))
}

impl WireChatRequest {
    fn from_neutral(req: &ChatRequest) -> Self {
        let messages = req
            .messages
            .iter()
            .map(|m| WireMessage {
                role: role_str(m.role),
                content: content_to_wire(&m.content),
                tool_calls: m
                    .tool_calls
                    .iter()
                    .map(|tc| WireToolCall {
                        id: tc.id.clone(),
                        kind: "function",
                        function: WireFunctionCall {
                            name: tc.name.clone(),
                            arguments: tc.arguments.clone(),
                        },
                    })
                    .collect(),
                tool_call_id: m.tool_call_id.clone(),
            })
            .collect();
        let tools = req
            .tools
            .iter()
            .map(|t| WireToolDef {
                kind: "function",
                function: WireFunctionDef {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: t.parameters.clone(),
                },
            })
            .collect();
        WireChatRequest {
            model: req.model.clone(),
            messages,
            tools,
        }
    }
}

impl WireChatResponse {
    fn into_neutral(self) -> ChatResponse {
        let choice = self.choices.into_iter().next();
        let (message, finish_reason) = match choice {
            Some(c) => {
                let content = c.message.content.filter(|s| !s.is_empty());
                let tool_calls = c
                    .message
                    .tool_calls
                    .into_iter()
                    .map(|tc| ToolCall {
                        id: tc.id,
                        name: tc.function.name,
                        arguments: tc.function.arguments,
                    })
                    .collect();
                let message = Message {
                    role: Role::Assistant,
                    content: content.map(|t| vec![Part::Text(t)]).unwrap_or_default(),
                    tool_calls,
                    tool_call_id: None,
                };
                let finish_reason = match c.finish_reason.as_deref() {
                    Some("stop") => FinishReason::Stop,
                    Some("tool_calls") => FinishReason::ToolCalls,
                    Some("length") => FinishReason::Length,
                    _ => FinishReason::Other,
                };
                (message, finish_reason)
            }
            None => (
                Message {
                    role: Role::Assistant,
                    ..Default::default()
                },
                FinishReason::Other,
            ),
        };
        let usage = self.usage.map(|u| Usage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        });
        ChatResponse {
            message,
            finish_reason,
            usage,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ToolDef;

    #[test]
    fn text_only_message_serializes_as_a_plain_string() {
        let req = ChatRequest {
            model: "test-model".to_string(),
            messages: vec![Message::system("be helpful"), Message::user("hello")],
            tools: vec![],
        };
        let value = serde_json::to_value(WireChatRequest::from_neutral(&req)).unwrap();
        assert_eq!(value["messages"][0]["role"], "system");
        assert_eq!(value["messages"][0]["content"], "be helpful");
        assert_eq!(value["messages"][1]["content"], "hello");
        // No tools were passed -- the field should be entirely absent, not `[]`,
        // since some providers reject an empty tools array.
        assert!(value.get("tools").is_none());
    }

    #[test]
    fn image_message_serializes_as_content_parts_with_a_data_uri() {
        let req = ChatRequest {
            model: "test-model".to_string(),
            messages: vec![Message::user_with_image(
                "what's in this screenshot?",
                Part::ImagePngB64("QUJD".to_string()),
            )],
            tools: vec![],
        };
        let value = serde_json::to_value(WireChatRequest::from_neutral(&req)).unwrap();
        let content = &value["messages"][0]["content"];
        assert!(
            content.is_array(),
            "expected a content-parts array, got: {content}"
        );
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], "what's in this screenshot?");
        assert_eq!(content[1]["type"], "image_url");
        assert_eq!(content[1]["image_url"]["url"], "data:image/png;base64,QUJD");
    }

    #[test]
    fn tool_calls_and_tool_results_serialize_with_the_function_wrapper() {
        let req = ChatRequest {
            model: "test-model".to_string(),
            messages: vec![
                Message::assistant_tool_calls(vec![ToolCall {
                    id: "call_1".to_string(),
                    name: "click".to_string(),
                    arguments: r#"{"ref":"e6"}"#.to_string(),
                }]),
                Message::tool_result("call_1", "ok"),
            ],
            tools: vec![ToolDef {
                name: "click".to_string(),
                description: "Clicks an element by ref".to_string(),
                parameters: serde_json::json!({"type": "object", "properties": {"ref": {"type": "string"}}}),
            }],
        };
        let value = serde_json::to_value(WireChatRequest::from_neutral(&req)).unwrap();

        let assistant_msg = &value["messages"][0];
        assert_eq!(assistant_msg["role"], "assistant");
        assert!(
            assistant_msg.get("content").is_none(),
            "no content when only tool_calls are present"
        );
        assert_eq!(assistant_msg["tool_calls"][0]["id"], "call_1");
        assert_eq!(assistant_msg["tool_calls"][0]["type"], "function");
        assert_eq!(assistant_msg["tool_calls"][0]["function"]["name"], "click");
        assert_eq!(
            assistant_msg["tool_calls"][0]["function"]["arguments"],
            r#"{"ref":"e6"}"#
        );

        let tool_msg = &value["messages"][1];
        assert_eq!(tool_msg["role"], "tool");
        assert_eq!(tool_msg["tool_call_id"], "call_1");
        assert_eq!(tool_msg["content"], "ok");

        assert_eq!(value["tools"][0]["type"], "function");
        assert_eq!(value["tools"][0]["function"]["name"], "click");
        assert_eq!(
            value["tools"][0]["function"]["parameters"]["type"],
            "object"
        );
    }

    #[test]
    fn response_with_text_deserializes_to_a_stop_message() {
        let raw = serde_json::json!({
            "choices": [{
                "message": {"role": "assistant", "content": "hello there"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 3, "total_tokens": 13}
        });
        let wire: WireChatResponse = serde_json::from_value(raw).unwrap();
        let resp = wire.into_neutral();

        assert_eq!(resp.finish_reason, FinishReason::Stop);
        assert_eq!(resp.message.text(), "hello there");
        assert!(resp.message.tool_calls.is_empty());
        let usage = resp.usage.expect("usage present");
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.total_tokens, 13);
    }

    #[test]
    fn response_with_tool_calls_deserializes_correctly() {
        let raw = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_9",
                        "function": {"name": "navigate", "arguments": "{\"url\":\"https://example.com\"}"}
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });
        let wire: WireChatResponse = serde_json::from_value(raw).unwrap();
        let resp = wire.into_neutral();

        assert_eq!(resp.finish_reason, FinishReason::ToolCalls);
        assert_eq!(resp.message.text(), "");
        assert_eq!(resp.message.tool_calls.len(), 1);
        assert_eq!(resp.message.tool_calls[0].id, "call_9");
        assert_eq!(resp.message.tool_calls[0].name, "navigate");
        assert_eq!(
            resp.message.tool_calls[0].arguments,
            r#"{"url":"https://example.com"}"#
        );
    }
}
