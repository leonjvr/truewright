//! Credential resolution for a provider (llm-providers spec: "Per-provider
//! API-key/env-var auth"; oauth-subscription-auth spec: "PKCE
//! authorization flow", "Token store", "Transparent refresh").
//! `CredentialSource::bearer()` is the one thing every client cares about;
//! how the token was obtained (a literal key, or a refreshed OAuth token)
//! stays behind this one async call, so `CompatClient`/`ResponsesClient`
//! never change when a new credential kind is added.

mod callback;
mod flows;
mod jwt;
mod login;
mod pkce;
mod store;

pub use callback::{accept_one as accept_callback, bind as bind_callback, CallbackResult};
pub use flows::{flow, OAuthFlowSpec, CHATGPT};
pub use login::{
    exchange_code_with_flow, login as oauth_login, login_with_flow, refresh as oauth_refresh,
    refresh_with_flow,
};
pub use pkce::Pkce;
pub use store::{StoredTokens, TokenStore};

use crate::error::{LlmError, Result};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Refresh this many seconds before actual expiry, so a request that's
/// mid-flight when a token is right on the boundary doesn't get an
/// almost-immediately-stale token.
const REFRESH_SKEW_SECS: u64 = 60;

#[derive(Clone)]
pub enum CredentialSource {
    /// A resolved, ready-to-use bearer token/API key -- either a literal
    /// `api_key` from config, or an `api_key_env` value already read from
    /// the environment at config-resolve time.
    Static(String),
    /// A provider authenticated via OAuth (e.g. a ChatGPT subscription).
    /// `provider` identifies the config's provider name for error
    /// messages only ("run `truewright auth login <provider>`" needs to name
    /// what the *user* configured, not the underlying flow id). Storage
    /// is keyed by `flow_id`, not `provider` -- `login_with_flow`/
    /// `refresh_with_flow` both save under `flow.id`, and a config could
    /// name its provider something else entirely while still pointing at
    /// the same flow (`oauth_flow = "chatgpt"` under `[providers.my-work-account]`,
    /// say), so `provider` and `flow_id` are NOT interchangeable as a
    /// store key -- confirmed as a real bug caught before shipping (see
    /// design.md), not a hypothetical.
    OAuth {
        provider: String,
        flow_id: String,
        store: Arc<TokenStore>,
    },
}

impl CredentialSource {
    /// The token to send as `Authorization: Bearer <token>`. Refreshes an
    /// OAuth credential transparently when it's near expiry (or already
    /// expired); a `Static` credential never fails or does any I/O here.
    #[allow(clippy::result_large_err)]
    pub async fn bearer(&self) -> Result<String> {
        match self {
            CredentialSource::Static(token) => Ok(token.clone()),
            CredentialSource::OAuth {
                provider,
                flow_id,
                store,
            } => {
                let mut tokens = store
                    .load(flow_id)?
                    .ok_or_else(|| LlmError::NotLoggedIn(provider.clone()))?;

                if seconds_until_expiry(tokens.expires_at_epoch_s) < REFRESH_SKEW_SECS {
                    let Some(refresh_token) = tokens.refresh_token.clone() else {
                        return Err(LlmError::NotLoggedIn(provider.clone()));
                    };
                    tokens = login::refresh(flow_id, &refresh_token).await?;
                    store.save(flow_id, &tokens)?;
                }

                Ok(tokens.access_token)
            }
        }
    }

    /// The ChatGPT account id to send as the `ChatGPT-Account-ID` header
    /// (oauth-subscription-auth spec: "ChatGPT-subscription backend").
    /// `None` for a `Static` credential, or an OAuth credential with no
    /// stored account id (e.g. the id_token had no claim for it).
    pub async fn account_id(&self) -> Option<String> {
        match self {
            CredentialSource::Static(_) => None,
            CredentialSource::OAuth { flow_id, store, .. } => store
                .load(flow_id)
                .ok()
                .flatten()
                .and_then(|t| t.account_id),
        }
    }
}

fn seconds_until_expiry(expires_at_epoch_s: u64) -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    expires_at_epoch_s.saturating_sub(now)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_until_expiry_never_underflows_for_an_already_expired_token() {
        assert_eq!(seconds_until_expiry(0), 0);
        assert_eq!(seconds_until_expiry(1), 0);
    }

    #[tokio::test]
    async fn static_credential_never_touches_the_filesystem_or_network() {
        let source = CredentialSource::Static("literal-token".to_string());
        assert_eq!(source.bearer().await.unwrap(), "literal-token");
        assert_eq!(source.account_id().await, None);
    }

    #[tokio::test]
    async fn oauth_credential_with_no_stored_login_fails_clearly() {
        let dir = std::env::temp_dir().join(format!(
            "truewright-llm-credsource-test-{}",
            std::process::id()
        ));
        let source = CredentialSource::OAuth {
            provider: "chatgpt".to_string(),
            flow_id: "chatgpt".to_string(),
            store: Arc::new(TokenStore::new(dir)),
        };
        let err = source.bearer().await.expect_err("no login stored yet");
        assert!(matches!(err, LlmError::NotLoggedIn(p) if p == "chatgpt"));
    }

    /// A real bug caught before shipping: `bearer()`/`account_id()` once
    /// loaded by `provider` while `login_with_flow`/`refresh_with_flow`
    /// save by `flow_id` -- silently invisible whenever a config names its
    /// provider the same as the flow id (the common case, and every other
    /// test here), so this test deliberately uses DIFFERENT names for the
    /// two to catch a regression the coincidentally-matching tests can't.
    #[tokio::test]
    async fn stored_tokens_are_found_when_provider_name_differs_from_flow_id() {
        let dir = std::env::temp_dir().join(format!(
            "truewright-llm-credsource-mismatch-test-{}",
            std::process::id()
        ));
        let store = Arc::new(TokenStore::new(dir.clone()));
        store
            .save(
                "chatgpt", // saved under the flow id...
                &StoredTokens {
                    access_token: "at-mismatch-test".to_string(),
                    refresh_token: None,
                    id_token: None,
                    expires_at_epoch_s: 4_000_000_000,
                    account_id: Some("acct-mismatch-test".to_string()),
                },
            )
            .expect("save succeeds");

        let source = CredentialSource::OAuth {
            provider: "my-work-chatgpt".to_string(), // ...but the provider is named differently
            flow_id: "chatgpt".to_string(),
            store,
        };

        assert_eq!(
            source.bearer().await.expect("token is found by flow_id"),
            "at-mismatch-test"
        );
        assert_eq!(
            source.account_id().await,
            Some("acct-mismatch-test".to_string())
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
