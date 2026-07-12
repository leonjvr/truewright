//! Shared session handle and progress/outcome types (agent-harness spec).

use engine::Session;
use std::sync::Arc;
use tokio::sync::Mutex;

/// The exact shape `AibTools` (crates/mcp) already stores its own session
/// in -- so the CLI (which owns its session outright) and the MCP tool
/// (which shares the session with every other `browser_*` tool) drive the
/// same harness code, not two implementations.
#[derive(Clone)]
pub struct SharedSession(pub Arc<Mutex<Option<Session>>>);

impl SharedSession {
    pub fn new(session: Session) -> Self {
        Self(Arc::new(Mutex::new(Some(session))))
    }

    pub fn from_arc(inner: Arc<Mutex<Option<Session>>>) -> Self {
        Self(inner)
    }
}

/// One event per model turn / tool call, for CLI progress output and MCP
/// transcript accumulation.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    Step {
        n: u32,
        max: u32,
    },
    ToolCall {
        name: String,
        args_summary: String,
    },
    ToolResult {
        name: String,
        ok: bool,
        summary: String,
    },
    Vision {
        chars: usize,
    },
    Done {
        passed: bool,
        summary: String,
    },
}

#[derive(Debug, Clone)]
pub struct TaskOutcome {
    pub passed: bool,
    pub summary: String,
    pub steps_used: u32,
}
