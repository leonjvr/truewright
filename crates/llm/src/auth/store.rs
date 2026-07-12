//! Persisted OAuth tokens (oauth-subscription-auth spec: "Token store").
//! One JSON file per provider at `<data-dir>/aib/auth/<provider>.json` --
//! the same per-user data dir every other `aib` subsystem uses (profiles,
//! traces, recordings), just a new subdirectory.

use crate::error::{LlmError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredTokens {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub id_token: Option<String>,
    /// Unix epoch seconds. Derived from the id_token's own `exp` claim at
    /// login/refresh time (the token endpoint's response here doesn't
    /// include a separate `expires_in` field -- confirmed against
    /// OpenAI's own Codex CLI source, not assumed), falling back to a
    /// conservative default if that claim is somehow absent.
    pub expires_at_epoch_s: u64,
    #[serde(default)]
    pub account_id: Option<String>,
}

pub struct TokenStore {
    dir: PathBuf,
}

impl TokenStore {
    pub fn new(auth_dir: PathBuf) -> Self {
        Self { dir: auth_dir }
    }

    fn path_for(&self, provider: &str) -> PathBuf {
        self.dir.join(format!("{provider}.json"))
    }

    #[allow(clippy::result_large_err)]
    pub fn load(&self, provider: &str) -> Result<Option<StoredTokens>> {
        let path = self.path_for(provider);
        if !path.is_file() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path).map_err(|source| LlmError::TokenStoreIo {
            path: path.clone(),
            source,
        })?;
        let tokens: StoredTokens = serde_json::from_str(&text)
            .map_err(|source| LlmError::TokenStoreParse { path, source })?;
        Ok(Some(tokens))
    }

    #[allow(clippy::result_large_err)]
    pub fn save(&self, provider: &str, tokens: &StoredTokens) -> Result<()> {
        std::fs::create_dir_all(&self.dir).map_err(|source| LlmError::TokenStoreIo {
            path: self.dir.clone(),
            source,
        })?;
        let path = self.path_for(provider);
        let text = serde_json::to_string_pretty(tokens).expect("StoredTokens always serializes");
        std::fs::write(&path, &text).map_err(|source| LlmError::TokenStoreIo {
            path: path.clone(),
            source,
        })?;
        restrict_permissions(&path);
        Ok(())
    }

    #[allow(clippy::result_large_err)]
    pub fn delete(&self, provider: &str) -> Result<()> {
        let path = self.path_for(provider);
        if path.is_file() {
            std::fs::remove_file(&path)
                .map_err(|source| LlmError::TokenStoreIo { path, source })?;
        }
        Ok(())
    }

    /// Every flow id with a stored token file -- used by `aib auth status`.
    /// An empty list (including when the auth directory doesn't exist yet
    /// at all) is not an error; it just means nobody has logged in.
    pub fn list(&self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                e.path()
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
            })
            .collect();
        names.sort();
        names
    }
}

/// Best-effort `chmod 600` on Unix -- tokens are secrets. Windows relies
/// on `%LOCALAPPDATA%`'s own per-user ACLs (already private to the
/// owning account by default); fighting Windows ACLs explicitly is out
/// of scope for this change.
#[cfg(unix)]
fn restrict_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "aib-llm-token-store-test-{name}-{}",
            std::process::id()
        ))
    }

    #[test]
    fn round_trips_save_load_delete() {
        let dir = temp_dir("roundtrip");
        let store = TokenStore::new(dir.clone());

        assert!(
            store.load("chatgpt").unwrap().is_none(),
            "nothing stored yet"
        );

        let tokens = StoredTokens {
            access_token: "at-123".to_string(),
            refresh_token: Some("rt-456".to_string()),
            id_token: Some("idt-789".to_string()),
            expires_at_epoch_s: 1_800_000_000,
            account_id: Some("acct-1".to_string()),
        };
        store.save("chatgpt", &tokens).expect("save succeeds");

        let loaded = store
            .load("chatgpt")
            .expect("load succeeds")
            .expect("present");
        assert_eq!(loaded.access_token, "at-123");
        assert_eq!(loaded.refresh_token.as_deref(), Some("rt-456"));
        assert_eq!(loaded.account_id.as_deref(), Some("acct-1"));

        store.delete("chatgpt").expect("delete succeeds");
        assert!(store.load("chatgpt").unwrap().is_none(), "deleted");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn corrupted_token_file_errors_clearly_instead_of_panicking() {
        let dir = temp_dir("corrupt");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("chatgpt.json"), "not valid json").unwrap();

        let store = TokenStore::new(dir.clone());
        let err = store
            .load("chatgpt")
            .expect_err("corrupt file is an error, not a panic");
        assert!(matches!(err, LlmError::TokenStoreParse { .. }));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn list_reports_every_stored_provider_and_is_empty_when_the_dir_is_absent() {
        let dir = temp_dir("list");
        let store = TokenStore::new(dir.clone());
        assert_eq!(
            store.list(),
            Vec::<String>::new(),
            "auth dir doesn't exist yet"
        );

        let tokens = StoredTokens {
            access_token: "at".to_string(),
            refresh_token: None,
            id_token: None,
            expires_at_epoch_s: 0,
            account_id: None,
        };
        store.save("chatgpt", &tokens).unwrap();
        store.save("another-flow", &tokens).unwrap();
        assert_eq!(
            store.list(),
            vec!["another-flow".to_string(), "chatgpt".to_string()]
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
