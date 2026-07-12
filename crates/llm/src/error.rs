use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("failed to read config file {path}: {source}")]
    ConfigRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse config file {path}: {source}")]
    ConfigParse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("unknown role {0:?} (not defined under [roles] in config)")]
    UnknownRole(String),

    #[error("role {role:?} references unknown provider {provider:?} (not defined under [providers] in config)")]
    UnknownProvider { role: String, provider: String },

    #[error("provider {provider:?} has no credential configured -- set api_key or api_key_env under [providers.{provider}]")]
    NoCredentialConfigured { provider: String },

    #[error("provider {provider:?}: environment variable {env_var:?} is not set")]
    MissingCredential { provider: String, env_var: String },

    #[error("provider kind {kind:?} is not implemented yet")]
    NotYetImplemented { kind: String },

    #[error("http request to {url} failed: {source}")]
    Http {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("provider returned HTTP {status} from {url}: {body}")]
    HttpStatus {
        url: String,
        status: u16,
        body: String,
    },

    #[error("failed to parse provider response from {url}: {source}")]
    ResponseParse {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("failed to read token store file {path}: {source}")]
    TokenStoreIo {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse stored tokens at {path}: {source}")]
    TokenStoreParse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("unknown OAuth flow {0:?}")]
    UnknownOAuthFlow(String),

    #[error("no stored login for provider {0:?} -- run `aib auth login {0}`")]
    NotLoggedIn(String),

    #[error("OAuth login for {provider:?} failed: {reason}")]
    OAuthLoginFailed { provider: String, reason: String },

    #[error(
        "OAuth callback state mismatch -- the login link may be stale; run `aib auth login` again"
    )]
    OAuthStateMismatch,

    #[error("refreshing the OAuth token for {provider:?} failed: {reason} -- run `aib auth login {provider}` again")]
    OAuthRefreshFailed { provider: String, reason: String },

    #[error("local OAuth callback server failed: {0}")]
    OAuthCallbackServer(String),

    #[error("SSE stream from {url} ended without a completed response event")]
    SseIncomplete { url: String },
}

pub type Result<T> = std::result::Result<T, LlmError>;
