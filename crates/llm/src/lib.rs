//! Provider-agnostic LLM client (llm-providers spec). Wire-neutral chat
//! types (`types`), per-provider config loading and role resolution
//! (`config`), credential resolution including OAuth (`auth`,
//! oauth-subscription-auth spec), the OpenAI-compatible chat-completions
//! client (`client_compat`) most providers speak, and the OpenAI Responses
//! client (`client_responses`) for ChatGPT-subscription usage. Never
//! touches the browser -- `crates/agent` layers the tool-calling loop on
//! top of this.

mod auth;
mod client;
mod client_compat;
mod client_responses;
mod config;
mod error;
mod types;

pub use auth::{
    accept_callback, bind_callback, exchange_code_with_flow, flow as oauth_flow, login_with_flow,
    oauth_login, oauth_refresh, refresh_with_flow, CallbackResult, CredentialSource, OAuthFlowSpec,
    Pkce, StoredTokens, TokenStore, CHATGPT,
};
pub use client::{Client, RoleClient};
pub use client_compat::CompatClient;
pub use client_responses::{ResponsesClient, CHATGPT_CODEX_BASE_URL};
pub use config::{AgentSettings, Config, ProviderConfig, ProviderKind, RoleConfig, SkillsConfig};
pub use error::{LlmError, Result};
pub use types::{
    ChatRequest, ChatResponse, FinishReason, Message, Part, Role, ToolCall, ToolDef, Usage,
};
