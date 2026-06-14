//! WinSweep CLI Application
//!
//! Command-line interface for WinSweep disk cleaning tool.
//! Phase 1 implementation with TUI.

mod app;

use anyhow::Result;
use app::App;
use clap::Parser;
use std::path::PathBuf;
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;
use winsweep_common::types::ScanConfig;

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

    /// Only report artifact directories whose lock-file is older than N days.
    /// Example: --older 30  (skip projects touched in the last 30 days)
    #[arg(long, value_name = "DAYS")]
    older: Option<u32>,

    /// Output format (text or ndjson)
    #[arg(long, value_parser = ["text", "ndjson"])]
    output: Option<String>,

    /// Dry run: show what would be deleted without deleting
    #[arg(long)]
    dry_run: bool,

    /// Paths to scan (space-separated). Defaults to current directory.
    #[arg(value_name = "PATH")]
    paths: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging (writes to stderr so stdout stays clean for ndjson mode)
    let subscriber = FmtSubscriber::builder()
        .with_max_level(if cli.verbose {
            Level::DEBUG
        } else {
            Level::INFO
        })
        .with_writer(std::io::stderr)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting WinSweep CLI v{}", env!("CARGO_PKG_VERSION"));

    // NDJSON mode: bypass TUI and stream scan results as JSON lines
    if cli.output.as_deref() == Some("ndjson") {
        let config = ScanConfig {
            paths: if cli.paths.is_empty() {
                vec![std::env::current_dir()?]
            } else {
                cli.paths.iter().map(PathBuf::from).collect()
            },
            include_hidden: false,
            follow_symlinks: false,
            max_file_size: None,
            exclude_patterns: vec![],
            include_patterns: vec![],
            parallel_jobs: None,
            min_age_days: cli.older,
        };
        let scanner = winsweep_core::Scanner::new(config)?;
        let handle = scanner.scan().await?;
        let results = handle.collect_all().await;
        for result in results {
            println!("{}", serde_json::to_string(&result)?);
        }
        return Ok(());
    }

    // Create and run the application
    let mut app = App::new()?;
    app.dry_run = cli.dry_run;

    // Wire CLI flags into scan state
    if let Some(days) = cli.older {
        app.ui_state.scan.min_age_days = Some(days);
    }
    if !cli.paths.is_empty() {
        app.ui_state.scan.paths = cli.paths;
    }

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
