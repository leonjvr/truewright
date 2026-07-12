//! Orchestrates one full OAuth login (oauth-subscription-auth spec:
//! "PKCE authorization flow"), and the token exchange/refresh HTTP calls
//! `CredentialSource::OAuth` and `aib auth login` both need.

use super::callback;
use super::flows::{self, OAuthFlowSpec};
use super::jwt;
use super::pkce::{self, Pkce};
use super::store::{StoredTokens, TokenStore};
use crate::error::{LlmError, Result};
use serde::Deserialize;
use std::time::Duration;

const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);
/// Fallback default if the id_token has no readable `exp` claim -- the
/// token endpoint's own response doesn't carry `expires_in` (confirmed
/// against OpenAI's Codex CLI source, not assumed), so this only ever
/// applies to a malformed/unexpected token shape, not the normal path.
const FALLBACK_EXPIRY_SECS: u64 = 3600;

/// Runs one full login by flow id (looked up in the static registry --
/// `flows::flow`). Real callers use this; `login_with_flow` below is the
/// actual implementation, parameterized on an explicit `OAuthFlowSpec` so
/// it's independently testable against a local stub server and usable
/// with a config-overridden/future custom flow that isn't in the static
/// registry.
pub async fn login(flow_id: &str, store: &TokenStore) -> Result<StoredTokens> {
    let flow =
        flows::flow(flow_id).ok_or_else(|| LlmError::UnknownOAuthFlow(flow_id.to_string()))?;
    login_with_flow(flow, store).await
}

/// Binds the local callback listener, prints (and best-effort opens) the
/// authorize URL, waits for the redirect, exchanges the code, and
/// persists the result under `flow.id`. Returns the freshly stored
/// tokens.
pub async fn login_with_flow(flow: &OAuthFlowSpec, store: &TokenStore) -> Result<StoredTokens> {
    let (listener, port) = bind_with_fallback(flow).await?;
    let pkce = Pkce::generate();
    let state = pkce::random_state();
    let redirect_uri = format!("http://localhost:{port}{}", flow.redirect_path);
    let authorize_url = build_authorize_url(flow, &pkce, &state, &redirect_uri);

    println!("Open this URL to sign in ({}):", flow.id);
    println!("{authorize_url}");
    let _ = open_in_browser(&authorize_url);

    let callback::CallbackResult {
        code,
        state: got_state,
    } = match callback::accept_one(listener, flow.redirect_path, LOGIN_TIMEOUT).await {
        Ok(cb) => cb,
        Err(LlmError::OAuthLoginFailed { reason, .. }) => {
            return Err(LlmError::OAuthLoginFailed {
                provider: flow.id.to_string(),
                reason,
            });
        }
        Err(e) => return Err(e),
    };
    if got_state != state {
        return Err(LlmError::OAuthStateMismatch);
    }

    let tokens = exchange_code_with_flow(flow, &code, &pkce.verifier, &redirect_uri).await?;
    store.save(flow.id, &tokens)?;
    Ok(tokens)
}

async fn bind_with_fallback(flow: &OAuthFlowSpec) -> Result<(tokio::net::TcpListener, u16)> {
    match callback::bind(flow.redirect_port).await {
        Ok(listener) => Ok((listener, flow.redirect_port)),
        Err(_) => {
            let listener = callback::bind(flow.redirect_port_fallback).await?;
            Ok((listener, flow.redirect_port_fallback))
        }
    }
}

fn build_authorize_url(
    flow: &OAuthFlowSpec,
    pkce: &Pkce,
    state: &str,
    redirect_uri: &str,
) -> String {
    let mut url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&code_challenge={}&code_challenge_method=S256&state={}",
        flow.authorize_url,
        urlencode(flow.client_id),
        urlencode(redirect_uri),
        urlencode(flow.scope),
        urlencode(&pkce.challenge),
        urlencode(state),
    );
    for (key, value) in flow.extra_authorize_params {
        url.push('&');
        url.push_str(key);
        url.push('=');
        url.push_str(&urlencode(value));
    }
    url
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Best-effort: prints the URL either way, so a headless/browserless
/// environment (over SSH, in a container) still lets the user copy it
/// manually -- opening the browser is a convenience, not a requirement.
fn open_in_browser(url: &str) -> std::io::Result<std::process::Child> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn()
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open").arg(url).spawn()
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
}

/// The token-exchange HTTP call, parameterized on an explicit flow so it's
/// independently testable against a local stub. `login_with_flow` is the
/// only real caller.
pub async fn exchange_code_with_flow(
    flow: &OAuthFlowSpec,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> Result<StoredTokens> {
    let http = reqwest::Client::new();
    let params = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", flow.client_id),
        ("code_verifier", verifier),
    ];
    // Codex's own initial exchange is form-encoded, but its refresh call
    // (below) is JSON -- a real asymmetry, not a mistake.
    debug_assert!(
        flow.token_exchange_is_form_encoded,
        "chatgpt flow's exchange is form-encoded"
    );
    let resp = http
        .post(flow.token_url)
        .form(&params)
        .send()
        .await
        .map_err(|source| LlmError::Http {
            url: flow.token_url.to_string(),
            source,
        })?;
    tokens_from_response(flow, resp).await
}

pub async fn refresh(flow_id: &str, refresh_token: &str) -> Result<StoredTokens> {
    let flow =
        flows::flow(flow_id).ok_or_else(|| LlmError::UnknownOAuthFlow(flow_id.to_string()))?;
    refresh_with_flow(flow, refresh_token)
        .await
        .map_err(|e| LlmError::OAuthRefreshFailed {
            provider: flow.id.to_string(),
            reason: e.to_string(),
        })
}

/// The refresh HTTP call, parameterized on an explicit flow so it's
/// independently testable against a local stub.
pub async fn refresh_with_flow(flow: &OAuthFlowSpec, refresh_token: &str) -> Result<StoredTokens> {
    let http = reqwest::Client::new();
    let body = serde_json::json!({
        "client_id": flow.client_id,
        "grant_type": "refresh_token",
        "refresh_token": refresh_token,
    });
    let resp = http
        .post(flow.token_url)
        .json(&body)
        .send()
        .await
        .map_err(|source| LlmError::Http {
            url: flow.token_url.to_string(),
            source,
        })?;
    tokens_from_response(flow, resp).await
}

async fn tokens_from_response(
    flow: &OAuthFlowSpec,
    resp: reqwest::Response,
) -> Result<StoredTokens> {
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(LlmError::OAuthLoginFailed {
            provider: flow.id.to_string(),
            reason: format!("token endpoint returned HTTP {status}: {body}"),
        });
    }
    let parsed: TokenResponse = resp
        .json()
        .await
        .map_err(|source| LlmError::ResponseParse {
            url: flow.token_url.to_string(),
            source,
        })?;
    let access_token = parsed
        .access_token
        .ok_or_else(|| LlmError::OAuthLoginFailed {
            provider: flow.id.to_string(),
            reason: "token response had no access_token".to_string(),
        })?;

    let (account_id, expires_at_epoch_s) = parsed
        .id_token
        .as_deref()
        .and_then(jwt::decode_payload)
        .map(|claims| {
            let account_id = claims
                .get("https://api.openai.com/auth")
                .and_then(|auth| auth.get("chatgpt_account_id"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let expires_at = claims.get("exp").and_then(|v| v.as_u64());
            (account_id, expires_at)
        })
        .unwrap_or((None, None));

    let expires_at_epoch_s = expires_at_epoch_s.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() + FALLBACK_EXPIRY_SECS)
            .unwrap_or(FALLBACK_EXPIRY_SECS)
    });

    Ok(StoredTokens {
        access_token,
        refresh_token: parsed.refresh_token,
        id_token: parsed.id_token,
        expires_at_epoch_s,
        account_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencode_leaves_unreserved_characters_alone_and_escapes_the_rest() {
        assert_eq!(urlencode("abc-._~123"), "abc-._~123");
        assert_eq!(urlencode("a b"), "a%20b");
        assert_eq!(urlencode("a=b&c"), "a%3Db%26c");
    }

    #[test]
    fn authorize_url_includes_pkce_and_flow_specific_params() {
        let flow = &flows::CHATGPT;
        let pkce = Pkce::generate();
        let url = build_authorize_url(
            flow,
            &pkce,
            "test-state",
            "http://localhost:1455/auth/callback",
        );
        assert!(url.starts_with(flow.authorize_url));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains(&format!("code_challenge={}", urlencode(&pkce.challenge))));
        assert!(url.contains("state=test-state"));
        assert!(url.contains("codex_cli_simplified_flow=true"));
        assert!(url.contains(&urlencode(flow.client_id)));
    }
}
