mod agent_cmd;
mod doctor;

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use truewright::mcp;

#[derive(Parser)]
#[command(
    name = "truewright",
    about = "truewright: an LLM-first browser-testing engine",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Which browser binary headless runs use (browser-attach spec: "Managed
/// chrome-headless-shell for headless runs").
#[derive(Clone, Copy, Default, ValueEnum)]
enum BrowserArg {
    /// Managed chrome-headless-shell (auto-downloaded and cached on first
    /// use), falling back to the installed browser if unavailable.
    #[default]
    Auto,
    /// Always the installed Chrome/Edge; never download anything.
    Installed,
}

impl From<BrowserArg> for cdp::launch::BrowserPreference {
    fn from(arg: BrowserArg) -> Self {
        match arg {
            BrowserArg::Auto => cdp::launch::BrowserPreference::Auto,
            BrowserArg::Installed => cdp::launch::BrowserPreference::Installed,
        }
    }
}

#[derive(Subcommand)]
enum Command {
    /// Run the full attach→navigate→evaluate→screenshot cycle against the
    /// managed headless-shell and every installed Chromium browser,
    /// reporting pass/fail, command latency, and process-tree memory
    /// (doctor-cli spec).
    Doctor {
        /// Emit a single machine-readable JSON report instead of text.
        #[arg(long)]
        json: bool,
        /// Launch browsers headed instead of headless.
        #[arg(long)]
        headed: bool,
        /// Browser selection for headless runs.
        #[arg(long, value_enum, default_value_t = BrowserArg::Auto)]
        browser: BrowserArg,
        /// Extra raw Chrome/Edge command-line flag to pass at launch
        /// (repeatable), e.g. `--chrome-arg=--no-sandbox
        /// --chrome-arg=--window-size=1440,900`. Merged with
        /// `[browser].extra_args` from config and the TRUEWRIGHT_CHROME_ARGS env
        /// var.
        #[arg(long = "chrome-arg", value_name = "FLAG")]
        chrome_args: Vec<String>,
        /// Shortcut for `--chrome-arg=--no-sandbox` (auto-applied anyway in
        /// a detected container/CI/root context; use this to force it when
        /// detection misses, e.g. an unprivileged LXC).
        #[arg(long)]
        no_sandbox: bool,
        /// Explicit config file path, overriding TRUEWRIGHT_CONFIG / ./truewright.toml /
        /// the per-user data dir default. Used to read `[browser].extra_args`.
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// Run the `browser` MCP server over stdio (mcp-server spec). Configure
    /// this as an MCP server in an agent host; the browser session is
    /// created lazily on the first tool call.
    Mcp {
        /// Launch the browser headed instead of headless.
        #[arg(long)]
        headed: bool,
        /// Browser selection for headless runs.
        #[arg(long, value_enum, default_value_t = BrowserArg::Auto)]
        browser: BrowserArg,
        /// Extra raw Chrome/Edge command-line flag to pass at launch
        /// (repeatable), e.g. `--chrome-arg=--kiosk
        /// --chrome-arg=--window-size=1440,900`. Merged with
        /// `[browser].extra_args` from config and the TRUEWRIGHT_CHROME_ARGS env
        /// var.
        #[arg(long = "chrome-arg", value_name = "FLAG")]
        chrome_args: Vec<String>,
        /// Shortcut for `--chrome-arg=--no-sandbox` (auto-applied anyway in
        /// a detected container/CI/root context; use this to force it when
        /// detection misses, e.g. an unprivileged LXC).
        #[arg(long)]
        no_sandbox: bool,
        /// Serve over loopback HTTP (bearer-token authenticated) instead of
        /// stdio (mcp-streamable-http spec).
        #[arg(long)]
        http: bool,
        /// Port for --http mode. Always binds 127.0.0.1 only.
        #[arg(long, default_value_t = 8787)]
        port: u16,
        /// Bearer token for --http mode. A random one is generated and
        /// printed once if not given.
        #[arg(long)]
        token: Option<String>,
        /// Explicit config file path, overriding TRUEWRIGHT_CONFIG / ./truewright.toml /
        /// the per-user data dir default. Used to resolve [roles.driver]/
        /// [roles.vision] for browser_run_task and screenshot
        /// interpretation (mcp-task-delegation spec); the server keeps
        /// working with those tools reporting a clear error if no driver
        /// role is configured.
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// Render or inspect saved traces (html-trace-viewer spec).
    Trace {
        #[command(subcommand)]
        action: TraceCommand,
    },
    /// LLM provider/role config utilities (llm-providers spec).
    Llm {
        #[command(subcommand)]
        action: LlmCommand,
    },
    /// OAuth login for subscription-based LLM providers (e.g. a ChatGPT
    /// subscription instead of a platform API key) (oauth-subscription-auth
    /// spec).
    Auth {
        #[command(subcommand)]
        action: AuthCommand,
    },
    /// Check for and install the latest release, by invoking the
    /// `truewright-update` companion binary installed alongside this one (from
    /// the shell/PowerShell installer). A stable, self-documenting alias so
    /// an auto-update timer can call one consistent command; the companion
    /// binary already does the actual work.
    Update {
        /// Extra arguments forwarded verbatim to `truewright-update`.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Runs `task` autonomously using a configured LLM role as the driver
    /// (agent-harness spec) -- `truewright` drives its own browser session
    /// end to end, printing live step progress. Exit code 0 on
    /// task_complete, 1 on task_failed/timeout/step-budget exhaustion, 2
    /// on a config/launch error.
    Agent {
        /// The task, in natural language.
        task: String,
        /// A skill name to attach (repeatable). Resolved from
        /// ./.truewright/skills/, then <data-dir>/truewright/skills/.
        #[arg(long = "skill")]
        skills: Vec<String>,
        /// Overrides [roles.driver] with a specific provider/model pair
        /// (e.g. "deepseek/deepseek-chat"), bypassing [roles.*] entirely.
        #[arg(long)]
        driver: Option<String>,
        /// Overrides [roles.vision] the same way --driver overrides
        /// [roles.driver].
        #[arg(long)]
        vision: Option<String>,
        /// Overrides [agent].max_steps.
        #[arg(long)]
        max_steps: Option<u32>,
        /// Launch the browser headed instead of headless.
        #[arg(long)]
        headed: bool,
        /// Browser selection for headless runs.
        #[arg(long, value_enum, default_value_t = BrowserArg::Auto)]
        browser: BrowserArg,
        /// Extra raw Chrome/Edge command-line flag to pass at launch
        /// (repeatable), e.g. `--chrome-arg=--kiosk
        /// --chrome-arg=--window-size=1440,900`. Merged with
        /// `[browser].extra_args` from config and the TRUEWRIGHT_CHROME_ARGS env
        /// var.
        #[arg(long = "chrome-arg", value_name = "FLAG")]
        chrome_args: Vec<String>,
        /// Shortcut for `--chrome-arg=--no-sandbox` (auto-applied anyway in
        /// a detected container/CI/root context; use this to force it when
        /// detection misses, e.g. an unprivileged LXC).
        #[arg(long)]
        no_sandbox: bool,
        /// Profile name (isolated browser profile dir). Fixed by default
        /// so repeated runs are deterministic, matching stdio-MCP's own
        /// posture.
        #[arg(long, default_value = "truewright-agent")]
        profile: String,
        /// Explicit config file path, overriding TRUEWRIGHT_CONFIG / ./truewright.toml /
        /// the per-user data dir default.
        #[arg(long)]
        config: Option<PathBuf>,
        /// Emit one JSON object per event on stdout instead of the
        /// human-readable progress lines, for scripting.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum AuthCommand {
    /// Runs the OAuth login flow for `flow` (currently just "chatgpt"):
    /// prints (and best-effort opens) a sign-in URL, waits for the
    /// browser redirect on a local callback server, and stores the
    /// resulting tokens under `<data-dir>/truewright/auth/<flow>.json`.
    Login { flow: String },
    /// Lists every provider with stored tokens and their expiry.
    Status,
    /// Deletes the stored tokens for `flow`, if any.
    Logout { flow: String },
}

#[derive(Subcommand)]
enum LlmCommand {
    /// Resolves a configured role and sends one trivial completion, to
    /// verify the provider/model/credential are actually reachable --
    /// prints the model, round-trip latency, and the reply.
    Ping {
        /// Role name from `[roles]` in the config file (e.g. "driver").
        role: String,
        /// Explicit config file path, overriding TRUEWRIGHT_CONFIG / ./truewright.toml /
        /// the per-user data dir default.
        #[arg(long)]
        config: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum TraceCommand {
    /// Render a saved trace (from browser_console_start/stop) as a
    /// self-contained HTML file, written alongside the trace itself.
    View {
        /// Name of a trace saved via browser_console_start/stop.
        name: String,
    },
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Doctor {
            json,
            headed,
            browser,
            chrome_args,
            no_sandbox,
            config,
        } => {
            let installed_only = matches!(
                cdp::launch::BrowserPreference::from(browser),
                cdp::launch::BrowserPreference::Installed
            );
            let extra_chrome_args = resolve_chrome_args(config.as_deref(), chrome_args, no_sandbox);
            doctor::run(json, !headed, installed_only, extra_chrome_args).await
        }
        Command::Mcp {
            headed,
            browser,
            chrome_args,
            no_sandbox,
            http,
            port,
            token,
            config,
        } => {
            let agent_config = build_mcp_agent_config(config.as_deref());
            let extra_chrome_args = resolve_chrome_args(config.as_deref(), chrome_args, no_sandbox);
            if http {
                mcp::run_http(
                    !headed,
                    browser.into(),
                    extra_chrome_args,
                    port,
                    token,
                    agent_config,
                )
                .await
            } else {
                mcp::run(!headed, browser.into(), extra_chrome_args, agent_config).await
            }
        }
        Command::Update { args } => run_self_update(&args),
        Command::Trace { action } => match action {
            TraceCommand::View { name } => match engine::render_trace_html(&name) {
                Ok(path) => {
                    println!("{}", path.display());
                    std::process::ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("failed to render trace {name:?}: {e}");
                    std::process::ExitCode::FAILURE
                }
            },
        },
        Command::Llm { action } => match action {
            LlmCommand::Ping { role, config } => llm_ping(&role, config).await,
        },
        Command::Auth { action } => match action {
            AuthCommand::Login { flow } => auth_login(&flow).await,
            AuthCommand::Status => auth_status(),
            AuthCommand::Logout { flow } => auth_logout(&flow),
        },
        Command::Agent {
            task,
            skills,
            driver,
            vision,
            max_steps,
            headed,
            browser,
            chrome_args,
            no_sandbox,
            profile,
            config,
            json,
        } => {
            let extra_chrome_args = resolve_chrome_args(config.as_deref(), chrome_args, no_sandbox);
            agent_cmd::run(
                &task,
                &skills,
                driver.as_deref(),
                vision.as_deref(),
                max_steps,
                headed,
                browser.into(),
                extra_chrome_args,
                &profile,
                config,
                json,
            )
            .await
        }
    }
}

/// Builds the optional agent config `truewright mcp` threads into every
/// `TruewrightTools` it creates. A missing config file (or a valid one with no
/// `[roles.driver]`) is the ordinary, zero-LLM-setup case -- silently
/// `None`, every browser-only tool keeps working exactly as before this
/// capability existed. A config file that *fails to parse* is different:
/// the user clearly meant to configure something, so it's a one-line
/// stderr warning, not silence (mcp-task-delegation design.md Decision
/// #5).
fn build_mcp_agent_config(
    config_path: Option<&std::path::Path>,
) -> Option<mcp_server::AgentConfig> {
    let truewright_data_dir = resolve_truewright_data_dir()
        .inspect_err(|e| {
            eprintln!("truewright mcp: failed to resolve per-user data directory: {e}")
        })
        .ok()?;

    let config = match llm::Config::load(&truewright_data_dir, config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "truewright mcp: failed to load LLM config ({e}); browser_run_task and screenshot \
                 interpretation will be unavailable until it's fixed"
            );
            return None;
        }
    };

    let driver = config.resolve_role("driver").ok()?;
    let vision = config.resolve_role("vision").ok();
    let harness = std::sync::Arc::new(agent::Harness {
        driver,
        vision,
        max_steps: config.agent.max_steps,
        step_timeout: std::time::Duration::from_secs(config.agent.step_timeout_secs),
        task_timeout: std::time::Duration::from_secs(config.agent.task_timeout_secs),
        max_retained_snapshots: config.agent.max_retained_snapshots,
    });
    let skill_dirs = agent::default_skill_dirs(&truewright_data_dir, &config.skills.dirs);
    Some(mcp_server::AgentConfig {
        harness,
        skill_dirs,
    })
}

/// Merges the three sources of extra Chrome flags into the final list a CLI
/// entry point threads down to launch: config `[browser].extra_args`, then
/// the repeated `--chrome-arg` values, then a `--no-sandbox` shortcut if
/// requested (de-duplicated against flags already present). The fourth
/// source, `TRUEWRIGHT_CHROME_ARGS`, is applied at the launch layer itself so
/// it reaches every path uniformly, and so is deliberately *not* merged
/// here.
fn resolve_chrome_args(
    config_path: Option<&std::path::Path>,
    cli_args: Vec<String>,
    no_sandbox: bool,
) -> Vec<String> {
    merge_chrome_args(config_browser_args(config_path), cli_args, no_sandbox)
}

/// The pure merge underlying [`resolve_chrome_args`], split out so the
/// ordering and `--no-sandbox` de-duplication are unit-testable without
/// touching the filesystem: config flags first, then CLI `--chrome-arg`,
/// then a `--no-sandbox` shortcut appended only if not already present.
fn merge_chrome_args(
    mut config_args: Vec<String>,
    cli_args: Vec<String>,
    no_sandbox: bool,
) -> Vec<String> {
    config_args.extend(cli_args);
    if no_sandbox && !config_args.iter().any(|a| a == "--no-sandbox") {
        config_args.push("--no-sandbox".to_string());
    }
    config_args
}

/// Reads `[browser].extra_args` from the resolved config, tolerating every
/// failure the way the browser tools always have: a missing/unparseable
/// config, or an unresolvable data dir, just yields no extra flags rather
/// than aborting a launch that would otherwise succeed.
fn config_browser_args(config_path: Option<&std::path::Path>) -> Vec<String> {
    let Ok(dir) = resolve_truewright_data_dir() else {
        return Vec::new();
    };
    match llm::Config::load(&dir, config_path) {
        Ok(config) => config.browser.extra_args,
        Err(_) => Vec::new(),
    }
}

/// Invokes the `truewright-update` companion binary installed alongside this
/// one, forwarding `args`. A thin, stable alias (task item #3) so an
/// auto-update timer calls one consistent `truewright update` regardless of how
/// the companion is named or where it lives.
fn run_self_update(args: &[String]) -> std::process::ExitCode {
    let updater = match locate_updater() {
        Some(path) => path,
        None => {
            eprintln!(
                "truewright update: companion updater '{}' not found next to this binary.",
                updater_file_name()
            );
            eprintln!(
                "It ships with the shell/PowerShell installer (`install-updater = true`). If you \
                 built from source or installed another way, update through that channel instead."
            );
            return std::process::ExitCode::FAILURE;
        }
    };

    match std::process::Command::new(&updater).args(args).status() {
        Ok(status) => match status.code() {
            Some(0) => std::process::ExitCode::SUCCESS,
            // Preserve the updater's own non-zero exit code where it fits a
            // u8; a signal death (None) has no code, so report a generic
            // failure.
            Some(code) => std::process::ExitCode::from(code.clamp(1, 255) as u8),
            None => std::process::ExitCode::FAILURE,
        },
        Err(e) => {
            eprintln!(
                "truewright update: failed to run {}: {e}",
                updater.display()
            );
            std::process::ExitCode::FAILURE
        }
    }
}

fn updater_file_name() -> &'static str {
    if cfg!(windows) {
        "truewright-update.exe"
    } else {
        "truewright-update"
    }
}

/// Locates the `truewright-update` companion in the same directory as the
/// running `truewright` executable — where every installer places the pair.
fn locate_updater() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let candidate = exe.parent()?.join(updater_file_name());
    candidate.is_file().then_some(candidate)
}

/// Resolves `role` from config and sends one trivial completion, printing
/// model/latency/reply -- the live-verification hook for the llm-providers
/// change (no browser session involved at all).
async fn llm_ping(role: &str, config_path: Option<PathBuf>) -> std::process::ExitCode {
    let truewright_data_dir = match resolve_truewright_data_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("failed to resolve per-user data directory: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };

    let config = match llm::Config::load(&truewright_data_dir, config_path.as_deref()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to load config: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };

    let role_client = match config.resolve_role(role) {
        Ok(rc) => rc,
        Err(e) => {
            eprintln!("failed to resolve role {role:?}: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };

    let req = llm::ChatRequest {
        model: String::new(), // RoleClient::complete fills in the configured model
        messages: vec![
            llm::Message::system("You are a terse connectivity test. Reply with exactly one word."),
            llm::Message::user("Reply with exactly: pong"),
        ],
        tools: vec![],
    };

    let start = std::time::Instant::now();
    match role_client.complete(req).await {
        Ok(resp) => {
            let elapsed = start.elapsed();
            println!(
                "role={role} model={} latency={elapsed:?}",
                role_client.model
            );
            println!("reply: {}", resp.message.text());
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("ping failed: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// `<data-dir>/truewright`, the same per-user directory every other `truewright`
/// subsystem uses (profiles, traces, recordings) -- OAuth tokens live at
/// `<this>/auth/<flow>.json`.
// CdpError is kept as one flat enum (matches its own design.md Decision
// #5); boxing it here too would ripple through call sites for marginal
// benefit at this size.
#[allow(clippy::result_large_err)]
pub(crate) fn resolve_truewright_data_dir() -> cdp::error::Result<PathBuf> {
    cdp::launch::profile_base_dir().map(|dir| dir.join("truewright"))
}

/// Runs the OAuth login flow and prints the outcome. The flow itself
/// prints the sign-in URL and best-effort opens it; this just reports
/// success/failure once the browser redirect completes.
async fn auth_login(flow: &str) -> std::process::ExitCode {
    let truewright_data_dir = match resolve_truewright_data_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("failed to resolve per-user data directory: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let store = llm::TokenStore::new(truewright_data_dir.join("auth"));

    match llm::oauth_login(flow, &store).await {
        Ok(tokens) => {
            println!("Signed in to {flow}.");
            if let Some(account_id) = &tokens.account_id {
                println!("Account: {account_id}");
            }
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("login failed: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn auth_status() -> std::process::ExitCode {
    let truewright_data_dir = match resolve_truewright_data_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("failed to resolve per-user data directory: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let store = llm::TokenStore::new(truewright_data_dir.join("auth"));

    let flows = store.list();
    if flows.is_empty() {
        println!("No stored logins. Run `truewright auth login <flow>` (e.g. `truewright auth login chatgpt`).");
        return std::process::ExitCode::SUCCESS;
    }
    for flow in flows {
        match store.load(&flow) {
            Ok(Some(tokens)) => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let status = if tokens.expires_at_epoch_s > now {
                    format!("expires in {}s", tokens.expires_at_epoch_s - now)
                } else {
                    "expired (will refresh on next use)".to_string()
                };
                let account = tokens.account_id.as_deref().unwrap_or("(no account id)");
                println!("{flow}: {account} -- {status}");
            }
            Ok(None) => println!("{flow}: (no longer present)"),
            Err(e) => println!("{flow}: error reading stored tokens: {e}"),
        }
    }
    std::process::ExitCode::SUCCESS
}

fn auth_logout(flow: &str) -> std::process::ExitCode {
    let truewright_data_dir = match resolve_truewright_data_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("failed to resolve per-user data directory: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let store = llm::TokenStore::new(truewright_data_dir.join("auth"));

    match store.delete(flow) {
        Ok(()) => {
            println!("Signed out of {flow}.");
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("logout failed: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::merge_chrome_args;

    #[test]
    fn merge_orders_config_then_cli_and_appends_no_sandbox_once() {
        // Config flags come first, then the repeated --chrome-arg values.
        let merged = merge_chrome_args(
            vec!["--kiosk".into()],
            vec!["--window-size=1440,900".into()],
            false,
        );
        assert_eq!(merged, vec!["--kiosk", "--window-size=1440,900"]);

        // --no-sandbox shortcut appends when absent...
        let merged = merge_chrome_args(vec!["--kiosk".into()], vec![], true);
        assert_eq!(merged, vec!["--kiosk", "--no-sandbox"]);

        // ...but never duplicates one already supplied via config or CLI.
        let from_config = merge_chrome_args(vec!["--no-sandbox".into()], vec![], true);
        assert_eq!(from_config, vec!["--no-sandbox"]);
        let from_cli = merge_chrome_args(vec![], vec!["--no-sandbox".into()], true);
        assert_eq!(from_cli, vec!["--no-sandbox"]);

        // No shortcut requested and no flags anywhere -> empty.
        assert!(merge_chrome_args(vec![], vec![], false).is_empty());
    }
}
