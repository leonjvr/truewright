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
}

pub type Result<T> = std::result::Result<T, LlmError>;
