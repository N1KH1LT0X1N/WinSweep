//! View model layer for WinSweep GUI
//! 
//! This module contains the view models that separate the UI logic from the business logic.

mod dashboard;
mod scan;
mod wsl;
mod docker;
mod package_managers;
mod windows_update;
mod services;
mod settings;

use eframe::egui;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use winsweep_core::{WindowsEditionDetector, WslDetector, HomeEditionCompat, DockerClient, PackageManagerRegistry};
use winsweep_common::Config;

/// Main view model for the WinSweep application
#[derive(Serialize, Deserialize)]
pub struct WinSweepViewModel {
    /// Current navigation view
    #[serde(skip)]
    current_view: NavigationView,
    /// Windows edition detector
    #[serde(skip)]
    windows_detector: Option<WindowsEditionDetector>,
    /// WSL detector
    #[serde(skip)]
    wsl_detector: Option<WslDetector>,
    /// Home edition compatibility
    #[serde(skip)]
    home_edition_compat: Option<HomeEditionCompat>,
    /// Docker client
    #[serde(skip)]
    docker_client: Option<DockerClient>,
    /// Package manager registry
    #[serde(skip)]
    package_manager_registry: PackageManagerRegistry,
    /// Configuration
    config: Config,
    /// Status message
    status_message: Option<String>,
    /// Operation progress (0.0 to 1.0)
    operation_progress: Option<f32>,
    /// Whether an operation is currently running
    operation_running: bool,
    /// Last update time
    #[serde(skip)]
    last_update: Instant,
    /// Dashboard view model
    pub dashboard: dashboard::DashboardViewModel,
    /// Scan view model
    pub scan: scan::ScanViewModel,
    /// WSL view model
    pub wsl: wsl::WslViewModel,
    /// Docker view model
    pub docker: docker::DockerViewModel,
    /// Package managers view model
    pub package_managers: package_managers::PackageManagersViewModel,
    /// Windows Update view model
    pub windows_update: windows_update::WindowsUpdateViewModel,
    /// Services view model
    pub services: services::ServicesViewModel,
    /// Settings view model
    pub settings: settings::SettingsViewModel,
}

/// Navigation views
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NavigationView {
    Dashboard,
    Scan,
    Wsl,
    Docker,
    PackageManagers,
    WindowsUpdate,
    Services,
    Settings,
}

impl WinSweepViewModel {
    /// Create a new view model
    pub fn new(
        windows_detector: Option<WindowsEditionDetector>,
        wsl_detector: Option<WslDetector>,
        home_edition_compat: Option<HomeEditionCompat>,
        docker_client: Option<DockerClient>,
        package_manager_registry: PackageManagerRegistry,
        config: Config,
    ) -> Self {
        Self {
            current_view: NavigationView::Dashboard,
            windows_detector,
            wsl_detector,
            home_edition_compat,
            docker_client,
            package_manager_registry,
            config,
            status_message: None,
            operation_progress: None,
            operation_running: false,
            last_update: Instant::now(),
            dashboard: dashboard::DashboardViewModel::new(),
            scan: scan::ScanViewModel::new(),
            wsl: wsl::WslViewModel::new(),
            docker: docker::DockerViewModel::new(),
            package_managers: package_managers::PackageManagersViewModel::new(),
            windows_update: windows_update::WindowsUpdateViewModel::new(),
            services: services::ServicesViewModel::new(),
            settings: settings::SettingsViewModel::new(config),
        }
    }
    
    /// Get the current navigation view
    pub fn current_view(&self) -> NavigationView {
        self.current_view
    }
    
    /// Set the current navigation view
    pub fn set_current_view(&mut self, view: NavigationView) {
        self.current_view = view;
        self.set_status_message(None);
    }
    
    /// Get the status message
    pub fn status_message(&self) -> Option<&str> {
        self.status_message.as_deref()
    }
    
    /// Set the status message
    pub fn set_status_message(&mut self, message: Option<String>) {
        self.status_message = message;
    }
    
    /// Get the operation progress
    pub fn operation_progress(&self) -> Option<f32> {
        self.operation_progress
    }
    
    /// Set the operation progress
    pub fn set_operation_progress(&mut self, progress: Option<f32>) {
        self.operation_progress = progress;
    }
    
    /// Check if an operation is running
    pub fn is_operation_running(&self) -> bool {
        self.operation_running
    }
    
    /// Set whether an operation is running
    pub fn set_operation_running(&mut self, running: bool) {
        self.operation_running = running;
        if !running {
            self.operation_progress = None;
        }
    }
    
    /// Update the view model (called every frame)
    pub fn update(&mut self, ctx: &egui::Context) {
        // Request repaint if needed
        if self.operation_running {
            ctx.request_repaint();
        }
        
        // Update sub-view models
        self.dashboard.update();
        self.scan.update();
        self.wsl.update();
        self.docker.update();
        self.package_managers.update();
        self.windows_update.update();
        self.services.update();
        self.settings.update();
        
        self.last_update = Instant::now();
    }
    
    /// Getters for core components
    pub fn windows_detector(&self) -> Option<&WindowsEditionDetector> {
        self.windows_detector.as_ref()
    }
    
    pub fn wsl_detector(&self) -> Option<&WslDetector> {
        self.wsl_detector.as_ref()
    }
    
    pub fn home_edition_compat(&self) -> Option<&HomeEditionCompat> {
        self.home_edition_compat.as_ref()
    }
    
    pub fn docker_client(&self) -> Option<&DockerClient> {
        self.docker_client.as_ref()
    }
    
    pub fn docker_client_mut(&mut self) -> Option<&mut DockerClient> {
        self.docker_client.as_mut()
    }
    
    pub fn package_manager_registry(&self) -> &PackageManagerRegistry {
        &self.package_manager_registry
    }
    
    pub fn config(&self) -> &Config {
        &self.config
    }
    
    pub fn config_mut(&mut self) -> &mut Config {
        &mut self.config
    }
}

// Re-export sub-modules
pub use dashboard::*;
pub use scan::*;
pub use wsl::*;
pub use docker::*;
pub use package_managers::*;
pub use windows_update::*;
pub use services::*;
pub use settings::*;
