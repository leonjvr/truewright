//! OpenAI Responses-API client for ChatGPT-subscription usage
//! (oauth-subscription-auth spec: "ChatGPT-subscription backend"). A
//! genuinely different wire shape from `client_compat.rs` -- `input`
//! items instead of `messages`, flat (not `function`-wrapped) tool defs,
//! and an SSE-only backend that this client aggregates into the same
//! non-streaming `ChatResponse` every other client returns, so nothing
//! above this module needs to know the difference.
//!
//! Endpoint, header names, and the SSE-terminal-event assumption are
//! implemented from OpenAI's own public Responses API documentation and
//! (for the ChatGPT-specific backend/header bits) `openai/codex`'s own
//! source -- not independently live-verified against a real ChatGPT
//! subscription in this environment. See design.md's testing note.

use crate::auth::CredentialSource;
use crate::error::{LlmError, Result};
use crate::types::{
    ChatRequest, ChatResponse, FinishReason, Message, Part, Role, ToolCall, ToolDef, Usage,
};
use futures_util::StreamExt;
use serde::Deserialize;

/// Confirmed against `openai/codex`'s own source, not a third-party
/// writeup (see design.md).
pub const CHATGPT_CODEX_BASE_URL: &str = "https://chatgpt.com/backend-api/codex";
const RESPONSES_PATH: &str = "/responses";
/// aib's own identifier, not OpenAI's `codex_cli_rs` -- this project
/// doesn't misrepresent itself as the Codex CLI client (see design.md's
/// honesty-vs-compatibility discussion).
const ORIGINATOR: &str = "aib_agent_harness";

#[derive(Clone)]
pub struct ResponsesClient {
    http: reqwest::Client,
    base_url: String,
    credential: CredentialSource,
}

impl ResponsesClient {
    pub fn new(base_url: String, credential: CredentialSource) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url,
            credential,
        }
    }

    pub async fn complete(&self, req: &ChatRequest) -> Result<ChatResponse> {
        let url = format!("{}{RESPONSES_PATH}", self.base_url.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": req.model,
            "input": build_input(&req.messages),
            "tools": build_tools(&req.tools),
            "stream": true,
        });

        let bearer = self.credential.bearer().await?;
        let mut builder = self
            .http
            .post(&url)
            .bearer_auth(bearer)
            .header("originator", ORIGINATOR)
            .json(&body);
        if let Some(account_id) = self.credential.account_id().await {
            builder = builder.header("ChatGPT-Account-ID", account_id);
        }

        let resp = builder.send().await.map_err(|source| LlmError::Http {
            url: url.clone(),
            source,
        })?;
        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            return Err(LlmError::HttpStatus {
                url,
                status: status.as_u16(),
                body: body_text,
            });
        }

        aggregate_sse(resp, &url).await
    }
}

async fn aggregate_sse(resp: reqwest::Response, url: &str) -> Result<ChatResponse> {
    let mut stream = resp.bytes_stream();
    let mut buffer = String::new();
    let mut final_envelope: Option<ResponsesEnvelope> = None;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|source| LlmError::Http {
            url: url.to_string(),
            source,
        })?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(pos) = buffer.find("\n\n") {
            let block: String = buffer.drain(..pos + 2).collect();
            for line in block.lines() {
                let data = line.strip_prefix("data:").map(str::trim_start);
                let Some(data) = data else { continue };
                if let Ok(envelope) = serde_json::from_str::<ResponsesEnvelope>(data) {
                    final_envelope = Some(envelope);
                }
            }
        }
    }

    let envelope = final_envelope.ok_or_else(|| LlmError::SseIncomplete {
        url: url.to_string(),
    })?;
    Ok(envelope.into_neutral())
}

// ---- wire format (private) ----

fn role_str(role: Role) -> &'static str {
    match role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Tool => "tool",
    }
}

fn content_parts(parts: &[Part]) -> Vec<serde_json::Value> {
    parts
        .iter()
        .map(|p| match p {
            Part::Text(t) => serde_json::json!({"type": "input_text", "text": t}),
            Part::ImagePngB64(b64) => serde_json::json!({
                "type": "input_image",
                "image_url": format!("data:image/png;base64,{b64}"),
            }),
            Part::ImageJpegB64(b64) => serde_json::json!({
                "type": "input_image",
                "image_url": format!("data:image/jpeg;base64,{b64}"),
            }),
        })
        .collect()
}

/// Unlike chat-completions' one-item-per-message shape, an assistant
/// message carrying multiple tool calls expands to one `function_call`
/// input item per call -- so this maps each `Message` to zero-or-more
/// input items, not exactly one.
fn build_input(messages: &[Message]) -> Vec<serde_json::Value> {
    messages
        .iter()
        .flat_map(|m| -> Vec<serde_json::Value> {
            match m.role {
                Role::System | Role::User => {
                    vec![serde_json::json!({"role": role_str(m.role), "content": content_parts(&m.content)})]
                }
                Role::Assistant if !m.tool_calls.is_empty() => m
                    .tool_calls
                    .iter()
                    .map(|tc| {
                        serde_json::json!({
                            "type": "function_call",
                            "call_id": tc.id,
                            "name": tc.name,
                            "arguments": tc.arguments,
                        })
                    })
                    .collect(),
                Role::Assistant => {
                    let content: Vec<_> = m
                        .content
                        .iter()
                        .filter_map(|p| match p {
                            Part::Text(t) => Some(serde_json::json!({"type": "output_text", "text": t})),
                            _ => None,
                        })
                        .collect();
                    vec![serde_json::json!({"type": "message", "role": "assistant", "content": content})]
                }
                Role::Tool => vec![serde_json::json!({
                    "type": "function_call_output",
                    "call_id": m.tool_call_id.clone().unwrap_or_default(),
                    "output": m.text(),
                })],
            }
        })
        .collect()
}

fn build_tools(tools: &[ToolDef]) -> Vec<serde_json::Value> {
    tools
        .iter()
        .map(|t| {
            serde_json::json!({
                "type": "function",
                "name": t.name,
                "description": t.description,
                "parameters": t.parameters,
            })
        })
        .collect()
}

#[derive(Deserialize)]
struct ResponsesEnvelope {
    response: ResponsesBody,
}

#[derive(Deserialize)]
struct ResponsesBody {
    #[serde(default)]
    output: Vec<OutputItem>,
    #[serde(default)]
    usage: Option<ResponsesUsage>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OutputItem {
    Message {
        #[serde(default)]
        content: Vec<OutputContentPart>,
    },
    FunctionCall {
        call_id: String,
        name: String,
        arguments: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OutputContentPart {
    OutputText {
        text: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct ResponsesUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    total_tokens: u64,
}

impl ResponsesEnvelope {
    fn into_neutral(self) -> ChatResponse {
        let mut text = String::new();
        let mut tool_calls = Vec::new();
        for item in self.response.output {
            match item {
                OutputItem::Message { content } => {
                    for part in content {
                        if let OutputContentPart::OutputText { text: t } = part {
                            text.push_str(&t);
                        }
                    }
                }
                OutputItem::FunctionCall {
                    call_id,
                    name,
                    arguments,
                } => {
                    tool_calls.push(ToolCall {
                        id: call_id,
                        name,
                        arguments,
                    });
                }
                OutputItem::Other => {}
            }
        }
        let finish_reason = if !tool_calls.is_empty() {
            FinishReason::ToolCalls
        } else {
            FinishReason::Stop
        };
        let message = Message {
            role: Role::Assistant,
            content: if text.is_empty() {
                vec![]
            } else {
                vec![Part::Text(text)]
            },
            tool_calls,
            tool_call_id: None,
        };
        let usage = self.response.usage.map(|u| Usage {
            prompt_tokens: u.input_tokens,
            completion_tokens: u.output_tokens,
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
    use crate::types::ChatRequest;

    #[test]
    fn text_message_becomes_a_role_plus_content_input_item() {
        let req = ChatRequest {
            model: "test".to_string(),
            messages: vec![Message::system("be helpful"), Message::user("hi")],
            tools: vec![],
        };
        let input = build_input(&req.messages);
        assert_eq!(input[0]["role"], "system");
        assert_eq!(input[0]["content"][0]["type"], "input_text");
        assert_eq!(input[0]["content"][0]["text"], "be helpful");
        assert_eq!(input[1]["role"], "user");
    }

    #[test]
    fn assistant_tool_calls_expand_to_one_function_call_item_each() {
        let msg = Message::assistant_tool_calls(vec![
            ToolCall {
                id: "c1".to_string(),
                name: "click".to_string(),
                arguments: "{}".to_string(),
            },
            ToolCall {
                id: "c2".to_string(),
                name: "snapshot".to_string(),
                arguments: "{}".to_string(),
            },
        ]);
        let input = build_input(std::slice::from_ref(&msg));
        assert_eq!(input.len(), 2);
        assert_eq!(input[0]["type"], "function_call");
        assert_eq!(input[0]["call_id"], "c1");
        assert_eq!(input[1]["call_id"], "c2");
    }

    #[test]
    fn tool_result_becomes_a_function_call_output_item() {
        let msg = Message::tool_result("c1", "clicked ok");
        let input = build_input(std::slice::from_ref(&msg));
        assert_eq!(input[0]["type"], "function_call_output");
        assert_eq!(input[0]["call_id"], "c1");
        assert_eq!(input[0]["output"], "clicked ok");
    }

    #[test]
    fn tools_serialize_flat_not_function_wrapped() {
        let tools = vec![ToolDef {
            name: "click".to_string(),
            description: "Clicks an element".to_string(),
            parameters: serde_json::json!({"type": "object"}),
        }];
        let wire = build_tools(&tools);
        assert_eq!(wire[0]["type"], "function");
        assert_eq!(wire[0]["name"], "click");
        assert!(
            wire[0].get("function").is_none(),
            "Responses API tools are flat, not function-wrapped"
        );
    }

    #[test]
    fn sse_terminal_envelope_parses_text_and_tool_calls() {
        let raw = serde_json::json!({
            "response": {
                "output": [
                    {"type": "message", "content": [{"type": "output_text", "text": "here you go"}]},
                    {"type": "function_call", "call_id": "call_1", "name": "click", "arguments": "{\"ref\":\"e6\"}"}
                ],
                "usage": {"input_tokens": 5, "output_tokens": 2, "total_tokens": 7}
            }
        });
        let envelope: ResponsesEnvelope = serde_json::from_value(raw).unwrap();
        let resp = envelope.into_neutral();

        assert_eq!(resp.message.text(), "here you go");
        assert_eq!(resp.finish_reason, FinishReason::ToolCalls);
        assert_eq!(resp.message.tool_calls.len(), 1);
        assert_eq!(resp.message.tool_calls[0].name, "click");
        assert_eq!(resp.usage.unwrap().total_tokens, 7);
    }
}
