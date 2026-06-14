//! Main GUI application structure

use crate::elevated_coordinator::ElevatedCoordinator;
use crate::viewmodel::{NavigationView, WinSweepViewModel};
use crate::views;
use anyhow::Result;
use eframe::egui;
use tracing::info;
use winsweep_common::t;
use winsweep_common::Config;
use winsweep_core::{
    DockerClient, HomeEditionCompat, PackageManagerRegistry, ServiceManager,
    WindowsEditionDetector, WslDetector,
};

#[cfg(feature = "system-tray")]
use crate::tray::{TrayEvent, TrayManager};

/// Main WinSweep GUI application
pub struct WinSweepApp {
    /// View model containing application state
    pub(crate) viewmodel: WinSweepViewModel,
    /// Window visibility state (only actively read when system-tray feature is on)
    #[allow(dead_code)]
    window_visible: bool,
    #[cfg(feature = "system-tray")]
    /// System tray manager
    tray_manager: Option<TrayManager>,
}

impl WinSweepApp {
    /// Create a new GUI application instance
    pub async fn new(runtime: &'static tokio::runtime::Runtime) -> Result<Self> {
        info!("Initializing WinSweep GUI");

        // Load configuration
        let config = Config::load().unwrap_or_default();

        // Initialize locale from config (falls back to English)
        winsweep_common::set_locale(&config.ui.language);

        // Initialize core components
        let windows_detector = WindowsEditionDetector::new().ok();
        let wsl_detector = WslDetector::new().ok();
        let home_edition_compat = HomeEditionCompat::new().ok();
        let docker_client = DockerClient::new().await.ok();
        let service_manager = ServiceManager::new().ok();
        let package_manager_registry = PackageManagerRegistry::new().await;

        // Create elevated coordinator (needed by view model)
        let elevated_coordinator = ElevatedCoordinator::new();

        // Create view model
        let mut viewmodel = WinSweepViewModel::new(
            windows_detector,
            wsl_detector,
            home_edition_compat,
            docker_client,
            service_manager,
            package_manager_registry,
            config.clone(),
            runtime,
            elevated_coordinator.clone(),
        );
        viewmodel.settings.sync_startup_from_registry();

        #[cfg(feature = "system-tray")]
        let tray_manager = if config.ui.minimize_to_tray {
            Some(TrayManager::new()?)
        } else {
            None
        };

        let _ = (config, elevated_coordinator, runtime);
        Ok(Self {
            viewmodel,
            window_visible: true,
            #[cfg(feature = "system-tray")]
            tray_manager,
        })
    }
}

impl eframe::App for WinSweepApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ── Tray event polling ────────────────────────────────────────────────
        #[cfg(feature = "system-tray")]
        {
            let tray_events: Vec<TrayEvent> = match self.tray_manager {
                Some(ref tray) => std::iter::from_fn(|| tray.next_event()).collect(),
                None => Vec::new(),
            };
            for event in tray_events {
                match event {
                    TrayEvent::Show => {
                        self.window_visible = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    }
                    TrayEvent::QuickScan => {
                        self.viewmodel.set_current_view(NavigationView::Scan);
                    }
                    TrayEvent::CleanTemp => {
                        self.viewmodel
                            .set_status_message(Some("Cleaning temp files…".to_string()));
                        self.viewmodel.start_elevated_task(
                            crate::elevated_coordinator::ElevatedOperation::CleanSystemTemp {
                                include_user_temp: true,
                                include_system_temp: true,
                            },
                            "Clean system temp files".to_string(),
                        );
                    }
                    TrayEvent::CleanAll => {
                        self.viewmodel.set_current_view(NavigationView::Scan);
                    }
                    TrayEvent::Settings => {
                        self.viewmodel.set_current_view(NavigationView::Settings);
                    }
                    TrayEvent::About => {
                        self.viewmodel.set_current_view(NavigationView::About);
                    }
                    TrayEvent::Quit => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
        }

        // ── Minimize-to-tray on window close ─────────────────────────────────
        #[cfg(feature = "system-tray")]
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.viewmodel.config().ui.minimize_to_tray && self.tray_manager.is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                self.window_visible = false;
            }
        }

        // Apply theme from config
        match self.viewmodel.config().ui.theme.as_str() {
            "light" => ctx.set_visuals(egui::Visuals::light()),
            "system" => ctx.set_visuals(egui::Visuals::dark()), // egui doesn't support system theme detection
            _ => ctx.set_visuals(egui::Visuals::dark()),
        }

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

        // ── Navigation sidebar ────────────────────────────────────────────────
        egui::SidePanel::left("nav_panel")
            .resizable(false)
            .exact_width(195.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.strong("WinSweep");
                ui.separator();

                let cv = self.viewmodel.current_view();

                macro_rules! nav {
                    ($label:expr, $view:expr) => {
                        if ui.selectable_label(cv == $view, $label).clicked() {
                            self.viewmodel.set_current_view($view);
                        }
                    };
                }

                nav!("📊  Dashboard", NavigationView::Dashboard);
                nav!("🔍  System Scan", NavigationView::Scan);
                nav!("🐧  WSL Management", NavigationView::Wsl);
                nav!("🐳  Docker Cleanup", NavigationView::Docker);
                nav!("📦  Package Managers", NavigationView::PackageManagers);
                nav!("�  Windows Update", NavigationView::WindowsUpdate);
                nav!("⚙️  Services", NavigationView::Services);
                nav!(
                    &format!("📝  {}", t!("nav_settings")),
                    NavigationView::Settings
                );

                // About pinned to the bottom
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.add_space(4.0);
                    if ui
                        .selectable_label(cv == NavigationView::About, "ℹ  About")
                        .clicked()
                    {
                        self.viewmodel.set_current_view(NavigationView::About);
                    }
                    ui.separator();
                });
            });

        // ── Main content ──────────────────────────────────────────────────────
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("main_content_scroll")
                .show(ui, |ui| match self.viewmodel.current_view() {
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
                    NavigationView::About => {
                        show_about(ui);
                    }
                });
        });

        // ── Confirmation dialog ───────────────────────────────────────────────
        let mut confirmed = false;
        let mut cancelled = false;
        if let Some(ref pending) = self.viewmodel.pending_cleanup {
            let desc = pending.description.clone();
            let count = pending.items.len();
            let size = pending.total_size;
            egui::Window::new("⚠  Confirm Delete")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(format!("{} — {} item(s)", desc, count));
                    ui.label(format!("Total size: {}", views::utils::format_bytes(size)));
                    ui.separator();
                    ui.label("⚠  This action cannot be undone. Proceed?");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("🗑️  Delete").clicked() {
                            confirmed = true;
                        }
                        if ui.button("Cancel").clicked() {
                            cancelled = true;
                        }
                    });
                });
        }
        if confirmed {
            if let Some(pending) = self.viewmodel.pending_cleanup.take() {
                self.viewmodel
                    .start_cleanup_task(pending.items, pending.description);
            }
        }
        if cancelled {
            self.viewmodel.pending_cleanup = None;
        }

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

fn show_about(ui: &mut egui::Ui) {
    ui.heading("About WinSweep");
    ui.separator();
    ui.label(format!("Version: {}", env!("CARGO_PKG_VERSION")));
    ui.label("A safe, high-performance disk cleaning tool for Windows.");
    ui.hyperlink("https://github.com/winsweep/winsweep");
    ui.add_space(8.0);
    ui.label("Licensed under the MIT License.");
}
