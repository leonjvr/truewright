//! The agent harness (agent-harness spec): the tool-calling loop that
//! lets a configured LLM role (`llm::RoleClient`, from `llm-providers`/
//! `oauth-subscription-auth`) drive an `engine::Session` to complete a
//! browser task autonomously. `truewright agent "task"` (the CLI) and
//! `browser_run_task` (the MCP tool, `mcp-task-delegation`) are both thin
//! wrappers over `Harness::run_task`.

mod error;
mod harness;
mod prompt;
mod skills;
mod tools;
mod types;
mod vision;

pub use error::{AgentError, Result};
pub use harness::Harness;
pub use skills::{default_search_dirs as default_skill_dirs, resolve as resolve_skills, Skill};
pub use tools::tool_defs;
pub use types::{AgentEvent, SharedSession, TaskOutcome};
