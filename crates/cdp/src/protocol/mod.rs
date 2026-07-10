pub mod browser;
pub mod page;
pub mod runtime;
pub mod target;

use serde::{Deserialize, Serialize};

/// Params for commands that take no arguments (serializes to `{}`).
#[derive(Debug, Default, Serialize)]
pub struct EmptyParams {}

/// Response for commands that return `{}`.
#[derive(Debug, Deserialize)]
pub struct EmptyResponse {}
