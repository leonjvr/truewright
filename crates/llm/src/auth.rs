//! Credential resolution for a provider (llm-providers spec: "Per-provider
//! API-key/env-var auth"). `CredentialSource::bearer()` is the one thing
//! every client cares about -- how a credential is obtained (a literal key,
//! an env var read once, or eventually a refreshed OAuth token,
//! oauth-subscription-auth spec) stays behind this one async call, so
//! `CompatClient` never needs to change when a new credential kind is
//! added.

use crate::error::Result;

#[derive(Debug, Clone)]
pub enum CredentialSource {
    /// A resolved, ready-to-use bearer token/API key -- either a literal
    /// `api_key` from config, or an `api_key_env` value already read from
    /// the environment at config-resolve time.
    Static(String),
}

impl CredentialSource {
    /// The token to send as `Authorization: Bearer <token>`. Fallible and
    /// async because a future credential kind (OAuth) may need to refresh
    /// over the network first; `Static` never fails.
    pub async fn bearer(&self) -> Result<String> {
        match self {
            CredentialSource::Static(token) => Ok(token.clone()),
        }
    }
}
