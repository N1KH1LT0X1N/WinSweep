//! View model layer for WinSweep GUI
//!
//! This module contains the view models that separate the UI logic from the business logic.

mod dashboard;
mod docker;
mod package_managers;
pub mod scan;
mod services;
pub mod settings;
pub mod windows_update;
mod wsl;

use eframe::egui;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use winsweep_common::Config;
use winsweep_core::{
    DockerClient, HomeEditionCompat, PackageManagerRegistry, ServiceManager,
    WindowsEditionDetector, WslDetector,
};

/// Result type returned by a background task
pub enum BackgroundResult {
    Cleanup(winsweep_common::types::CleanupResult),
    Elevated(crate::elevated_coordinator::ElevatedOperationResult),
    DockerRefresh(Result<docker::DockerResources, String>),
    DockerPrune(Result<u64, String>),
    PackageManagerRefresh(Result<Vec<package_managers::PackageManagerInfo>, String>),
    PackageManagerClean(Result<winsweep_core::PackageCleanResult, String>),
    ServiceAction(Result<String, String>),
    WindowsUpdateCleanup(Result<String, String>),
}

/// Main view model for the WinSweep application
#[derive(Serialize, Deserialize)]
pub struct WinSweepViewModel {
    /// Current navigation view
    #[serde(skip)]
    current_view: NavigationView,
    /// Windows edition detector
    #[serde(skip)]
    pub windows_detector: Option<WindowsEditionDetector>,
    /// WSL detector
    #[serde(skip)]
    pub wsl_detector: Option<WslDetector>,
    /// Home edition compatibility
    #[serde(skip)]
    pub home_edition_compat: Option<HomeEditionCompat>,
    /// Docker client
    #[serde(skip)]
    pub docker_client: Option<DockerClient>,
    /// Service manager
    #[serde(skip)]
    pub service_manager: Option<ServiceManager>,
    /// Package manager registry
    #[serde(skip)]
    pub package_manager_registry: PackageManagerRegistry,
    /// Configuration
    config: Config,
    /// Status message
    status_message: Option<String>,
    /// Operation progress (0.0 to 1.0)
    operation_progress: Option<f32>,
    /// Whether an operation is currently running
    operation_running: bool,
    /// Last update time
    #[serde(skip, default = "default_instant")]
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
    /// Tokio runtime for blocking on and spawning async tasks (not persisted)
    #[serde(skip)]
    pub runtime: Option<&'static tokio::runtime::Runtime>,
    /// Active background task handle for polling async work (not persisted)
    #[serde(skip)]
    pub background_handle: Option<tokio::task::JoinHandle<anyhow::Result<BackgroundResult>>>,
    /// Description of the active background task (for status display)
    #[serde(skip)]
    pub background_task_description: Option<String>,
    /// Elevated operation coordinator (not persisted)
    #[serde(skip)]
    pub elevated_coordinator: Option<crate::elevated_coordinator::ElevatedCoordinator>,
    /// Cleanup items waiting for user confirmation (not persisted)
    #[serde(skip)]
    pub pending_cleanup: Option<PendingCleanup>,
}

/// Navigation views
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum NavigationView {
    #[default]
    Dashboard,
    Scan,
    Wsl,
    Docker,
    PackageManagers,
    WindowsUpdate,
    Services,
    Settings,
    About,
}

/// Items staged for confirmation before a destructive cleanup operation.
#[derive(Debug)]
pub struct PendingCleanup {
    pub items: Vec<winsweep_common::types::ScanResult>,
    pub description: String,
    pub total_size: u64,
}

impl WinSweepViewModel {
    /// Create a new view model
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        windows_detector: Option<WindowsEditionDetector>,
        wsl_detector: Option<WslDetector>,
        home_edition_compat: Option<HomeEditionCompat>,
        docker_client: Option<DockerClient>,
        service_manager: Option<ServiceManager>,
        package_manager_registry: PackageManagerRegistry,
        config: Config,
        runtime: &'static tokio::runtime::Runtime,
        elevated_coordinator: crate::elevated_coordinator::ElevatedCoordinator,
    ) -> Self {
        Self {
            current_view: NavigationView::Dashboard,
            windows_detector,
            wsl_detector,
            home_edition_compat,
            docker_client,
            service_manager,
            package_manager_registry,
            config: config.clone(),
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
            runtime: Some(runtime),
            background_handle: None,
            background_task_description: None,
            elevated_coordinator: Some(elevated_coordinator),
            pending_cleanup: None,
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

        // --- Auto-cleanup scheduler ---
        if self.config.auto_cleanup_enabled && !self.is_operation_running() {
            let threshold =
                std::time::Duration::from_secs(self.config.auto_cleanup_days as u64 * 86400);
            if should_auto_clean(&self.dashboard.last_auto_cleanup, threshold) {
                self.dashboard.last_auto_cleanup = Some(chrono::Local::now().to_rfc3339());
                if self.config.cleanup.clean_temp_files {
                    self.start_elevated_task(
                        crate::elevated_coordinator::ElevatedOperation::CleanSystemTemp {
                            include_user_temp: true,
                            include_system_temp: self.config.cleanup.clean_temp_files,
                        },
                        "Auto cleanup — temp files".to_string(),
                    );
                }
                if self.config.cleanup.clean_browser_cache {
                    self.start_browser_cache_clean_task();
                }
                if self.config.cleanup.clean_prefetch {
                    self.start_elevated_task(
                        crate::elevated_coordinator::ElevatedOperation::CleanPrefetch,
                        "Auto cleanup — prefetch".to_string(),
                    );
                }
            }
        }

        // Low-disk notification (fire once per "low" transition)
        if self.config.notify_low_disk_space && self.config.ui.show_notifications {
            let total = self.dashboard.system_info.total_disk_space;
            let free = self.dashboard.system_info.free_disk_space;
            if total > 0 {
                let free_pct = (free as f64 / total as f64) * 100.0;
                let threshold = self.config.low_disk_threshold as f64;
                if free_pct < threshold {
                    if !self.dashboard.low_disk_notified {
                        self.dashboard.low_disk_notified = true;
                        crate::notifications::show_toast_safe(
                            "Low Disk Space",
                            &format!("Only {:.1}% free (threshold: {}%)", free_pct, threshold),
                        );
                    }
                } else {
                    self.dashboard.low_disk_notified = false;
                }
            }
        }
        self.scan.update();
        // If the scan just finished, feed its category breakdown into the dashboard
        if let Some(breakdown) = self.scan.pending_category_breakdown.take() {
            self.dashboard.category_breakdown = breakdown.clone();
            self.dashboard.quick_stats.temp_files_size = breakdown.temp_bytes;
            self.dashboard.quick_stats.package_cache_size = breakdown.package_cache_bytes;
            if self.config.ui.show_notifications {
                crate::notifications::show_toast_safe(
                    "Scan Complete",
                    &format!(
                        "Found {} reclaimable bytes across {} items",
                        breakdown.artifact_bytes
                            + breakdown.temp_bytes
                            + breakdown.package_cache_bytes
                            + breakdown.recycle_bin_bytes
                            + breakdown.other_bytes,
                        self.scan.scan_results.len()
                    ),
                );
            }
        }
        self.wsl.update();
        self.docker.update();
        self.package_managers.update();
        self.windows_update.update();
        self.services.update();
        self.settings.update();

        // Poll active background task
        if let Some(ref handle) = self.background_handle {
            if handle.is_finished() {
                let handle = self.background_handle.take().unwrap();
                let desc = self.background_task_description.take().unwrap_or_default();
                if let Some(rt) = self.runtime {
                    if let Ok(Ok(result)) = rt.block_on(handle) {
                        match result {
                            BackgroundResult::Cleanup(cleanup) => {
                                let success = cleanup.items_failed.is_empty();
                                self.dashboard.record_operation(
                                    desc.clone(),
                                    cleanup.space_freed_bytes,
                                    success,
                                );
                                if self.config.ui.show_notifications
                                    && self.config.notify_cleanup_complete
                                {
                                    crate::notifications::show_toast_safe(
                                        if success {
                                            "Cleanup Complete"
                                        } else {
                                            "Cleanup Partial"
                                        },
                                        &format!("Freed {} bytes", cleanup.space_freed_bytes),
                                    );
                                }
                            }
                            BackgroundResult::Elevated(elevated) => {
                                self.dashboard.record_operation(
                                    desc.clone(),
                                    elevated.space_freed,
                                    elevated.success,
                                );
                                if self.config.ui.show_notifications
                                    && self.config.notify_cleanup_complete
                                {
                                    crate::notifications::show_toast_safe(
                                        if elevated.success {
                                            "Operation Complete"
                                        } else {
                                            "Operation Failed"
                                        },
                                        &format!("{}: freed {} bytes", desc, elevated.space_freed),
                                    );
                                }
                            }
                            BackgroundResult::DockerRefresh(Ok(resources)) => {
                                self.docker.resources = resources;
                                self.docker.status_message =
                                    Some("Docker resources refreshed".to_string());
                                let image_bytes: u64 =
                                    self.docker.resources.images.iter().map(|i| i.size).sum();
                                let volume_bytes: u64 = self
                                    .docker
                                    .resources
                                    .volumes
                                    .iter()
                                    .filter_map(|v| v.size)
                                    .sum();
                                self.dashboard.quick_stats.docker_cache_size =
                                    image_bytes + volume_bytes;
                            }
                            BackgroundResult::DockerRefresh(Err(e)) => {
                                self.docker.status_message =
                                    Some(format!("Docker refresh failed: {}", e));
                            }
                            BackgroundResult::DockerPrune(Ok(space_freed)) => {
                                self.dashboard
                                    .record_operation(desc.clone(), space_freed, true);
                                if self.config.ui.show_notifications
                                    && self.config.notify_cleanup_complete
                                {
                                    crate::notifications::show_toast_safe(
                                        "Docker Prune Complete",
                                        &format!("Freed {} bytes", space_freed),
                                    );
                                }
                            }
                            BackgroundResult::DockerPrune(Err(e)) => {
                                self.docker.status_message =
                                    Some(format!("Docker prune failed: {}", e));
                            }
                            BackgroundResult::PackageManagerRefresh(Ok(managers)) => {
                                self.package_managers.managers = managers;
                                self.package_managers.status_message =
                                    Some("Package managers refreshed".to_string());
                            }
                            BackgroundResult::PackageManagerRefresh(Err(e)) => {
                                self.package_managers.status_message =
                                    Some(format!("Refresh failed: {}", e));
                            }
                            BackgroundResult::PackageManagerClean(Ok(result)) => {
                                self.dashboard.record_operation(
                                    desc.clone(),
                                    result.space_freed,
                                    result.errors.is_empty(),
                                );
                                if self.config.ui.show_notifications
                                    && self.config.notify_cleanup_complete
                                {
                                    crate::notifications::show_toast_safe(
                                        "Cache Clean Complete",
                                        &format!(
                                            "Freed {} bytes from {}",
                                            result.space_freed, result.package_manager
                                        ),
                                    );
                                }
                            }
                            BackgroundResult::PackageManagerClean(Err(e)) => {
                                self.package_managers.status_message =
                                    Some(format!("Clean failed: {}", e));
                            }
                            BackgroundResult::ServiceAction(Ok(service_name)) => {
                                self.services.status_message =
                                    Some(format!("Service action completed for {}", service_name));
                                if let Some(ref sm) = self.service_manager {
                                    let _ = self.services.refresh_services(sm);
                                }
                            }
                            BackgroundResult::ServiceAction(Err(e)) => {
                                self.services.status_message =
                                    Some(format!("Service action failed: {}", e));
                            }
                            BackgroundResult::WindowsUpdateCleanup(result) => {
                                self.windows_update.cleanup_in_progress = false;
                                match result {
                                    Ok(msg) => {
                                        self.windows_update.status_message = Some(msg);
                                    }
                                    Err(e) => {
                                        self.windows_update.status_message =
                                            Some(format!("Cleanup failed: {}", e));
                                    }
                                }
                            }
                        }
                    }
                }
                self.set_operation_running(false);
            }
        }

        self.last_update = Instant::now();
    }

    pub fn docker_client(&self) -> Option<&DockerClient> {
        self.docker_client.as_ref()
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Set the live configuration (used after settings are saved)
    pub fn set_config(&mut self, config: Config) {
        self.config = config;
    }

    /// Start a background cleanup task for the given scan results
    pub fn start_cleanup_task(
        &mut self,
        items: Vec<winsweep_common::types::ScanResult>,
        description: String,
    ) {
        if self.is_operation_running() {
            return;
        }
        if let Some(rt) = self.runtime {
            self.set_operation_running(true);
            self.background_task_description = Some(description.clone());
            let use_recycle_bin = self.config.cleanup.use_recycle_bin;
            self.background_handle = Some(rt.spawn(async move {
                let windows_api = std::sync::Arc::new(winsweep_core::WindowsApi::new()?);
                let audit_logger = std::sync::Arc::new(winsweep_core::AuditLogger::new()?);
                let manager = winsweep_core::CleanupManager::new(
                    windows_api,
                    audit_logger,
                    use_recycle_bin,
                    false,
                    false,
                );
                let scan_id = uuid::Uuid::new_v4();
                let result = manager.cleanup(scan_id, items).await?;
                Ok(BackgroundResult::Cleanup(result))
            }));
        }
    }

    /// Stage items for a confirmation dialog instead of deleting immediately.
    pub fn set_pending_cleanup(
        &mut self,
        items: Vec<winsweep_common::types::ScanResult>,
        description: String,
    ) {
        let total_size = items.iter().map(|r| r.size_bytes).sum();
        self.pending_cleanup = Some(PendingCleanup {
            items,
            description,
            total_size,
        });
    }

    /// Start a background elevated operation task
    pub fn start_elevated_task(
        &mut self,
        operation: crate::elevated_coordinator::ElevatedOperation,
        description: String,
    ) {
        if self.is_operation_running() {
            return;
        }
        if let (Some(rt), Some(coord)) = (self.runtime, self.elevated_coordinator.clone()) {
            self.set_operation_running(true);
            self.background_task_description = Some(description.clone());
            self.background_handle = Some(rt.spawn(async move {
                let result = coord.execute_operation(operation, |_progress| {}).await?;
                Ok(BackgroundResult::Elevated(result))
            }));
        }
    }

    /// Start an async Docker resource refresh
    pub fn start_docker_refresh_task(&mut self) {
        if self.is_operation_running() {
            return;
        }
        if let Some(rt) = self.runtime {
            if let Some(ref docker_client) = self.docker_client.clone() {
                self.set_operation_running(true);
                self.background_task_description = Some("Refresh Docker resources".to_string());
                let dc = docker_client.clone();
                self.background_handle = Some(rt.spawn(async move {
                    let resources = docker::DockerResources {
                        containers: dc.get_containers().await.map_err(|e| anyhow::anyhow!(e))?,
                        images: dc.get_images().await.map_err(|e| anyhow::anyhow!(e))?,
                        volumes: dc.get_volumes().await.map_err(|e| anyhow::anyhow!(e))?,
                        networks: dc.get_networks().await.map_err(|e| anyhow::anyhow!(e))?,
                    };
                    Ok(BackgroundResult::DockerRefresh(Ok(resources)))
                }));
            }
        }
    }

    /// Start a Docker prune for the given resource type
    pub fn start_docker_prune_task(&mut self, resource_type: &str) {
        if self.is_operation_running() {
            return;
        }
        if let Some(rt) = self.runtime {
            if let Some(ref docker_client) = self.docker_client.clone() {
                self.set_operation_running(true);
                self.background_task_description = Some(format!("Prune Docker {}", resource_type));
                let dc = docker_client.clone();
                let resource = resource_type.to_string();
                self.background_handle = Some(rt.spawn(async move {
                    let space_freed = match resource.as_str() {
                        "containers" => {
                            let containers =
                                dc.get_containers().await.map_err(|e| anyhow::anyhow!(e))?;
                            let mut freed = 0u64;
                            for c in containers {
                                if matches!(
                                    c.status,
                                    winsweep_core::docker::ContainerStatus::Exited
                                ) && dc.remove_container(&c.id, false).await.is_ok()
                                {
                                    freed += c.size_rw.unwrap_or(0) + c.size_root_fs.unwrap_or(0);
                                }
                            }
                            Ok(freed)
                        }
                        "images" => {
                            let images = dc.get_images().await.map_err(|e| anyhow::anyhow!(e))?;
                            let mut freed = 0u64;
                            for img in images {
                                if img.dangling && dc.remove_image(&img.id, false).await.is_ok() {
                                    freed += img.size;
                                }
                            }
                            Ok(freed)
                        }
                        "volumes" => {
                            let volumes = dc.get_volumes().await.map_err(|e| anyhow::anyhow!(e))?;
                            let mut freed = 0u64;
                            for vol in volumes {
                                if dc.remove_volume(&vol.name, false).await.is_ok() {
                                    // Approximate freed size could be calculated here
                                    freed += 1;
                                }
                            }
                            Ok(freed)
                        }
                        "networks" => {
                            let networks =
                                dc.get_networks().await.map_err(|e| anyhow::anyhow!(e))?;
                            let mut freed = 0u64;
                            for net in networks {
                                if net.name == "bridge" || net.name == "host" || net.name == "none"
                                {
                                    continue;
                                }
                                if dc.remove_network(&net.name).await.is_ok() {
                                    freed += 1;
                                }
                            }
                            Ok(freed)
                        }
                        _ => Err(anyhow::anyhow!("Unknown resource type")),
                    };
                    Ok(BackgroundResult::DockerPrune(
                        space_freed.map_err(|e| e.to_string()),
                    ))
                }));
            }
        }
    }

    /// Start a single background task that prunes all Docker resource types in sequence.
    /// This avoids the "only containers pruned" bug that occurs when calling
    /// `start_docker_prune_task` four times in a row (subsequent calls are no-ops).
    pub fn start_docker_prune_all_task(&mut self) {
        if self.is_operation_running() {
            return;
        }
        if let Some(rt) = self.runtime {
            if let Some(ref docker_client) = self.docker_client.clone() {
                self.set_operation_running(true);
                self.background_task_description = Some("Prune all Docker resources".to_string());
                let dc = docker_client.clone();
                self.background_handle = Some(rt.spawn(async move {
                    let mut total_freed = 0u64;

                    // Containers — stopped only
                    if let Ok(containers) = dc.get_containers().await {
                        for c in containers {
                            if matches!(c.status, winsweep_core::docker::ContainerStatus::Exited)
                                && dc.remove_container(&c.id, false).await.is_ok()
                            {
                                total_freed +=
                                    c.size_rw.unwrap_or(0) + c.size_root_fs.unwrap_or(0);
                            }
                        }
                    }

                    // Images — dangling only
                    if let Ok(images) = dc.get_images().await {
                        for img in images {
                            if img.dangling && dc.remove_image(&img.id, false).await.is_ok() {
                                total_freed += img.size;
                            }
                        }
                    }

                    // Volumes — all unused
                    if let Ok(volumes) = dc.get_volumes().await {
                        for vol in volumes {
                            if dc.remove_volume(&vol.name, false).await.is_ok() {
                                total_freed += vol.size.unwrap_or(0);
                            }
                        }
                    }

                    // Networks — skip built-in bridge/host/none
                    if let Ok(networks) = dc.get_networks().await {
                        for net in networks {
                            if net.name == "bridge"
                                || net.name == "host"
                                || net.name == "none"
                            {
                                continue;
                            }
                            let _ = dc.remove_network(&net.name).await;
                        }
                    }

                    Ok(BackgroundResult::DockerPrune(Ok(total_freed)))
                }));
            }
        }
    }

    /// Start an async package manager refresh
    pub fn start_package_manager_refresh_task(&mut self) {
        if self.is_operation_running() {
            return;
        }
        if let Some(rt) = self.runtime {
            self.set_operation_running(true);
            self.background_task_description = Some("Refresh package managers".to_string());
            self.background_handle = Some(rt.spawn(async move {
                let registry = PackageManagerRegistry::new().await;
                let mut vm = package_managers::PackageManagersViewModel::new();
                let result = vm.refresh_managers(&registry).await;
                match result {
                    Ok(managers) => Ok(BackgroundResult::PackageManagerRefresh(Ok(managers))),
                    Err(e) => Ok(BackgroundResult::PackageManagerRefresh(Err(e.to_string()))),
                }
            }));
        }
    }

    /// Start cleaning a single package manager cache
    pub fn start_package_manager_clean_task(&mut self, index: usize) {
        if self.is_operation_running() {
            return;
        }
        if let Some(rt) = self.runtime {
            self.set_operation_running(true);
            self.background_task_description = Some("Clean package manager cache".to_string());
            self.background_handle = Some(rt.spawn(async move {
                let registry = PackageManagerRegistry::new().await;
                let mut vm = package_managers::PackageManagersViewModel::new();
                vm.selected_manager = Some(index);
                let result = vm.clean_selected(&registry).await;
                match result {
                    Ok(clean_result) => Ok(BackgroundResult::PackageManagerClean(Ok(clean_result))),
                    Err(e) => Ok(BackgroundResult::PackageManagerClean(Err(e.to_string()))),
                }
            }));
        }
    }

    /// Start cleaning all package manager caches
    pub fn start_package_manager_clean_all_task(&mut self) {
        if self.is_operation_running() {
            return;
        }
        if let Some(rt) = self.runtime {
            self.set_operation_running(true);
            self.background_task_description = Some("Clean all package manager caches".to_string());
            self.background_handle = Some(rt.spawn(async move {
                let registry = PackageManagerRegistry::new().await;
                let mut vm = package_managers::PackageManagersViewModel::new();
                let result = vm.clean_all(&registry).await;
                match result {
                    Ok(results) => {
                        // Aggregate the results into a single PackageCleanResult for display
                        let total_freed: u64 = results.iter().map(|r| r.space_freed).sum();
                        let total_errors: Vec<String> =
                            results.iter().flat_map(|r| r.errors.clone()).collect();
                        let agg = winsweep_core::PackageCleanResult {
                            package_manager: "All".to_string(),
                            space_freed: total_freed,
                            items_deleted: results.iter().map(|r| r.items_deleted).sum(),
                            errors: total_errors,
                            duration_ms: results.iter().map(|r| r.duration_ms).sum(),
                        };
                        Ok(BackgroundResult::PackageManagerClean(Ok(agg)))
                    }
                    Err(e) => Ok(BackgroundResult::PackageManagerClean(Err(e.to_string()))),
                }
            }));
        }
    }

    /// Start cleaning browser caches (Chrome, Edge, Firefox) only
    pub fn start_browser_cache_clean_task(&mut self) {
        if self.is_operation_running() {
            return;
        }
        if let Some(rt) = self.runtime {
            self.set_operation_running(true);
            self.background_task_description = Some("Clean browser caches".to_string());
            self.background_handle = Some(rt.spawn(async move {
                let registry = PackageManagerRegistry::new().await;
                let browser_names = ["chrome", "edge", "firefox"];
                let mut total_freed = 0u64;
                let mut total_items = 0u64;
                let mut all_errors = Vec::new();
                let mut total_ms = 0u64;
                for name in &browser_names {
                    if let Some(mgr) = registry.get_by_name(name) {
                        if mgr.is_installed().await {
                            match mgr.clean_all_caches().await {
                                Ok(r) => {
                                    total_freed += r.space_freed;
                                    total_items += r.items_deleted;
                                    all_errors.extend(r.errors);
                                    total_ms += r.duration_ms;
                                }
                                Err(e) => all_errors.push(format!("{}: {}", name, e)),
                            }
                        }
                    }
                }
                let agg = winsweep_core::PackageCleanResult {
                    package_manager: "Browsers".to_string(),
                    space_freed: total_freed,
                    items_deleted: total_items,
                    errors: all_errors,
                    duration_ms: total_ms,
                };
                Ok(BackgroundResult::PackageManagerClean(Ok(agg)))
            }));
        }
    }

    /// Start compacting a WSL distribution's VHDX via elevated coordinator
    pub fn start_wsl_compact_task(&mut self, distribution_name: String) {
        self.start_elevated_task(
            crate::elevated_coordinator::ElevatedOperation::CompactWslVhdx {
                distribution_name: Some(distribution_name.clone()),
            },
            format!("Compact WSL distribution {}", distribution_name),
        );
    }

    /// Start Windows Update cleanup using the current cleanup options
    pub fn start_windows_update_cleanup(&mut self) {
        let options = self.windows_update.cleanup_options.clone();
        self.windows_update.cleanup_in_progress = true;
        self.windows_update.cleanup_progress = 0.0;
        self.windows_update.status_message = Some("Cleaning Windows Update cache...".to_string());

        if let (Some(rt), Some(coord)) = (self.runtime, self.elevated_coordinator.clone()) {
            self.set_operation_running(true);
            self.background_task_description = Some("Clean Windows Update cache".to_string());
            self.background_handle = Some(rt.spawn(async move {
                let dism = tokio::process::Command::new("dism")
                    .args(["/Online", "/Cleanup-Image", "/StartComponentCleanup"])
                    .output()
                    .await;
                let dism_msg = match dism {
                    Ok(o) if o.status.success() => {
                        Ok(String::from_utf8_lossy(&o.stdout).to_string())
                    }
                    Ok(o) => Err(String::from_utf8_lossy(&o.stderr).to_string()),
                    Err(e) => Err(e.to_string()),
                };

                let elevated = coord
                    .execute_operation(
                        crate::elevated_coordinator::ElevatedOperation::CleanWindowsUpdate {
                            remove_downloads: options.remove_downloads,
                            compress_backups: options.compress_backups,
                            remove_old_versions: options.remove_old_versions,
                        },
                        |_progress| {},
                    )
                    .await;

                let space_freed = elevated.as_ref().map(|e| e.space_freed).unwrap_or(0);

                let msg = format!(
                    "DISM: {}. Space freed: {} bytes.",
                    match &dism_msg {
                        Ok(m) => m.trim(),
                        Err(e) => e.as_str(),
                    },
                    space_freed
                );

                if dism_msg.is_err() && elevated.is_err() {
                    Ok(BackgroundResult::WindowsUpdateCleanup(Err(msg)))
                } else {
                    Ok(BackgroundResult::WindowsUpdateCleanup(Ok(msg)))
                }
            }));
        }
    }

    /// Start a background service refresh task
    pub fn start_service_refresh_task(&mut self) {
        if self.is_operation_running() {
            return;
        }
        if let Some(rt) = self.runtime {
            self.set_operation_running(true);
            self.background_task_description = Some("Refresh services".to_string());
            self.background_handle = Some(rt.spawn(async move {
                match ServiceManager::new() {
                    Ok(sm) => {
                        let mut vm = services::ServicesViewModel::new();
                        match vm.refresh_services(&sm) {
                            Ok(_) => Ok(BackgroundResult::ServiceAction(Ok("refresh".to_string()))),
                            Err(e) => Ok(BackgroundResult::ServiceAction(Err(e.to_string()))),
                        }
                    }
                    Err(e) => Ok(BackgroundResult::ServiceAction(Err(e.to_string()))),
                }
            }));
        }
    }

    /// Start a background task to start a service
    pub fn start_service_task(&mut self, service_name: String) {
        self.start_elevated_task(
            crate::elevated_coordinator::ElevatedOperation::ManageService {
                service_name: service_name.clone(),
                action: crate::elevated_coordinator::ServiceAction::Start,
            },
            format!("Start service {}", service_name),
        );
    }

    /// Start a background task to stop a service
    pub fn stop_service_task(&mut self, service_name: String) {
        self.start_elevated_task(
            crate::elevated_coordinator::ElevatedOperation::ManageService {
                service_name: service_name.clone(),
                action: crate::elevated_coordinator::ServiceAction::Stop,
            },
            format!("Stop service {}", service_name),
        );
    }

    /// Empty the Recycle Bin
    pub fn empty_recycle_bin(&mut self) {
        crate::util::empty_recycle_bin();
        self.dashboard.quick_stats.recycle_bin_size = crate::util::recycle_bin_size();
    }

    /// Start a background task to restart a service
    pub fn restart_service_task(&mut self, service_name: String) {
        self.start_elevated_task(
            crate::elevated_coordinator::ElevatedOperation::ManageService {
                service_name: service_name.clone(),
                action: crate::elevated_coordinator::ServiceAction::Restart,
            },
            format!("Restart service {}", service_name),
        );
    }
}

fn default_instant() -> Instant {
    Instant::now()
}

/// Returns true if no auto-cleanup has been run, or if the elapsed time since the
/// last recorded run exceeds the configured threshold.
fn should_auto_clean(last: &Option<String>, threshold: std::time::Duration) -> bool {
    let Some(ref ts) = *last else { return true };
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
        let elapsed = chrono::Utc::now()
            .signed_duration_since(dt)
            .to_std()
            .unwrap_or(std::time::Duration::MAX);
        elapsed >= threshold
    } else {
        true
    }
}

// Re-export sub-modules
pub use docker::*;

#[cfg(test)]
mod tests {
    use super::should_auto_clean;
    use std::time::Duration;

    #[test]
    fn test_should_auto_clean_none() {
        // No previous run → always clean
        assert!(should_auto_clean(&None, Duration::from_secs(86400)));
    }

    #[test]
    fn test_should_auto_clean_recent() {
        // A timestamp from right now → threshold not yet exceeded
        let now = chrono::Utc::now().to_rfc3339();
        assert!(!should_auto_clean(&Some(now), Duration::from_secs(86400)));
    }

    #[test]
    fn test_should_auto_clean_old() {
        // A timestamp from 2 days ago → threshold (1 day) exceeded
        let old = (chrono::Utc::now() - chrono::Duration::days(2)).to_rfc3339();
        assert!(should_auto_clean(&Some(old), Duration::from_secs(86400)));
    }

    #[test]
    fn test_should_auto_clean_invalid_ts() {
        // Corrupt timestamp → treat as clean needed
        assert!(should_auto_clean(
            &Some("not-a-date".to_string()),
            Duration::from_secs(86400)
        ));
    }
}
