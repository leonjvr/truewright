use std::time::Duration;

/// Errors an MCP tool call needs to translate for an agent — deliberately
/// distinct from `cdp::CdpError` so callers get "why this action failed"
/// rather than a raw protocol error.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("ref {0:?} does not resolve to an attached element (stale or from a previous page)")]
    StaleRef(String),

    #[error(
        "ref {r#ref:?} did not become actionable within {timeout:?} (last seen visible: {last_visible})"
    )]
    ActionTimeout {
        r#ref: String,
        timeout: Duration,
        last_visible: bool,
    },

    #[error("wait_for({text:?}) timed out after {timeout:?}")]
    WaitTimeout { text: String, timeout: Duration },

    #[error("unknown key: {0:?}")]
    UnknownKey(String),

    #[error("unknown persona: {0:?} (expected one of: careful, average, fast)")]
    UnknownPersona(String),

    #[error(transparent)]
    Cdp(#[from] cdp::CdpError),

    #[error("failed to parse walker/resolve response: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("recording failed: {0}")]
    Recording(String),
}

pub type Result<T> = std::result::Result<T, EngineError>;
