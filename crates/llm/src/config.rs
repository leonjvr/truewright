//! Config loading and role resolution (llm-providers spec: "Config file
//! loading", "Role resolution"). A missing config file is a valid, empty
//! configuration -- `aib`'s browser tools must keep working with no LLM
//! setup at all; only agent-facing commands need roles resolved, and they
//! fail with a clear, specific error at that point instead.

use crate::auth::CredentialSource;
use crate::client::{not_yet_implemented, Client, RoleClient};
use crate::client_compat::CompatClient;
use crate::error::{LlmError, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum ProviderKind {
    // Explicit #[serde(rename)], not rename_all = "kebab-case": serde's
    // case conversion splits "OpenAi" into "open-ai" (each capital starts
    // a new word), not the "openai" every provider's docs/this config
    // schema actually use -- caught by a real parse failure in this
    // module's own tests, not assumed.
    #[serde(rename = "openai-compat")]
    OpenAiCompat,
    #[serde(rename = "openai-responses")]
    OpenAiResponses,
}

impl ProviderKind {
    fn as_str(self) -> &'static str {
        match self {
            ProviderKind::OpenAiCompat => "openai-compat",
            ProviderKind::OpenAiResponses => "openai-responses",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RoleConfig {
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub vision: bool,
    #[serde(default)]
    pub guidance_skill: Option<String>,
}

fn default_max_steps() -> u32 {
    40
}
fn default_step_timeout_secs() -> u64 {
    120
}
fn default_task_timeout_secs() -> u64 {
    600
}
fn default_max_retained_snapshots() -> u32 {
    2
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AgentSettings {
    pub max_steps: u32,
    pub step_timeout_secs: u64,
    pub task_timeout_secs: u64,
    pub max_retained_snapshots: u32,
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            max_steps: default_max_steps(),
            step_timeout_secs: default_step_timeout_secs(),
            task_timeout_secs: default_task_timeout_secs(),
            max_retained_snapshots: default_max_retained_snapshots(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SkillsConfig {
    pub dirs: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct RawConfig {
    providers: BTreeMap<String, ProviderConfig>,
    roles: BTreeMap<String, RoleConfig>,
    agent: AgentSettings,
    skills: SkillsConfig,
}

pub struct Config {
    providers: BTreeMap<String, ProviderConfig>,
    roles: BTreeMap<String, RoleConfig>,
    pub agent: AgentSettings,
    pub skills: SkillsConfig,
}

impl Config {
    /// Empty configuration -- no providers, no roles. Valid; every
    /// browser-only code path keeps working. `resolve_role` on this always
    /// returns `UnknownRole`.
    pub fn empty() -> Self {
        Self {
            providers: BTreeMap::new(),
            roles: BTreeMap::new(),
            agent: AgentSettings::default(),
            skills: SkillsConfig::default(),
        }
    }

    /// Resolves the config file to load, in priority order: `explicit_path`
    /// (e.g. `--config`) -> `AIB_CONFIG` env var -> `./aib.toml` (project
    /// local) -> `<aib_data_dir>/config.toml`. `aib_data_dir` is the
    /// caller's already-resolved `<data-dir>/aib` (matching every other
    /// per-user path in this project, e.g. `cdp::launch::profile_base_dir`
    /// callers). A missing file at the resolved path is not an error --
    /// see `Config::empty`.
    #[allow(clippy::result_large_err)]
    pub fn load(aib_data_dir: &Path, explicit_path: Option<&Path>) -> Result<Self> {
        let path = if let Some(p) = explicit_path {
            p.to_path_buf()
        } else if let Ok(env_path) = std::env::var("AIB_CONFIG") {
            PathBuf::from(env_path)
        } else if Path::new("./aib.toml").is_file() {
            PathBuf::from("./aib.toml")
        } else {
            aib_data_dir.join("config.toml")
        };

        if !path.is_file() {
            return Ok(Config::empty());
        }

        let text = std::fs::read_to_string(&path).map_err(|source| LlmError::ConfigRead {
            path: path.clone(),
            source,
        })?;
        let raw: RawConfig =
            toml::from_str(&text).map_err(|source| LlmError::ConfigParse { path, source })?;

        Ok(Config {
            providers: raw.providers,
            roles: raw.roles,
            agent: raw.agent,
            skills: raw.skills,
        })
    }

    /// Resolves a role name (e.g. `"driver"`, `"vision"`) to a ready
    /// `RoleClient` -- looks up the role, then its provider, then builds
    /// the concrete client with resolved credentials.
    // LlmError is kept as one flat enum (matches cdp::CdpError's own
    // precedent, design.md Decision #5 there); boxing it would ripple
    // through call sites for marginal benefit at this size.
    #[allow(clippy::result_large_err)]
    pub fn resolve_role(&self, name: &str) -> Result<RoleClient> {
        let role = self
            .roles
            .get(name)
            .ok_or_else(|| LlmError::UnknownRole(name.to_string()))?;
        let provider =
            self.providers
                .get(&role.provider)
                .ok_or_else(|| LlmError::UnknownProvider {
                    role: name.to_string(),
                    provider: role.provider.clone(),
                })?;

        let client = match provider.kind {
            ProviderKind::OpenAiCompat => {
                let base_url =
                    provider
                        .base_url
                        .clone()
                        .ok_or_else(|| LlmError::UnknownProvider {
                            role: name.to_string(),
                            provider: role.provider.clone(),
                        })?;
                let credential = resolve_credential(provider, &role.provider)?;
                Client::Compat(CompatClient::new(
                    base_url,
                    credential,
                    provider.headers.clone(),
                ))
            }
            ProviderKind::OpenAiResponses => {
                return Err(not_yet_implemented(ProviderKind::OpenAiResponses.as_str()));
            }
        };

        Ok(RoleClient {
            client,
            model: role.model.clone(),
            vision: role.vision,
        })
    }

    /// True if `name` is defined under `[roles]` -- lets callers give a
    /// clear "not configured" message before attempting resolution (e.g.
    /// the agent loop checking for an optional vision role).
    pub fn has_role(&self, name: &str) -> bool {
        self.roles.contains_key(name)
    }
}

#[allow(clippy::result_large_err)]
fn resolve_credential(provider: &ProviderConfig, provider_name: &str) -> Result<CredentialSource> {
    if let Some(key) = &provider.api_key {
        return Ok(CredentialSource::Static(key.clone()));
    }
    if let Some(var) = &provider.api_key_env {
        let key = std::env::var(var).map_err(|_| LlmError::MissingCredential {
            provider: provider_name.to_string(),
            env_var: var.clone(),
        })?;
        return Ok(CredentialSource::Static(key));
    }
    Err(LlmError::NoCredentialConfigured {
        provider: provider_name.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::Client;

    fn write_temp_toml(name: &str, contents: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "aib-llm-config-test-{name}-{}.toml",
            std::process::id()
        ));
        std::fs::write(&path, contents).expect("write temp config");
        path
    }

    #[test]
    fn parses_full_config_and_resolves_role() {
        let path = write_temp_toml(
            "full",
            r#"
                [providers.deepseek]
                kind = "openai-compat"
                base_url = "https://api.deepseek.com/v1"
                api_key = "sk-literal-test-key"

                [roles.driver]
                provider = "deepseek"
                model = "deepseek-chat"
                vision = false

                [agent]
                max_steps = 10
            "#,
        );

        let config = Config::load(&PathBuf::from("unused"), Some(&path)).expect("config parses");
        std::fs::remove_file(&path).ok();

        assert_eq!(config.agent.max_steps, 10);
        // Fields not present in the [agent] table fall back to defaults.
        assert_eq!(config.agent.step_timeout_secs, default_step_timeout_secs());

        let role = config.resolve_role("driver").expect("driver role resolves");
        assert_eq!(role.model, "deepseek-chat");
        assert!(!role.vision);
        assert!(matches!(role.client, Client::Compat(_)));
    }

    #[test]
    fn missing_config_file_is_a_valid_empty_config() {
        let nonexistent = std::env::temp_dir().join("aib-llm-config-test-does-not-exist.toml");
        let config = Config::load(&PathBuf::from("unused"), Some(&nonexistent))
            .expect("missing file is not an error");
        assert!(!config.has_role("driver"));
        assert!(matches!(
            config.resolve_role("driver"),
            Err(LlmError::UnknownRole(_))
        ));
    }

    #[test]
    fn unknown_role_and_unknown_provider_error_clearly() {
        let path = write_temp_toml(
            "unknown-refs",
            r#"
                [providers.deepseek]
                kind = "openai-compat"
                base_url = "https://api.deepseek.com/v1"
                api_key = "sk-test"

                [roles.driver]
                provider = "not-configured-provider"
                model = "whatever"
            "#,
        );
        let config = Config::load(&PathBuf::from("unused"), Some(&path)).expect("config parses");
        std::fs::remove_file(&path).ok();

        assert!(matches!(
            config.resolve_role("missing-role"),
            Err(LlmError::UnknownRole(name)) if name == "missing-role"
        ));
        assert!(matches!(
            config.resolve_role("driver"),
            Err(LlmError::UnknownProvider { .. })
        ));
    }

    #[test]
    fn provider_with_no_credential_configured_errors_clearly() {
        let path = write_temp_toml(
            "no-credential",
            r#"
                [providers.deepseek]
                kind = "openai-compat"
                base_url = "https://api.deepseek.com/v1"

                [roles.driver]
                provider = "deepseek"
                model = "deepseek-chat"
            "#,
        );
        let config = Config::load(&PathBuf::from("unused"), Some(&path)).expect("config parses");
        std::fs::remove_file(&path).ok();

        assert!(matches!(
            config.resolve_role("driver"),
            Err(LlmError::NoCredentialConfigured { .. })
        ));
    }

    #[test]
    fn openai_responses_provider_is_not_yet_implemented() {
        let path = write_temp_toml(
            "responses-nyi",
            r#"
                [providers.chatgpt]
                kind = "openai-responses"
                api_key = "unused-in-this-change"

                [roles.driver]
                provider = "chatgpt"
                model = "gpt-5"
            "#,
        );
        let config = Config::load(&PathBuf::from("unused"), Some(&path)).expect("config parses");
        std::fs::remove_file(&path).ok();

        assert!(matches!(
            config.resolve_role("driver"),
            Err(LlmError::NotYetImplemented { kind }) if kind == "openai-responses"
        ));
    }

    // AIB_CHROME_PATH-style lesson from this project's own history: env-var
    // mutation is process-global and Rust's default test harness runs tests
    // concurrently, so every AIB_CONFIG/api_key_env assertion lives in one
    // test function rather than racing across several.
    #[test]
    fn env_var_precedence_and_credential_resolution() {
        let config_path = write_temp_toml(
            "env-config",
            r#"
                [providers.deepseek]
                kind = "openai-compat"
                base_url = "https://api.deepseek.com/v1"
                api_key_env = "AIB_LLM_TEST_DEEPSEEK_KEY"

                [roles.driver]
                provider = "deepseek"
                model = "deepseek-chat"
            "#,
        );

        // AIB_CONFIG is honored when no explicit path is given.
        std::env::set_var("AIB_CONFIG", &config_path);
        let config = Config::load(&PathBuf::from("unused-fallback"), None)
            .expect("config parses via AIB_CONFIG");

        // api_key_env resolves once the env var is set...
        std::env::set_var("AIB_LLM_TEST_DEEPSEEK_KEY", "sk-from-env");
        assert!(config.resolve_role("driver").is_ok());

        // ...and fails clearly when it isn't.
        std::env::remove_var("AIB_LLM_TEST_DEEPSEEK_KEY");
        assert!(matches!(
            config.resolve_role("driver"),
            Err(LlmError::MissingCredential { .. })
        ));

        std::env::remove_var("AIB_CONFIG");
        std::fs::remove_file(&config_path).ok();
    }
}
