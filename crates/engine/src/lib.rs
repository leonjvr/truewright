//! Session layer on top of `cdp`: one browser session an agent can
//! navigate, snapshot, act on by ref, and record. See
//! /openspec/changes/archive/*-phase-1-agent-mvp/design.md and
//! /openspec/changes/archive/*-screencast-capture/design.md for scope
//! decisions.

mod error;
mod keys;
pub mod recording;
mod session;
mod snapshot;

pub use error::{EngineError, Result};
pub use recording::{Recording, RecordingOptions, RecordingOutput};
pub use session::Session;
