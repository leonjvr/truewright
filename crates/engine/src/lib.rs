//! Session layer on top of `cdp`: one browser session an agent can
//! navigate, snapshot, and act on by ref. See
//! /openspec/changes/phase-1-agent-mvp/design.md for scope decisions.

mod error;
mod keys;
mod session;
mod snapshot;

pub use error::{EngineError, Result};
pub use session::Session;
