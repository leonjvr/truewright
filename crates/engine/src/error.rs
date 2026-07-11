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

    #[error("no trained profile named {0:?}; run browser_train_start/browser_train_stop against it first")]
    UntrainedProfile(String),

    #[error("specify either persona or trained_profile, not both")]
    AmbiguousPersona,

    #[error("training failed: {0}")]
    Training(String),

    #[error(transparent)]
    Cdp(#[from] cdp::CdpError),

    #[error("failed to parse walker/resolve response: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("recording failed: {0}")]
    Recording(String),

    #[error("network mocking failed: {0}")]
    Network(String),

    #[error("no cassette named {0:?}; run browser_network_record_start/browser_network_record_stop against it first")]
    UnknownCassette(String),

    #[error("{0}")]
    Clock(String),
}

pub type Result<T> = std::result::Result<T, EngineError>;
