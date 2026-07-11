//! Network recording and replay (network-mocking spec). Record real
//! request/response pairs to a named cassette; replay a session entirely
//! from a cassette with no live-network dependency.

pub mod cassette;
mod recording;
mod replay;

pub use cassette::Cassette;
pub use recording::{NetworkRecording, NetworkRecordingSummary};
pub use replay::NetworkReplay;
