//! Provider-agnostic LLM client (llm-providers spec). Wire-neutral chat
//! types (`types`), per-provider config loading and role resolution
//! (`config`), credential resolution (`auth`), and the OpenAI-compatible
//! chat-completions client (`client_compat`) that every provider this
//! project targets speaks. Never touches the browser -- `crates/agent`
//! layers the tool-calling loop on top of this.

mod auth;
mod client;
mod client_compat;
mod config;
mod error;
mod types;

pub use auth::CredentialSource;
pub use client::{Client, RoleClient};
pub use client_compat::CompatClient;
pub use config::{AgentSettings, Config, ProviderConfig, ProviderKind, RoleConfig, SkillsConfig};
pub use error::{LlmError, Result};
pub use types::{
    ChatRequest, ChatResponse, FinishReason, Message, Part, Role, ToolCall, ToolDef, Usage,
};
