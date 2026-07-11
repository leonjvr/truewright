//! Session layer on top of `cdp`: one browser session an agent can
//! navigate, snapshot, act on by ref, record, and (optionally) drive
//! human-like. See /openspec/changes/archive/*-phase-1-agent-mvp/design.md,
//! /openspec/changes/archive/*-screencast-capture/design.md, and
//! /openspec/changes/archive/*-human-motion-synthetic/design.md for scope
//! decisions.

mod error;
mod keys;
pub mod motion;
pub mod recording;
mod session;
mod snapshot;

pub use error::{EngineError, Result};
pub use motion::profile_store::StoredProfile;
pub use motion::{Persona, Training};
pub use recording::{Recording, RecordingOptions, RecordingOutput};
pub use session::{HumanLike, Session};
