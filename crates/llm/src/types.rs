//! Wire-neutral chat types (llm-providers spec: "Provider-agnostic chat
//! types"). Deliberately carry no OpenAI-specific serde shape -- conversion
//! to/from a given provider's wire format lives in that provider's own
//! client module (e.g. `client_compat.rs`), so adding a second wire shape
//! (the Responses API, oauth-subscription-auth spec) never touches these.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Role {
    System,
    #[default]
    User,
    Assistant,
    Tool,
}

/// One piece of a message's content. A message can mix text and image parts
/// (e.g. a screenshot attached to a user message when the driver role has
/// vision -- agent-harness spec).
#[derive(Debug, Clone)]
pub enum Part {
    Text(String),
    ImagePngB64(String),
    ImageJpegB64(String),
}

/// A tool call the model asked for. `arguments` is the raw JSON string the
/// model produced -- not pre-parsed, since malformed argument JSON is a
/// recoverable error the agent loop feeds back to the model rather than a
/// client-level failure (agent-harness spec).
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Default)]
pub struct Message {
    pub role: Role,
    pub content: Vec<Part>,
    /// Only meaningful on an `Assistant` message.
    pub tool_calls: Vec<ToolCall>,
    /// Only meaningful on a `Tool` message -- which call this is the result of.
    pub tool_call_id: Option<String>,
}

impl Message {
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: vec![Part::Text(text.into())],
            ..Default::default()
        }
    }

    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: vec![Part::Text(text.into())],
            ..Default::default()
        }
    }

    pub fn user_with_image(text: impl Into<String>, image: Part) -> Self {
        Self {
            role: Role::User,
            content: vec![Part::Text(text.into()), image],
            ..Default::default()
        }
    }

    pub fn assistant_tool_calls(tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: Role::Assistant,
            content: vec![],
            tool_calls,
            tool_call_id: None,
        }
    }

    pub fn tool_result(tool_call_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: vec![Part::Text(text.into())],
            tool_calls: vec![],
            tool_call_id: Some(tool_call_id.into()),
        }
    }

    /// Concatenates every `Part::Text` in this message, ignoring image
    /// parts. Convenient for callers (progress logging, tests) that only
    /// care about the text.
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|p| match p {
                Part::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }
}

/// A tool the model may call. `parameters` is a JSON-schema object, same
/// shape every OpenAI-compatible provider expects.
#[derive(Debug, Clone)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinishReason {
    Stop,
    ToolCalls,
    Length,
    Other,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub message: Message,
    pub finish_reason: FinishReason,
    pub usage: Option<Usage>,
}
