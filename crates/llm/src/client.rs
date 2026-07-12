//! The resolved client for a role (llm-providers spec). An enum, not a
//! trait object: there are exactly two wire shapes in this project's scope
//! (OpenAI-compatible chat completions, and the OpenAI Responses shape for
//! ChatGPT-subscription usage, oauth-subscription-auth spec), resolved
//! once at config-load time -- no dynamic registry, no `Box<dyn>` plus the
//! async-fn-in-trait friction that would bring.

use crate::client_compat::CompatClient;
use crate::client_responses::ResponsesClient;
use crate::error::Result;
use crate::types::{ChatRequest, ChatResponse};

pub enum Client {
    Compat(CompatClient),
    Responses(ResponsesClient),
}

impl Client {
    pub async fn complete(&self, req: &ChatRequest) -> Result<ChatResponse> {
        match self {
            Client::Compat(c) => c.complete(req).await,
            Client::Responses(c) => c.complete(req).await,
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
