//! WinSweep GUI Application
//! 
//! Graphical user interface for WinSweep disk cleaning tool.
//! Phase 4 implementation using egui framework.

mod app;
mod views;
mod viewmodel;
mod elevated_coordinator;

#[cfg(feature = "system-tray")]
mod tray;

use anyhow::Result;
use eframe::egui;
use tracing::{info, error, Level};
use tracing_subscriber::FmtSubscriber;
use app::WinSweepApp;

#[derive(clap::Parser, Debug)]
#[command(name = "winsweep-gui")]
#[command(about = "A high-performance disk cleaning tool for Windows")]
#[command(version)]
struct Cli {
    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
    
    /// Log file path (optional)
    #[arg(short, long)]
    log_file: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(if cli.verbose { Level::DEBUG } else { Level::INFO })
        .finish();
    
    tracing::subscriber::set_global_default(subscriber)?;
    
    info!("Starting WinSweep GUI v{}", env!("CARGO_PKG_VERSION"));
    
    // Configure egui
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("WinSweep - Disk Cleaning Tool"),
        ..Default::default()
    };
    
    // Run the GUI application
    let rt = tokio::runtime::Runtime::new()?;
    let app = rt.block_on(async {
        WinSweepApp::new().await
    })?;
    
    eframe::run_native(
        "WinSweep",
        options,
        Box::new(|_cc| {
            // This is where you can customize egui setup
            Box::new(app)
        }),
    )?;
    
    Ok(())
}
