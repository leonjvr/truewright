//! On-disk persistence for trained personas (human-motion spec: "Persona
//! fitted from captured samples" persists by name; "Untrained profile fails
//! clearly" reads it back). Reuses `cdp::launch::profile_base_dir()`'s
//! platform-appropriate data directory, same family as `aib/profiles/<name>`
//! (browser user-data) and `aib/recordings/<id>` (screencasts).

use super::Persona;
use crate::error::{EngineError, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize)]
pub struct StoredProfile {
    pub name: String,
    pub persona: Persona,
    pub movements_captured: usize,
    pub keystrokes_captured: usize,
    pub captured_at_unix_ms: u128,
}

// EngineError is kept as one flat enum (matches cdp::CdpError's rationale);
// see the identical allow in cdp/src/launch.rs.
#[allow(clippy::result_large_err)]
fn profiles_dir() -> Result<PathBuf> {
    Ok(cdp::launch::profile_base_dir()?
        .join("aib")
        .join("profiles")
        .join("human"))
}

#[allow(clippy::result_large_err)]
fn profile_path(name: &str) -> Result<PathBuf> {
    Ok(profiles_dir()?.join(format!("{name}.json")))
}

// EngineError is kept as one flat enum (matches cdp::CdpError's rationale);
// see the identical allow in cdp/src/launch.rs.
#[allow(clippy::result_large_err)]
pub fn save(
    name: &str,
    persona: Persona,
    movements_captured: usize,
    keystrokes_captured: usize,
) -> Result<StoredProfile> {
    let dir = profiles_dir()?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| EngineError::Training(format!("failed to create profiles dir: {e}")))?;

    let stored = StoredProfile {
        name: name.to_string(),
        persona,
        movements_captured,
        keystrokes_captured,
        captured_at_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    };

    let path = profile_path(name)?;
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&stored)
            .map_err(|e| EngineError::Training(format!("failed to serialize profile: {e}")))?,
    )
    .map_err(|e| EngineError::Training(format!("failed to write {}: {e}", path.display())))?;

    Ok(stored)
}

/// Loads a trained profile by name, or a typed "not trained" error if it
/// doesn't exist (human-motion spec: "Untrained profile fails clearly" —
/// never a silent fallback to a synthetic persona).
#[allow(clippy::result_large_err)]
pub fn load(name: &str) -> Result<Persona> {
    let path = profile_path(name)?;
    let bytes = std::fs::read(&path).map_err(|_| EngineError::UntrainedProfile(name.to_string()))?;
    let stored: StoredProfile = serde_json::from_slice(&bytes)
        .map_err(|e| EngineError::Training(format!("failed to parse profile {name:?}: {e}")))?;
    Ok(stored.persona)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loading_an_unsaved_profile_is_untrained_error() {
        let err = load("definitely-never-trained-xyz").unwrap_err();
        assert!(matches!(err, EngineError::UntrainedProfile(name) if name == "definitely-never-trained-xyz"));
    }
}
