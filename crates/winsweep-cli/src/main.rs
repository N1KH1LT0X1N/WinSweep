//! WinSweep CLI Application
//!
//! Command-line interface for WinSweep disk cleaning tool.
//! Phase 1 implementation with TUI.

mod app;

use anyhow::Result;
use app::App;
use clap::Parser;
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[command(name = "winsweep")]
#[command(about = "A high-performance disk cleaning tool for Windows")]
#[command(version)]
struct Cli {
    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Log file path (optional)
    #[arg(short, long)]
    log_file: Option<String>,

    /// Start in specific mode
    #[arg(long, value_parser = ["scan", "wsl", "docker", "update", "services", "config"])]
    mode: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(if cli.verbose {
            Level::DEBUG
        } else {
            Level::INFO
        })
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting WinSweep CLI v{}", env!("CARGO_PKG_VERSION"));

    // Create and run the application
    let mut app = App::new()?;

    // Set initial mode if specified
    if let Some(mode) = cli.mode {
        app.mode = match mode.as_str() {
            "scan" => app::Mode::Scan,
            "wsl" => app::Mode::Wsl,
            "docker" => app::Mode::Docker,
            "update" => app::Mode::WindowsUpdate,
            "services" => app::Mode::Services,
            "config" => app::Mode::Config,
            _ => app::Mode::Main,
        };
    }

    // Run the TUI
    if let Err(e) = app.run().await {
        error!("Application error: {}", e);
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}
