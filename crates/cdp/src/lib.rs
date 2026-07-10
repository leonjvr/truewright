//! Minimal Chrome DevTools Protocol client. Hand-written, not codegen'd —
//! see /openspec/changes/phase-0-cdp-spike/design.md for the rationale.

pub mod connection;
pub mod error;
pub mod launch;
pub mod ops;
pub mod protocol;
pub mod session;
pub mod transport;

pub use connection::Connection;
pub use error::{CdpError, Result};
pub use session::Session;
