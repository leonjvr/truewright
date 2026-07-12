#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("LLM request failed: {0}")]
    Llm(#[from] llm::LlmError),

    #[error("browser session is not available")]
    NoSession,

    #[error("task exceeded the step budget ({0} steps)")]
    MaxStepsExceeded(u32),

    #[error("task exceeded its time budget")]
    TaskTimeout,

    #[error("the model produced no tool call and no termination signal after repeated nudges")]
    NoProgress,

    #[error("unknown tool call: {0:?}")]
    UnknownTool(String),

    #[error("no vision role configured -- add [roles.vision] to config.toml to use screenshots with a non-vision driver")]
    NoVisionRole,

    #[error("skill {0:?} not found in any skills directory")]
    UnknownSkill(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, AgentError>;
