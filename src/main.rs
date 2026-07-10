mod doctor;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "aib",
    about = "ai-browser: an LLM-first browser-testing engine"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Attach to every installed Chromium browser and run the full
    /// attach→navigate→evaluate→screenshot cycle, reporting pass/fail and
    /// command round-trip latency (doctor-cli spec).
    Doctor {
        /// Emit a single machine-readable JSON report instead of text.
        #[arg(long)]
        json: bool,
        /// Launch browsers headed instead of headless.
        #[arg(long)]
        headed: bool,
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
        Command::Doctor { json, headed } => doctor::run(json, !headed).await,
    }
}
