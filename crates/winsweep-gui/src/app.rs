//! Main GUI application structure

use crate::elevated_coordinator::ElevatedCoordinator;
use crate::viewmodel::WinSweepViewModel;
use anyhow::Result;
use eframe::egui;
use tracing::{debug, info};
use winsweep_common::Config;
use winsweep_core::{
    DockerClient, HomeEditionCompat, PackageManagerRegistry, WindowsEditionDetector, WslDetector,
};

#[cfg(feature = "system-tray")]
use crate::tray::{TrayEvent, TrayManager};

/// Main WinSweep GUI application
pub struct WinSweepApp {
    /// View model containing application state
    viewmodel: WinSweepViewModel,
    /// Configuration
    config: Config,
    /// Window visibility state
    window_visible: bool,
    /// Elevated operation coordinator
    elevated_coordinator: ElevatedCoordinator,
    #[cfg(feature = "system-tray")]
    /// System tray manager
    tray_manager: Option<TrayManager>,
}

impl WinSweepApp {
    /// Create a new GUI application instance
    pub async fn new() -> Result<Self> {
        info!("Initializing WinSweep GUI");

        // Load configuration
        let config = Config::load().unwrap_or_default();

        // Initialize core components
        let windows_detector = WindowsEditionDetector::new().ok();
        let wsl_detector = WslDetector::new().ok();
        let home_edition_compat = HomeEditionCompat::new().ok();
        let docker_client = DockerClient::new().await.ok();
        let package_manager_registry = PackageManagerRegistry::new();

        // Create view model
        let viewmodel = WinSweepViewModel::new(
            windows_detector,
            wsl_detector,
            home_edition_compat,
            docker_client,
            package_manager_registry,
            config.clone(),
        );

        #[cfg(feature = "system-tray")]
        let tray_manager = if config.ui.minimize_to_tray {
            Some(TrayManager::new()?)
        } else {
            None
        };

        // Create elevated coordinator
        let elevated_coordinator = ElevatedCoordinator::new(config.clone());

        Ok(Self {
            viewmodel,
            config,
            window_visible: true,
            elevated_coordinator,
            #[cfg(feature = "system-tray")]
            tray_manager,
        })
    }
}

impl eframe::App for WinSweepApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Set dark theme
        ctx.set_visuals(egui::Visuals::dark());

        // Configure top panel
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.add_space(5.0);

            // Title bar
            ui.horizontal(|ui| {
                ui.heading("WinSweep");
                ui.separator();
                ui.label("Disk Cleaning Tool for Windows");

                // Status indicator
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.viewmodel.is_operation_running() {
                        ui.spinner();
                        ui.label("Working...");
                    } else {
                        ui.label("Ready");
                    }
                });
            });

            ui.add_space(5.0);
        });

        // Main content area
        egui::CentralPanel::default().show(ctx, |ui| {
            // Navigation sidebar
            ui.horizontal(|ui| {
                // Sidebar
                ui.vertical(|ui| {
                    ui.heading("Navigation");
                    ui.separator();

                    let mut current_view = self.viewmodel.current_view();

                    if ui
                        .selectable_label(current_view == NavigationView::Dashboard, "📊 Dashboard")
                        .clicked()
                    {
                        self.viewmodel.set_current_view(NavigationView::Dashboard);
                    }

                    if ui
                        .selectable_label(current_view == NavigationView::Scan, "🔍 System Scan")
                        .clicked()
                    {
                        self.viewmodel.set_current_view(NavigationView::Scan);
                    }

                    if ui
                        .selectable_label(current_view == NavigationView::Wsl, "🐧 WSL Management")
                        .clicked()
                    {
                        self.viewmodel.set_current_view(NavigationView::Wsl);
                    }

                    if ui
                        .selectable_label(
                            current_view == NavigationView::Docker,
                            "🐳 Docker Cleanup",
                        )
                        .clicked()
                    {
                        self.viewmodel.set_current_view(NavigationView::Docker);
                    }

                    if ui
                        .selectable_label(
                            current_view == NavigationView::PackageManagers,
                            "📦 Package Managers",
                        )
                        .clicked()
                    {
                        self.viewmodel
                            .set_current_view(NavigationView::PackageManagers);
                    }

                    if ui
                        .selectable_label(
                            current_view == NavigationView::WindowsUpdate,
                            "🔄 Windows Update",
                        )
                        .clicked()
                    {
                        self.viewmodel
                            .set_current_view(NavigationView::WindowsUpdate);
                    }

                    if ui
                        .selectable_label(current_view == NavigationView::Services, "⚙️ Services")
                        .clicked()
                    {
                        self.viewmodel.set_current_view(NavigationView::Services);
                    }

                    if ui
                        .selectable_label(current_view == NavigationView::Settings, "📝 Settings")
                        .clicked()
                    {
                        self.viewmodel.set_current_view(NavigationView::Settings);
                    }
                });

                ui.separator();

                // Main content
                ui.vertical(|ui| match self.viewmodel.current_view() {
                    NavigationView::Dashboard => {
                        views::dashboard::show_dashboard(ui, &mut self.viewmodel);
                    }
                    NavigationView::Scan => {
                        views::scan::show_scan(ui, &mut self.viewmodel);
                    }
                    NavigationView::Wsl => {
                        views::wsl::show_wsl(ui, &mut self.viewmodel);
                    }
                    NavigationView::Docker => {
                        views::docker::show_docker(ui, &mut self.viewmodel);
                    }
                    NavigationView::PackageManagers => {
                        views::package_managers::show_package_managers(ui, &mut self.viewmodel);
                    }
                    NavigationView::WindowsUpdate => {
                        views::windows_update::show_windows_update(ui, &mut self.viewmodel);
                    }
                    NavigationView::Services => {
                        views::services::show_services(ui, &mut self.viewmodel);
                    }
                    NavigationView::Settings => {
                        views::settings::show_settings(ui, &mut self.viewmodel);
                    }
                });
            });
        });

        // Status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.separator();
            ui.horizontal(|ui| {
                // Status message
                if let Some(msg) = self.viewmodel.status_message() {
                    ui.label(msg);
                }

                // Progress bar
                if let Some(progress) = self.viewmodel.operation_progress() {
                    ui.add(egui::ProgressBar::new(progress).show_percentage());
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Version info
                    ui.label(format!("v{}", env!("CARGO_PKG_VERSION")));
                });
            });
        });

        // Handle background operations
        self.viewmodel.update(ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // Save application state
        eframe::set_value(storage, eframe::APP_KEY, &self.viewmodel);
    }
}
