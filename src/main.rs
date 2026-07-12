mod doctor;

use aib::mcp;
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "aib",
    about = "ai-browser: an LLM-first browser-testing engine"
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
}

#[derive(Subcommand)]
enum AuthCommand {
    /// Runs the OAuth login flow for `flow` (currently just "chatgpt"):
    /// prints (and best-effort opens) a sign-in URL, waits for the
    /// browser redirect on a local callback server, and stores the
    /// resulting tokens under `<data-dir>/aib/auth/<flow>.json`.
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
        /// Explicit config file path, overriding AIB_CONFIG / ./aib.toml /
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
        } => {
            let installed_only = matches!(
                cdp::launch::BrowserPreference::from(browser),
                cdp::launch::BrowserPreference::Installed
            );
            doctor::run(json, !headed, installed_only).await
        }
        Command::Mcp {
            headed,
            browser,
            http,
            port,
            token,
        } => {
            if http {
                mcp::run_http(!headed, browser.into(), port, token).await
            } else {
                mcp::run(!headed, browser.into()).await
            }
        }
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
    }
}

/// Resolves `role` from config and sends one trivial completion, printing
/// model/latency/reply -- the live-verification hook for the llm-providers
/// change (no browser session involved at all).
async fn llm_ping(role: &str, config_path: Option<PathBuf>) -> std::process::ExitCode {
    let aib_data_dir = match resolve_aib_data_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("failed to resolve per-user data directory: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };

    let config = match llm::Config::load(&aib_data_dir, config_path.as_deref()) {
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
            println!("role={role} model={} latency={elapsed:?}", role_client.model);
            println!("reply: {}", resp.message.text());
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("ping failed: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// `<data-dir>/aib`, the same per-user directory every other `aib`
/// subsystem uses (profiles, traces, recordings) -- OAuth tokens live at
/// `<this>/auth/<flow>.json`.
// CdpError is kept as one flat enum (matches its own design.md Decision
// #5); boxing it here too would ripple through call sites for marginal
// benefit at this size.
#[allow(clippy::result_large_err)]
fn resolve_aib_data_dir() -> cdp::error::Result<PathBuf> {
    cdp::launch::profile_base_dir().map(|dir| dir.join("aib"))
}

/// Runs the OAuth login flow and prints the outcome. The flow itself
/// prints the sign-in URL and best-effort opens it; this just reports
/// success/failure once the browser redirect completes.
async fn auth_login(flow: &str) -> std::process::ExitCode {
    let aib_data_dir = match resolve_aib_data_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("failed to resolve per-user data directory: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let store = llm::TokenStore::new(aib_data_dir.join("auth"));

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
    let aib_data_dir = match resolve_aib_data_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("failed to resolve per-user data directory: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let store = llm::TokenStore::new(aib_data_dir.join("auth"));

    let flows = store.list();
    if flows.is_empty() {
        println!("No stored logins. Run `aib auth login <flow>` (e.g. `aib auth login chatgpt`).");
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
    let aib_data_dir = match resolve_aib_data_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("failed to resolve per-user data directory: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let store = llm::TokenStore::new(aib_data_dir.join("auth"));

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
