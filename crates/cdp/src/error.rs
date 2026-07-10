/// Error taxonomy for the CDP client. Every layer above `cdp` maps its own
/// errors from this enum (see design.md "Decisions" #5) — changing these
/// variants later is expensive, so keep it minimal and stable.
#[derive(Debug, thiserror::Error)]
pub enum CdpError {
    #[error("CDP protocol error {code}: {message}")]
    Protocol { code: i64, message: String },

    #[error("command timed out after {0:?}")]
    Timeout(std::time::Duration),

    #[error("connection disconnected")]
    Disconnected,

    #[error("failed to launch browser: {0}")]
    LaunchFailed(String),

    #[error("timed out attaching to browser devtools endpoint")]
    AttachTimeout,

    #[error("no installed Chromium browser found (checked: {checked})")]
    NoBrowserFound { checked: String },

    #[error("websocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, CdpError>;

/// Signaled to an event subscriber when it fell behind and the broadcast
/// channel dropped events on its behalf (see cdp-client spec: "Bounded event
/// subscription").
#[derive(Debug, Clone, Copy)]
pub struct LagInfo {
    pub skipped: u64,
}
