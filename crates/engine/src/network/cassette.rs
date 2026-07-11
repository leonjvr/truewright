//! On-disk cassette format (network-mocking spec: "Passive network
//! recording to a named cassette"). One JSON file per cassette -- network
//! responses are far fewer and smaller than screencast frames captured over
//! the same window, so a single array is simpler than a manifest/asset-dir
//! split.

use crate::error::{EngineError, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CassetteEntry {
    pub method: String,
    pub url: String,
    pub status: i64,
    pub headers: Vec<(String, String)>,
    /// Base64-encoded response body, regardless of content type -- avoids
    /// needing to detect/handle text vs. binary specially (design.md
    /// Decision #4).
    pub body_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Cassette {
    pub entries: Vec<CassetteEntry>,
}

// EngineError is kept as one flat enum (matches cdp::CdpError's rationale);
// see the identical allow in cdp/src/launch.rs.
#[allow(clippy::result_large_err)]
fn cassettes_dir() -> Result<PathBuf> {
    Ok(cdp::launch::profile_base_dir()?.join("aib").join("network"))
}

#[allow(clippy::result_large_err)]
fn cassette_path(name: &str) -> Result<PathBuf> {
    Ok(cassettes_dir()?.join(format!("{name}.json")))
}

// EngineError is kept as one flat enum (matches cdp::CdpError's rationale);
// see the identical allow in cdp/src/launch.rs.
#[allow(clippy::result_large_err)]
pub fn save(name: &str, cassette: &Cassette) -> Result<PathBuf> {
    let dir = cassettes_dir()?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| EngineError::Network(format!("failed to create cassettes dir: {e}")))?;

    let path = cassette_path(name)?;
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(cassette)
            .map_err(|e| EngineError::Network(format!("failed to serialize cassette: {e}")))?,
    )
    .map_err(|e| EngineError::Network(format!("failed to write {}: {e}", path.display())))?;

    Ok(path)
}

/// Loads a cassette by name, or a typed error if it doesn't exist -- never
/// a silent empty-cassette fallback (mirrors `profile_store::load`'s
/// "untrained profile fails clearly" precedent).
#[allow(clippy::result_large_err)]
pub fn load(name: &str) -> Result<Cassette> {
    let path = cassette_path(name)?;
    let bytes =
        std::fs::read(&path).map_err(|_| EngineError::UnknownCassette(name.to_string()))?;
    serde_json::from_slice(&bytes)
        .map_err(|e| EngineError::Network(format!("failed to parse cassette {name:?}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loading_an_unsaved_cassette_is_a_typed_error() {
        let err = load("definitely-never-recorded-xyz").unwrap_err();
        assert!(matches!(err, EngineError::UnknownCassette(name) if name == "definitely-never-recorded-xyz"));
    }
}
