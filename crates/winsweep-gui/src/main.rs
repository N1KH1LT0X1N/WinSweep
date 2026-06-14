//! WinSweep GUI Application
//!
//! Graphical user interface for WinSweep disk cleaning tool.
//! Phase 4 implementation using egui framework.

mod app;
mod elevated_coordinator;
mod notifications;
mod scheduler;
mod util;
mod viewmodel;
mod views;

#[cfg(feature = "system-tray")]
mod tray;

use anyhow::Result;
use app::WinSweepApp;
use clap::Parser;
use eframe::egui;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

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
        .with_max_level(if cli.verbose {
            Level::DEBUG
        } else {
            Level::INFO
        })
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
    // Leak the runtime so it stays alive for the app lifetime.
    let rt: &'static tokio::runtime::Runtime = Box::leak(Box::new(tokio::runtime::Runtime::new()?));
    let mut app = rt.block_on(async { WinSweepApp::new(rt).await })?;

    eframe::run_native(
        "WinSweep",
        options,
        Box::new(|cc| {
            // Load persisted state and restore skipped runtime / coordinator fields
            if let Some(storage) = cc.storage {
                if let Some(vm) = eframe::get_value(storage, eframe::APP_KEY) {
                    // Capture non-persisted fields before overwriting the viewmodel
                    let wsl_detector = app.viewmodel.wsl_detector.take();
                    let docker_client = app.viewmodel.docker_client.take();
                    let windows_detector = app.viewmodel.windows_detector.take();
                    let home_edition_compat = app.viewmodel.home_edition_compat.take();
                    let package_manager_registry =
                        std::mem::take(&mut app.viewmodel.package_manager_registry);

                    app.viewmodel = vm;

                    // Re-attach the captured fields
                    app.viewmodel.wsl_detector = wsl_detector;
                    app.viewmodel.docker_client = docker_client;
                    app.viewmodel.windows_detector = windows_detector;
                    app.viewmodel.home_edition_compat = home_edition_compat;
                    app.viewmodel.package_manager_registry = package_manager_registry;
                    app.viewmodel.runtime = Some(rt);
                    app.viewmodel.elevated_coordinator =
                        Some(crate::elevated_coordinator::ElevatedCoordinator::new());
                }
            }
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))?;

    Ok(())
}
