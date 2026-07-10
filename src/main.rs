mod doctor;
mod mcp;

use clap::{Parser, Subcommand, ValueEnum};

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
        Command::Mcp { headed, browser } => mcp::run(!headed, browser.into()).await,
    }
}
