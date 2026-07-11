//! The resolved client for a role (llm-providers spec). An enum, not a
//! trait object: there are exactly two wire shapes in this project's scope
//! (OpenAI-compatible chat completions, and the OpenAI Responses shape for
//! ChatGPT-subscription usage added in oauth-subscription-auth), resolved
//! once at config-load time -- no dynamic registry, no `Box<dyn>` plus the
//! async-fn-in-trait friction that would bring.

use crate::client_compat::CompatClient;
use crate::error::{LlmError, Result};
use crate::types::{ChatRequest, ChatResponse};

pub enum Client {
    Compat(CompatClient),
}

impl Client {
    pub async fn complete(&self, req: &ChatRequest) -> Result<ChatResponse> {
        match self {
            Client::Compat(c) => c.complete(req).await,
        }
    }
}

/// A role resolved to a concrete client + the model name to send
/// (agent-harness spec: driver/vision roles). `vision` mirrors the role's
/// config flag and drives the agent loop's screenshot-routing decision.
pub struct RoleClient {
    pub client: Client,
    pub model: String,
    pub vision: bool,
}

impl RoleClient {
    pub async fn complete(&self, mut req: ChatRequest) -> Result<ChatResponse> {
        req.model = self.model.clone();
        self.client.complete(&req).await
    }
}

/// Placeholder error helper for a provider kind not implemented yet
/// (`openai-responses`, until oauth-subscription-auth ships it) -- kept
/// here rather than inline in `config.rs` so both modules construct the
/// same error consistently.
pub(crate) fn not_yet_implemented(kind: &str) -> LlmError {
    LlmError::NotYetImplemented {
        kind: kind.to_string(),
    }
}
