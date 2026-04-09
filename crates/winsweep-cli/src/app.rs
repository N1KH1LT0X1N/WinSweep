//! WinSweep CLI Application State and UI
//!
//! This module contains the main application state and UI rendering logic
//! for the WinSweep command-line interface.

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Clear, Gauge, List, ListItem, ListState, Padding, Paragraph, Tabs, Wrap,
    },
    Frame, Terminal,
};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, error, info};
use winsweep_common::Config;
use winsweep_core::{
    ContainerInfo, DockerClient, HomeEditionCompat, ImageInfo, NetworkInfo, PackageManagerRegistry,
    VolumeInfo, WindowsEditionDetector, WslDetector, WslState, WslVersion,
};

/// Format bytes to human readable format
fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    const THRESHOLD: f64 = 1024.0;

    if bytes == 0 {
        return "0 B".to_string();
    }

    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= THRESHOLD && unit_index < UNITS.len() - 1 {
        size /= THRESHOLD;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.2} {}", size, UNITS[unit_index])
    }
}

/// Main application state
pub struct App {
    /// Current UI mode/page
    pub mode: Mode,
    /// Should the application quit
    pub should_quit: bool,
    /// Windows edition detector
    pub windows_detector: Option<WindowsEditionDetector>,
    /// WSL detector
    pub wsl_detector: Option<WslDetector>,
    /// Home edition compatibility
    pub home_edition_compat: Option<HomeEditionCompat>,
    /// Docker client
    pub docker_client: Option<DockerClient>,
    /// Tokio runtime for async operations
    pub runtime: tokio::runtime::Runtime,
    /// Configuration
    pub config: Config,
    /// UI state
    pub ui_state: UiState,
    /// Event receiver
    pub event_rx: Option<mpsc::UnboundedReceiver<Event>>,
    /// Background task sender
    pub task_tx: Option<mpsc::UnboundedSender<TaskMessage>>,
    /// Last update time
    pub last_update: Instant,
}

/// UI modes/pages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Main,
    Scan,
    Wsl,
    Docker,
    WindowsUpdate,
    Services,
    PackageManagers,
    Config,
    About,
}

/// UI state for different pages
#[derive(Default)]
pub struct UiState {
    /// Main menu state
    pub main_menu: ListState,
    /// Scan page state
    pub scan: ScanState,
    /// WSL page state
    pub wsl: WslState,
    /// Docker page state
    pub docker: DockerState,
    /// Windows Update page state
    pub windows_update: WindowsUpdateState,
    /// Services page state
    pub services: ServicesState,
    /// Package Managers page state
    pub package_managers: PackageManagersState,
    /// Config page state
    pub config: ConfigState,
    /// About page state
    pub about: AboutState,
    /// Progress indicator
    pub progress: ProgressState,
}

/// Scan page state
#[derive(Default)]
pub struct ScanState {
    pub paths: Vec<String>,
    pub selected_path: usize,
    pub include_hidden: bool,
    pub follow_symlinks: bool,
    pub parallel_jobs: usize,
    pub scanning: bool,
    pub scan_results: Vec<String>,
}

/// WSL page state
#[derive(Default)]
pub struct WslState {
    pub distributions: Vec<String>,
    pub selected_dist: usize,
    pub compacting: bool,
    pub compact_progress: f64,
    pub status_message: String,
}

/// Docker page state
#[derive(Default)]
pub struct DockerState {
    pub containers: Vec<ContainerInfo>,
    pub images: Vec<ImageInfo>,
    pub volumes: Vec<VolumeInfo>,
    pub networks: Vec<NetworkInfo>,
    pub selected_container: usize,
    pub selected_image: usize,
    pub selected_volume: usize,
    pub selected_network: usize,
    pub selected_tab: usize,
    pub cleaning: bool,
    pub space_freed: u64,
    pub daemon_running: bool,
    pub status_message: String,
}

/// Windows Update page state
#[derive(Default)]
pub struct WindowsUpdateState {
    pub cleaning: bool,
    pub space_freed: u64,
    pub status_message: String,
}

/// Services page state
#[derive(Default)]
pub struct ServicesState {
    pub services: Vec<ServiceInfo>,
    pub selected_service: usize,
    pub managing: bool,
}

/// Package Managers page state
#[derive(Default)]
pub struct PackageManagersState {
    pub managers: Vec<PackageManagerInfo>,
    pub selected_manager: usize,
    pub cleaning: bool,
    pub total_space_freed: u64,
    pub status_message: String,
}

/// Information about a package manager
#[derive(Debug, Clone)]
pub struct PackageManagerInfo {
    pub name: String,
    pub display_name: String,
    pub version: Option<String>,
    pub cache_size: u64,
    pub installed: bool,
}

/// Config page state
#[derive(Default)]
pub struct ConfigState {
    pub editing: bool,
    pub config_values: HashMap<String, String>,
    pub selected_key: usize,
}

/// About page state
#[derive(Default)]
pub struct AboutState {
    pub scroll_offset: u16,
}

/// Progress state
#[derive(Default)]
pub struct ProgressState {
    pub active: bool,
    pub progress: f64,
    pub message: String,
}

/// Service information
#[derive(Debug, Clone)]
pub struct ServiceInfo {
    pub name: String,
    pub display_name: String,
    pub status: String,
    pub can_stop: bool,
}

/// Background task messages
#[derive(Debug, Clone)]
pub enum TaskMessage {
    ScanProgress { current: u64, total: u64 },
    ScanComplete { items_found: u64 },
    WslCompactProgress { progress: f64 },
    WslCompactComplete { success: bool, space_saved: u64 },
    DockerCleanupProgress { progress: f64 },
    DockerCleanupComplete { space_freed: u64 },
    WindowsUpdateProgress { progress: f64 },
    WindowsUpdateComplete { space_freed: u64 },
    Error { message: String },
}

impl App {
    /// Create a new application instance
    pub fn new() -> Result<Self> {
        let mut app = Self {
            mode: Mode::Main,
            should_quit: false,
            windows_detector: None,
            wsl_detector: None,
            home_edition_compat: None,
            docker_client: None,
            runtime: tokio::runtime::Runtime::new()?,
            config: Config::load().unwrap_or_default(),
            ui_state: UiState::default(),
            event_rx: None,
            task_tx: None,
            last_update: Instant::now(),
        };

        // Initialize UI state
        app.ui_state.main_menu.select(Some(0));

        Ok(app)
    }

    /// Initialize system detectors
    pub async fn initialize(&mut self) -> Result<()> {
        info!("Initializing WinSweep CLI");

        // Detect Windows edition
        self.windows_detector = Some(WindowsEditionDetector::new()?);

        // Detect WSL
        self.wsl_detector = Some(WslDetector::new()?);

        // Initialize Home edition compatibility
        self.home_edition_compat = Some(HomeEditionCompat::new()?);

        // Update UI state with detected information
        self.update_ui_from_detectors();

        Ok(())
    }

    /// Update UI state from detector information
    fn update_ui_from_detectors(&mut self) {
        if let Some(ref wsl_detector) = self.wsl_detector {
            self.ui_state.wsl.distributions =
                wsl_detector.distributions().keys().cloned().collect();
        }
    }

    /// Run the application main loop
    pub async fn run(&mut self) -> Result<()> {
        // Initialize terminal
        let backend = CrosstermBackend::new(std::io::stdout());
        let mut terminal = Terminal::new(backend)?;

        // Setup terminal
        terminal.clear()?;
        crossterm::terminal::enable_raw_mode()?;
        crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::EnterAlternateScreen,
            crossterm::event::EnableMouseCapture
        )?;

        // Initialize system
        self.initialize().await?;

        // Main loop
        loop {
            // Draw UI
            terminal.draw(|f| self.draw(f))?;

            // Handle events
            if crossterm::event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.handle_key_event(key);
                    }
                }
            }

            // Check for background task messages
            if let Some(ref tx) = self.task_tx {
                // In a real implementation, we'd have a receiver here
                // For now, we'll just update the time
                self.last_update = Instant::now();
            }

            // Check if should quit
            if self.should_quit {
                break;
            }
        }

        // Restore terminal
        crossterm::terminal::disable_raw_mode()?;
        crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        Ok(())
    }

    /// Handle keyboard events
    fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) {
        debug!("Key event: {:?}", key.code);

        match self.mode {
            Mode::Main => self.handle_main_menu_key(key),
            Mode::Scan => self.handle_scan_key(key),
            Mode::Wsl => self.handle_wsl_key(key),
            Mode::Docker => self.handle_docker_key(key),
            Mode::WindowsUpdate => self.handle_windows_update_key(key),
            Mode::Services => self.handle_services_key(key),
            Mode::PackageManagers => self.handle_package_managers_key(key),
            Mode::Config => self.handle_config_key(key),
            Mode::About => self.handle_about_key(key),
        }
    }

    /// Handle main menu key events
    fn handle_main_menu_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Up => {
                let selected = self.ui_state.main_menu.selected().unwrap_or(0);
                if selected > 0 {
                    self.ui_state.main_menu.select(Some(selected - 1));
                }
            }
            KeyCode::Down => {
                let selected = self.ui_state.main_menu.selected().unwrap_or(0);
                if selected < 7 {
                    // 8 menu items (0-7)
                    self.ui_state.main_menu.select(Some(selected + 1));
                }
            }
            KeyCode::Enter => {
                if let Some(selected) = self.ui_state.main_menu.selected() {
                    self.mode = match selected {
                        0 => Mode::Scan,
                        1 => Mode::Wsl,
                        2 => Mode::Docker,
                        3 => Mode::WindowsUpdate,
                        4 => Mode::Services,
                        5 => Mode::PackageManagers,
                        6 => Mode::Config,
                        7 => Mode::About,
                        _ => Mode::Main,
                    };
                }
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                self.should_quit = true;
            }
            _ => {}
        }
    }

    /// Handle scan page key events
    fn handle_scan_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Main;
            }
            KeyCode::Char('s') => {
                if !self.ui_state.scan.scanning {
                    self.start_scan();
                }
            }
            _ => {}
        }
    }

    /// Handle WSL page key events
    fn handle_wsl_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Main;
            }
            KeyCode::Up => {
                if self.ui_state.wsl.selected_dist > 0 {
                    self.ui_state.wsl.selected_dist -= 1;
                }
            }
            KeyCode::Down => {
                if self.ui_state.wsl.selected_dist
                    < self.ui_state.wsl.distributions.len().saturating_sub(1)
                {
                    self.ui_state.wsl.selected_dist += 1;
                }
            }
            KeyCode::Char('c') => {
                if !self.ui_state.wsl.distributions.is_empty() && !self.ui_state.wsl.compacting {
                    self.compact_wsl_distribution();
                }
            }
            _ => {}
        }
    }

    /// Handle Docker page key events
    fn handle_docker_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Main;
            }
            KeyCode::Tab => {
                // Switch tabs
                self.ui_state.docker.selected_tab = (self.ui_state.docker.selected_tab + 1) % 4;
            }
            KeyCode::Up => {
                // Navigate in current tab
                match self.ui_state.docker.selected_tab {
                    0 => {
                        if self.ui_state.docker.selected_container > 0 {
                            self.ui_state.docker.selected_container -= 1;
                        }
                    }
                    1 => {
                        if self.ui_state.docker.selected_image > 0 {
                            self.ui_state.docker.selected_image -= 1;
                        }
                    }
                    2 => {
                        if self.ui_state.docker.selected_volume > 0 {
                            self.ui_state.docker.selected_volume -= 1;
                        }
                    }
                    3 => {
                        if self.ui_state.docker.selected_network > 0 {
                            self.ui_state.docker.selected_network -= 1;
                        }
                    }
                    _ => {}
                }
            }
            KeyCode::Down => {
                // Navigate in current tab
                match self.ui_state.docker.selected_tab {
                    0 => {
                        if self.ui_state.docker.selected_container
                            < self.ui_state.docker.containers.len().saturating_sub(1)
                        {
                            self.ui_state.docker.selected_container += 1;
                        }
                    }
                    1 => {
                        if self.ui_state.docker.selected_image
                            < self.ui_state.docker.images.len().saturating_sub(1)
                        {
                            self.ui_state.docker.selected_image += 1;
                        }
                    }
                    2 => {
                        if self.ui_state.docker.selected_volume
                            < self.ui_state.docker.volumes.len().saturating_sub(1)
                        {
                            self.ui_state.docker.selected_volume += 1;
                        }
                    }
                    3 => {
                        if self.ui_state.docker.selected_network
                            < self.ui_state.docker.networks.len().saturating_sub(1)
                        {
                            self.ui_state.docker.selected_network += 1;
                        }
                    }
                    _ => {}
                }
            }
            KeyCode::Char('d') => {
                // Delete selected item
                self.delete_selected_docker_item();
            }
            KeyCode::Char('D') => {
                // Delete all items in current tab
                self.delete_all_docker_items();
            }
            KeyCode::Char('r') => {
                // Refresh Docker resources
                self.refresh_docker_resources();
            }
            _ => {}
        }
    }

    /// Handle Windows Update page key events
    fn handle_windows_update_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Main;
            }
            KeyCode::Char('c') => {
                if !self.ui_state.windows_update.cleaning {
                    self.cleanup_windows_update();
                }
            }
            _ => {}
        }
    }

    /// Handle services page key events
    fn handle_services_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Main;
            }
            KeyCode::Up => {
                if self.ui_state.services.selected_service > 0 {
                    self.ui_state.services.selected_service -= 1;
                }
            }
            KeyCode::Down => {
                if self.ui_state.services.selected_service
                    < self.ui_state.services.services.len().saturating_sub(1)
                {
                    self.ui_state.services.selected_service += 1;
                }
            }
            KeyCode::Char('s') => {
                self.toggle_service();
            }
            _ => {}
        }
    }

    /// Handle package managers page key events
    fn handle_package_managers_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Main;
            }
            KeyCode::Up => {
                if self.ui_state.package_managers.selected_manager > 0 {
                    self.ui_state.package_managers.selected_manager -= 1;
                }
            }
            KeyCode::Down => {
                if self.ui_state.package_managers.selected_manager
                    < self
                        .ui_state
                        .package_managers
                        .managers
                        .len()
                        .saturating_sub(1)
                {
                    self.ui_state.package_managers.selected_manager += 1;
                }
            }
            KeyCode::Char('c') => {
                self.clean_selected_package_manager();
            }
            KeyCode::Char('C') => {
                self.clean_all_package_managers();
            }
            KeyCode::Char('i') => {
                self.show_package_manager_info();
            }
            KeyCode::Char('r') => {
                self.refresh_package_managers();
            }
            _ => {}
        }
    }

    /// Handle config page key events
    fn handle_config_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Main;
                self.ui_state.config.editing = false;
            }
            KeyCode::Enter => {
                if !self.ui_state.config.editing {
                    self.ui_state.config.editing = true;
                } else {
                    self.save_config();
                }
            }
            _ => {}
        }
    }

    /// Handle about page key events
    fn handle_about_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Main;
            }
            KeyCode::Up => {
                self.ui_state.about.scroll_offset =
                    self.ui_state.about.scroll_offset.saturating_sub(1);
            }
            KeyCode::Down => {
                self.ui_state.about.scroll_offset =
                    self.ui_state.about.scroll_offset.saturating_add(1);
            }
            _ => {}
        }
    }

    /// Start a scan operation
    fn start_scan(&mut self) {
        self.ui_state.scan.scanning = true;
        self.ui_state.scan.scan_results.clear();
        self.ui_state.progress.active = true;
        self.ui_state.progress.message = "Scanning...".to_string();

        // In a real implementation, this would spawn a background task
        info!(
            "Starting scan with {} paths",
            self.ui_state.scan.paths.len()
        );
    }

    /// Compact WSL distribution
    fn compact_wsl_distribution(&mut self) {
        if let Some(dist) = self
            .ui_state
            .wsl
            .distributions
            .get(self.ui_state.wsl.selected_dist)
        {
            self.ui_state.wsl.compacting = true;
            self.ui_state.wsl.status_message = format!("Compacting {}...", dist);
            self.ui_state.progress.active = true;
            self.ui_state.progress.message = format!("Compacting WSL: {}", dist);

            info!("Compacting WSL distribution: {}", dist);
        }
    }

    /// Cleanup Docker resources
    fn cleanup_docker(&mut self) {
        self.ui_state.docker.cleaning = true;
        self.ui_state.progress.active = true;
        self.ui_state.progress.message = "Cleaning Docker...".to_string();

        info!("Starting Docker cleanup");
    }

    /// Cleanup Windows Update
    fn cleanup_windows_update(&mut self) {
        self.ui_state.windows_update.cleaning = true;
        self.ui_state.windows_update.status_message =
            "Cleaning Windows Update cache...".to_string();
        self.ui_state.progress.active = true;
        self.ui_state.progress.message = "Cleaning Windows Update...".to_string();

        info!("Starting Windows Update cleanup");
    }

    /// Toggle service state
    fn toggle_service(&mut self) {
        if let Some(service) = self
            .ui_state
            .services
            .services
            .get(self.ui_state.services.selected_service)
        {
            info!("Toggling service: {}", service.name);
            // Implementation would toggle the service
        }
    }

    /// Save configuration
    fn save_config(&mut self) {
        self.ui_state.config.editing = false;
        info!("Saving configuration");
        // Implementation would save the config
    }

    /// Refresh package managers list
    fn refresh_package_managers(&mut self) {
        info!("Refreshing package managers");

        // Clear current list
        self.ui_state.package_managers.managers.clear();

        // Create registry and detect package managers
        let registry = PackageManagerRegistry::new();

        // Add all package managers with their status
        for manager in registry.get_managers() {
            let info = PackageManagerInfo {
                name: manager.name().to_string(),
                display_name: manager.display_name().to_string(),
                version: None,    // Would get version asynchronously
                cache_size: 0,    // Would calculate size asynchronously
                installed: false, // Would check installation asynchronously
            };
            self.ui_state.package_managers.managers.push(info);
        }

        self.ui_state.package_managers.status_message = "Refreshed package managers".to_string();
    }

    /// Clean selected package manager
    fn clean_selected_package_manager(&mut self) {
        if let Some(manager) = self
            .ui_state
            .package_managers
            .managers
            .get(self.ui_state.package_managers.selected_manager)
        {
            if manager.installed {
                info!("Cleaning {} cache", manager.name);
                self.ui_state.package_managers.cleaning = true;
                self.ui_state.package_managers.status_message =
                    format!("Cleaning {}...", manager.display_name);
                // Implementation would clean the cache
            }
        }
    }

    /// Clean all package managers
    fn clean_all_package_managers(&mut self) {
        info!("Cleaning all package manager caches");
        self.ui_state.package_managers.cleaning = true;
        self.ui_state.package_managers.status_message =
            "Cleaning all package managers...".to_string();
        // Implementation would clean all caches
    }

    /// Show package manager info
    fn show_package_manager_info(&mut self) {
        if let Some(manager) = self
            .ui_state
            .package_managers
            .managers
            .get(self.ui_state.package_managers.selected_manager)
        {
            info!("Showing info for {}", manager.name);
            self.ui_state.package_managers.status_message =
                format!("Viewing {} cache details", manager.display_name);
            // Implementation would show detailed cache info
        }
    }

    /// Refresh Docker resources
    fn refresh_docker_resources(&mut self) {
        info!("Refreshing Docker resources");

        // Create Docker client if needed
        if self.docker_client.is_none() {
            self.docker_client = self
                .runtime
                .block_on(async { DockerClient::new().await.ok() });
        }

        if let Some(ref client) = self.docker_client {
            // Update daemon status
            self.ui_state.docker.daemon_running = client.is_daemon_running();

            if self.ui_state.docker.daemon_running {
                // Get containers
                self.ui_state.docker.containers = self
                    .runtime
                    .block_on(async { client.get_containers().await.unwrap_or_default() });

                // Get images
                self.ui_state.docker.images = self
                    .runtime
                    .block_on(async { client.get_images().await.unwrap_or_default() });

                // Get volumes
                self.ui_state.docker.volumes = self
                    .runtime
                    .block_on(async { client.get_volumes().await.unwrap_or_default() });

                // Get networks
                self.ui_state.docker.networks = self
                    .runtime
                    .block_on(async { client.get_networks().await.unwrap_or_default() });
            } else {
                // Clear all resources if daemon is not running
                self.ui_state.docker.containers.clear();
                self.ui_state.docker.images.clear();
                self.ui_state.docker.volumes.clear();
                self.ui_state.docker.networks.clear();
            }

            self.ui_state.docker.status_message = "Docker resources refreshed".to_string();
        } else {
            self.ui_state.docker.daemon_running = false;
            self.ui_state.docker.status_message = "Docker not found".to_string();
        }
    }

    /// Delete selected Docker item
    fn delete_selected_docker_item(&mut self) {
        if let Some(ref client) = self.docker_client {
            match self.ui_state.docker.selected_tab {
                0 => {
                    // Containers
                    if let Some(container) = self
                        .ui_state
                        .docker
                        .containers
                        .get(self.ui_state.docker.selected_container)
                    {
                        info!("Removing container: {}", container.name);

                        // Stop if running
                        if matches!(
                            container.status,
                            winsweep_core::docker::ContainerStatus::Running
                        ) {
                            if let Err(e) =
                                self.runtime.block_on(client.stop_container(&container.id))
                            {
                                self.ui_state.docker.status_message =
                                    format!("Failed to stop container: {}", e);
                                return;
                            }
                        }

                        // Remove container
                        if let Err(e) = self
                            .runtime
                            .block_on(client.remove_container(&container.id, false))
                        {
                            self.ui_state.docker.status_message =
                                format!("Failed to remove container: {}", e);
                        } else {
                            self.ui_state.docker.status_message =
                                format!("Removed container: {}", container.name);
                            // Refresh list
                            self.refresh_docker_resources();
                        }
                    }
                }
                1 => {
                    // Images
                    if let Some(image) = self
                        .ui_state
                        .docker
                        .images
                        .get(self.ui_state.docker.selected_image)
                    {
                        info!("Removing image: {}", image.repository);

                        if let Err(e) = self.runtime.block_on(client.remove_image(&image.id, false))
                        {
                            self.ui_state.docker.status_message =
                                format!("Failed to remove image: {}", e);
                        } else {
                            self.ui_state.docker.status_message =
                                format!("Removed image: {}", image.repository);
                            // Refresh list
                            self.refresh_docker_resources();
                        }
                    }
                }
                2 => {
                    // Volumes
                    if let Some(volume) = self
                        .ui_state
                        .docker
                        .volumes
                        .get(self.ui_state.docker.selected_volume)
                    {
                        info!("Removing volume: {}", volume.name);

                        if let Err(e) = self
                            .runtime
                            .block_on(client.remove_volume(&volume.name, false))
                        {
                            self.ui_state.docker.status_message =
                                format!("Failed to remove volume: {}", e);
                        } else {
                            self.ui_state.docker.status_message =
                                format!("Removed volume: {}", volume.name);
                            // Refresh list
                            self.refresh_docker_resources();
                        }
                    }
                }
                3 => {
                    // Networks
                    if let Some(network) = self
                        .ui_state
                        .docker
                        .networks
                        .get(self.ui_state.docker.selected_network)
                    {
                        // Skip default networks
                        if network.name == "bridge"
                            || network.name == "host"
                            || network.name == "none"
                        {
                            self.ui_state.docker.status_message =
                                "Cannot remove default network".to_string();
                            return;
                        }

                        info!("Removing network: {}", network.name);

                        if let Err(e) = self.runtime.block_on(client.remove_network(&network.name))
                        {
                            self.ui_state.docker.status_message =
                                format!("Failed to remove network: {}", e);
                        } else {
                            self.ui_state.docker.status_message =
                                format!("Removed network: {}", network.name);
                            // Refresh list
                            self.refresh_docker_resources();
                        }
                    }
                }
                _ => {}
            }
        } else {
            self.ui_state.docker.status_message = "Docker client not available".to_string();
        }
    }

    /// Delete all Docker items in current tab
    fn delete_all_docker_items(&mut self) {
        if let Some(ref client) = self.docker_client {
            match self.ui_state.docker.selected_tab {
                0 => {
                    // Containers
                    info!("Removing all containers");
                    self.ui_state.docker.cleaning = true;

                    // Clone the list to avoid modification during iteration
                    let containers = self.ui_state.docker.containers.clone();
                    let mut removed = 0;

                    for container in containers {
                        // Stop if running
                        if matches!(
                            container.status,
                            winsweep_core::docker::ContainerStatus::Running
                        ) {
                            let _ = self.runtime.block_on(client.stop_container(&container.id));
                        }

                        // Remove container
                        if self
                            .runtime
                            .block_on(client.remove_container(&container.id, true))
                            .is_ok()
                        {
                            removed += 1;
                        }
                    }

                    self.ui_state.docker.cleaning = false;
                    self.ui_state.docker.status_message = format!("Removed {} containers", removed);
                    self.refresh_docker_resources();
                }
                1 => {
                    // Images
                    info!("Removing all dangling images");
                    self.ui_state.docker.cleaning = true;

                    // Clone the list to avoid modification during iteration
                    let images = self.ui_state.docker.images.clone();
                    let mut removed = 0;

                    for image in images {
                        // Only remove dangling images by default
                        if image.dangling {
                            if self
                                .runtime
                                .block_on(client.remove_image(&image.id, true))
                                .is_ok()
                            {
                                removed += 1;
                            }
                        }
                    }

                    self.ui_state.docker.cleaning = false;
                    self.ui_state.docker.status_message =
                        format!("Removed {} dangling images", removed);
                    self.refresh_docker_resources();
                }
                2 => {
                    // Volumes
                    info!("Removing all unused volumes");
                    self.ui_state.docker.cleaning = true;

                    // Clone the list to avoid modification during iteration
                    let volumes = self.ui_state.docker.volumes.clone();
                    let mut removed = 0;

                    for volume in volumes {
                        if self
                            .runtime
                            .block_on(client.remove_volume(&volume.name, true))
                            .is_ok()
                        {
                            removed += 1;
                        }
                    }

                    self.ui_state.docker.cleaning = false;
                    self.ui_state.docker.status_message = format!("Removed {} volumes", removed);
                    self.refresh_docker_resources();
                }
                3 => {
                    // Networks
                    info!("Removing all custom networks");
                    self.ui_state.docker.cleaning = true;

                    // Clone the list to avoid modification during iteration
                    let networks = self.ui_state.docker.networks.clone();
                    let mut removed = 0;

                    for network in networks {
                        // Skip default networks
                        if network.name == "bridge"
                            || network.name == "host"
                            || network.name == "none"
                        {
                            continue;
                        }

                        if self
                            .runtime
                            .block_on(client.remove_network(&network.name))
                            .is_ok()
                        {
                            removed += 1;
                        }
                    }

                    self.ui_state.docker.cleaning = false;
                    self.ui_state.docker.status_message = format!("Removed {} networks", removed);
                    self.refresh_docker_resources();
                }
                _ => {}
            }
        } else {
            self.ui_state.docker.status_message = "Docker client not available".to_string();
        }
    }

    /// Draw the UI
    pub fn draw(&mut self, f: &mut Frame<CrosstermBackend<std::io::Stdout>>) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(0),    // Main content
                Constraint::Length(3), // Footer/Progress
            ])
            .split(f.area());

        // Draw header
        self.draw_header(f, chunks[0]);

        // Draw main content based on mode
        match self.mode {
            Mode::Main => self.draw_main_menu(f, chunks[1]),
            Mode::Scan => self.draw_scan_page(f, chunks[1]),
            Mode::Wsl => self.draw_wsl_page(f, chunks[1]),
            Mode::Docker => self.draw_docker_page(f, chunks[1]),
            Mode::WindowsUpdate => self.draw_windows_update_page(f, chunks[1]),
            Mode::Services => self.draw_services_page(f, chunks[1]),
            Mode::PackageManagers => self.draw_package_managers_page(f, chunks[1]),
            Mode::Config => self.draw_config_page(f, chunks[1]),
            Mode::About => self.draw_about_page(f, chunks[1]),
        }

        // Draw progress/footer
        self.draw_footer(f, chunks[2]);
    }

    /// Draw the header
    fn draw_header(&self, f: &mut Frame<CrosstermBackend<std::io::Stdout>>, area: Rect) {
        let header_text = vec![Line::from(vec![
            Span::styled(
                "WinSweep",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" - "),
            Span::styled(
                "Windows Disk Cleaning Tool",
                Style::default().fg(Color::Gray),
            ),
        ])];

        let header = Paragraph::new(header_text)
            .block(Block::default().borders(Borders::ALL).title("Header"));

        f.render_widget(header, area);
    }

    /// Draw the main menu
    fn draw_main_menu(&mut self, f: &mut Frame<CrosstermBackend<std::io::Stdout>>, area: Rect) {
        let items = vec![
            ListItem::new("🔍 Scan System"),
            ListItem::new("🐧 WSL Management"),
            ListItem::new("🐳 Docker Cleanup"),
            ListItem::new("🔄 Windows Update"),
            ListItem::new("⚙️  Services"),
            ListItem::new("� Package Managers"),
            ListItem::new("�� Configuration"),
            ListItem::new("ℹ️  About"),
        ];

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Main Menu"))
            .highlight_style(
                Style::default()
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            );

        f.render_stateful_widget(list, area, &mut self.ui_state.main_menu);
    }

    /// Draw the footer with progress information
    fn draw_footer(&self, f: &mut Frame<CrosstermBackend<std::io::Stdout>>, area: Rect) {
        if self.ui_state.progress.active {
            let gauge = Gauge::default()
                .block(Block::default().borders(Borders::ALL))
                .gauge_style(Style::default().fg(Color::Green))
                .percent((self.ui_state.progress.progress * 100.0) as u16)
                .label(&self.ui_state.progress.message);

            f.render_widget(gauge, area);
        } else {
            let footer_text = vec![Line::from(vec![
                Span::styled(
                    "q",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(":quit "),
                Span::styled(
                    "Enter",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(":select "),
                Span::styled(
                    "Esc",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(":back"),
            ])];

            let footer = Paragraph::new(footer_text).block(Block::default().borders(Borders::ALL));

            f.render_widget(footer, area);
        }
    }

    /// Placeholder for other draw methods
    fn draw_scan_page(&self, f: &mut Frame<CrosstermBackend<std::io::Stdout>>, area: Rect) {
        let text = Text::from("Scan page - Under construction");
        let paragraph =
            Paragraph::new(text).block(Block::default().borders(Borders::ALL).title("System Scan"));
        f.render_widget(paragraph, area);
    }

    fn draw_wsl_page(&self, f: &mut Frame<CrosstermBackend<std::io::Stdout>>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Min(0),    // Content
                Constraint::Length(3), // Status
            ])
            .split(area);

        // Title
        let title = Paragraph::new("Windows Subsystem for Linux (WSL) Management")
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::Cyan));
        f.render_widget(title, chunks[0]);

        // Content
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[1]);

        // Left side - Distribution list
        let items: Vec<ListItem> = self
            .ui_state
            .wsl
            .distributions
            .iter()
            .enumerate()
            .map(|(i, dist)| {
                let style = if i == self.ui_state.wsl.selected_dist {
                    Style::default()
                        .bg(Color::Blue)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                // Get distribution info
                let version = if let Some(ref wsl_detector) = self.wsl_detector {
                    wsl_detector
                        .get_distribution(dist)
                        .map(|d| {
                            format!(
                                " (WSL{})",
                                match d.version {
                                    WslVersion::Wsl1 => "1",
                                    WslVersion::Wsl2 => "2",
                                    WslVersion::Unknown => "?",
                                }
                            )
                        })
                        .unwrap_or("")
                } else {
                    String::new()
                };

                let state = if let Some(ref wsl_detector) = self.wsl_detector {
                    wsl_detector
                        .get_distribution(dist)
                        .map(|d| match d.state {
                            WslState::Running => " [Running]",
                            WslState::Stopped => " [Stopped]",
                            _ => " [Unknown]",
                        })
                        .unwrap_or("")
                } else {
                    ""
                };

                let text = format!("{}{}{}", dist, version, state);
                ListItem::new(text).style(style)
            })
            .collect();

        let dist_list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Distributions"),
            )
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));

        let mut list_state = ListState::default();
        list_state.select(Some(self.ui_state.wsl.selected_dist));
        f.render_stateful_widget(dist_list, content_chunks[0], &mut list_state);

        // Right side - Distribution info and actions
        let mut info_text = Vec::new();

        if let Some(dist) = self
            .ui_state
            .wsl
            .distributions
            .get(self.ui_state.wsl.selected_dist)
        {
            info_text.push(Line::from(vec![
                Span::styled("Selected: ", Style::default().fg(Color::Gray)),
                Span::styled(dist, Style::default().add_modifier(Modifier::BOLD)),
            ]));

            if let Some(ref wsl_detector) = self.wsl_detector {
                if let Some(distro_info) = wsl_detector.get_distribution(dist) {
                    info_text.push(Line::from(""));
                    info_text.push(Line::from(vec![
                        Span::styled("Version: ", Style::default().fg(Color::Gray)),
                        Span::styled(
                            match distro_info.version {
                                WslVersion::Wsl1 => "WSL 1",
                                WslVersion::Wsl2 => "WSL 2",
                                WslVersion::Unknown => "Unknown",
                            },
                            Style::default(),
                        ),
                    ]));

                    info_text.push(Line::from(vec![
                        Span::styled("State: ", Style::default().fg(Color::Gray)),
                        Span::styled(
                            match distro_info.state {
                                WslState::Running => "Running",
                                WslState::Stopped => "Stopped",
                                WslState::Installing => "Installing",
                                WslState::Uninstalling => "Uninstalling",
                                WslState::Unknown => "Unknown",
                            },
                            Style::default(),
                        ),
                    ]));

                    if distro_info.version == WslVersion::Wsl2 {
                        if let Ok(size) = wsl_detector.get_vhdx_size(dist) {
                            info_text.push(Line::from(""));
                            info_text.push(Line::from(vec![
                                Span::styled("VHDX Size: ", Style::default().fg(Color::Gray)),
                                Span::styled(
                                    format!("{:.2} MB", size as f64 / 1024.0 / 1024.0),
                                    Style::default(),
                                ),
                            ]));
                        }
                    }
                }
            }

            info_text.push(Line::from(""));
            info_text.push(Line::from("Actions:"));
            info_text.push(Line::from("  [c] Compact VHDX (WSL2 only)"));
            info_text.push(Line::from("  [s] Shutdown"));
            info_text.push(Line::from("  [r] Restart"));
        } else {
            info_text.push(Line::from("No distributions found"));
            info_text.push(Line::from(""));
            info_text.push(Line::from("Install WSL distributions from:"));
            info_text.push(Line::from("  Microsoft Store or"));
            info_text.push(Line::from("  wsl --install"));
        }

        let info = Paragraph::new(Text::from(info_text))
            .block(Block::default().borders(Borders::ALL).title("Information"))
            .wrap(Wrap { trim: true });
        f.render_widget(info, content_chunks[1]);

        // Status bar
        let status_text = if self.ui_state.wsl.compacting {
            format!(
                "Compacting... {:.1}%",
                self.ui_state.wsl.compact_progress * 100.0
            )
        } else if !self.ui_state.wsl.status_message.is_empty() {
            self.ui_state.wsl.status_message.clone()
        } else {
            "Use arrow keys to navigate, [c] to compact, [Esc] to go back".to_string()
        };

        let status = Paragraph::new(status_text).block(Block::default().borders(Borders::ALL));
        f.render_widget(status, chunks[2]);

        // Progress bar if compacting
        if self.ui_state.wsl.compacting {
            let progress_area = Rect {
                x: chunks[2].x + 1,
                y: chunks[2].y + 1,
                width: chunks[2].width - 2,
                height: 1,
            };

            let gauge = Gauge::default()
                .percent((self.ui_state.wsl.compact_progress * 100.0) as u16)
                .gauge_style(Style::default().fg(Color::Green));
            f.render_widget(gauge, progress_area);
        }
    }

    fn draw_docker_page(&mut self, f: &mut Frame<CrosstermBackend<std::io::Stdout>>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Min(0),    // Content
                Constraint::Length(3), // Status
            ])
            .split(area);

        // Title
        let title = Paragraph::new("Docker Cleanup")
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::Cyan));
        f.render_widget(title, chunks[0]);

        // Content with tabs
        let content_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Tabs
                Constraint::Min(0),    // List
            ])
            .split(chunks[1]);

        // Tabs
        let titles = ["Containers", "Images", "Volumes", "Networks"];
        let tabs = Tabs::new(titles.iter().map(|t| *t).collect::<Vec<_>>())
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default())
            .highlight_style(Style::default().fg(Color::Yellow))
            .select(self.ui_state.docker.selected_tab);
        f.render_widget(tabs, content_chunks[0]);

        // List based on selected tab
        let mut items = Vec::new();

        match self.ui_state.docker.selected_tab {
            0 => {
                // Containers
                if self.ui_state.docker.containers.is_empty() {
                    items.push(ListItem::new(if self.ui_state.docker.daemon_running {
                        "No containers found"
                    } else {
                        "Docker daemon is not running"
                    }));
                } else {
                    for (i, container) in self.ui_state.docker.containers.iter().enumerate() {
                        let style = if i == self.ui_state.docker.selected_container {
                            Style::default().bg(Color::Blue)
                        } else {
                            Style::default()
                        };

                        let status_color = match container.status {
                            winsweep_core::docker::ContainerStatus::Running => Color::Green,
                            winsweep_core::docker::ContainerStatus::Exited => Color::Red,
                            _ => Color::Yellow,
                        };

                        let size_text = if let (Some(rw), Some(root)) =
                            (container.size_rw, container.size_root_fs)
                        {
                            format!(" ({}B)", format_bytes(rw + root))
                        } else {
                            String::new()
                        };

                        let text = format!(
                            "{} - {} ({}){}",
                            container.name,
                            container.image,
                            format!("{:?}", container.status),
                            size_text
                        );

                        items.push(ListItem::new(text).style(style).fg(status_color));
                    }
                }
            }
            1 => {
                // Images
                if self.ui_state.docker.images.is_empty() {
                    items.push(ListItem::new(if self.ui_state.docker.daemon_running {
                        "No images found"
                    } else {
                        "Docker daemon is not running"
                    }));
                } else {
                    for (i, image) in self.ui_state.docker.images.iter().enumerate() {
                        let style = if i == self.ui_state.docker.selected_image {
                            Style::default().bg(Color::Blue)
                        } else {
                            Style::default()
                        };

                        let tag = if image.tag == "<none>" {
                            "<dangling>"
                        } else {
                            &image.tag
                        };
                        let text = format!(
                            "{}:{} ({}B)",
                            image.repository,
                            tag,
                            format_bytes(image.size)
                        );

                        items.push(ListItem::new(text).style(style));
                    }
                }
            }
            2 => {
                // Volumes
                if self.ui_state.docker.volumes.is_empty() {
                    items.push(ListItem::new(if self.ui_state.docker.daemon_running {
                        "No volumes found"
                    } else {
                        "Docker daemon is not running"
                    }));
                } else {
                    for (i, volume) in self.ui_state.docker.volumes.iter().enumerate() {
                        let style = if i == self.ui_state.docker.selected_volume {
                            Style::default().bg(Color::Blue)
                        } else {
                            Style::default()
                        };

                        let text = format!("{} ({})", volume.name, volume.driver);
                        items.push(ListItem::new(text).style(style));
                    }
                }
            }
            3 => {
                // Networks
                if self.ui_state.docker.networks.is_empty() {
                    items.push(ListItem::new(if self.ui_state.docker.daemon_running {
                        "No networks found"
                    } else {
                        "Docker daemon is not running"
                    }));
                } else {
                    for (i, network) in self.ui_state.docker.networks.iter().enumerate() {
                        let style = if i == self.ui_state.docker.selected_network {
                            Style::default().bg(Color::Blue)
                        } else {
                            Style::default()
                        };

                        let text = format!("{} ({})", network.name, network.driver);
                        items.push(ListItem::new(text).style(style));
                    }
                }
            }
            _ => {}
        }

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(
                match self.ui_state.docker.selected_tab {
                    0 => "Containers",
                    1 => "Images",
                    2 => "Volumes",
                    3 => "Networks",
                    _ => "Resources",
                },
            ))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));

        let mut list_state = ListState::default();
        match self.ui_state.docker.selected_tab {
            0 => list_state.select(Some(self.ui_state.docker.selected_container)),
            1 => list_state.select(Some(self.ui_state.docker.selected_image)),
            2 => list_state.select(Some(self.ui_state.docker.selected_volume)),
            3 => list_state.select(Some(self.ui_state.docker.selected_network)),
            _ => {}
        }
        f.render_stateful_widget(list, content_chunks[1], &mut list_state);

        // Status bar
        let status_text = if self.ui_state.docker.cleaning {
            format!(
                "Cleaning Docker... Space freed: {}",
                format_bytes(self.ui_state.docker.space_freed)
            )
        } else if !self.ui_state.docker.status_message.is_empty() {
            &self.ui_state.docker.status_message
        } else {
            "Actions: [Tab] Switch tabs, [d] Delete selected, [D] Delete all, [r] Refresh, [Esc] Back"
        };

        let status = Paragraph::new(status_text).block(Block::default().borders(Borders::ALL));
        f.render_widget(status, chunks[2]);
    }

    fn draw_windows_update_page(
        &self,
        f: &mut Frame<CrosstermBackend<std::io::Stdout>>,
        area: Rect,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Min(0),    // Content
                Constraint::Length(3), // Status
            ])
            .split(area);

        // Title
        let title = Paragraph::new("Windows Update Cleanup")
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::Cyan));
        f.render_widget(title, chunks[0]);

        // Content
        let mut info_text = Vec::new();
        info_text.push(Line::from("This will clean up Windows Update cache files:"));
        info_text.push(Line::from(""));
        info_text.push(Line::from("• Downloaded update files"));
        info_text.push(Line::from("• Temporary installation files"));
        info_text.push(Line::from("• Old update backups"));
        info_text.push(Line::from(""));
        info_text.push(Line::from("WARNING: This action is irreversible!"));
        info_text.push(Line::from(
            "You may not be able to uninstall updates after cleanup.",
        ));
        info_text.push(Line::from(""));

        if self.ui_state.windows_update.cleaning {
            info_text.push(Line::from("Cleaning in progress..."));
        } else if self.ui_state.windows_update.space_freed > 0 {
            info_text.push(Line::from(format!(
                "Last cleanup freed: {:.2} MB",
                self.ui_state.windows_update.space_freed as f64 / 1024.0 / 1024.0
            )));
        } else {
            info_text.push(Line::from("Press [c] to start cleanup"));
            info_text.push(Line::from("Press [Esc] to go back"));
        }

        let info = Paragraph::new(Text::from(info_text))
            .block(Block::default().borders(Borders::ALL).title("Information"))
            .wrap(Wrap { trim: true })
            .style(Style::default());
        f.render_widget(info, chunks[1]);

        // Status bar
        let status_text = if self.ui_state.windows_update.cleaning {
            &self.ui_state.windows_update.status_message
        } else {
            "Ready to clean Windows Update cache"
        };

        let status = Paragraph::new(status_text).block(Block::default().borders(Borders::ALL));
        f.render_widget(status, chunks[2]);

        // Progress bar if cleaning
        if self.ui_state.windows_update.cleaning {
            let progress_area = Rect {
                x: chunks[2].x + 1,
                y: chunks[2].y + 1,
                width: chunks[2].width - 2,
                height: 1,
            };

            let gauge = Gauge::default()
                .percent(50) // Simplified - would track actual progress
                .gauge_style(Style::default().fg(Color::Green));
            f.render_widget(gauge, progress_area);
        }
    }

    fn draw_services_page(&self, f: &mut Frame<CrosstermBackend<std::io::Stdout>>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Min(0),    // Content
                Constraint::Length(3), // Status
            ])
            .split(area);

        // Title
        let title = Paragraph::new("Windows Service Management")
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::Cyan));
        f.render_widget(title, chunks[0]);

        // Service list
        let mut items = Vec::new();

        if self.ui_state.services.services.is_empty() {
            items.push(ListItem::new("Loading services..."));
        } else {
            for (i, service) in self.ui_state.services.services.iter().enumerate() {
                let style = if i == self.ui_state.services.selected_service {
                    Style::default().bg(Color::Blue)
                } else {
                    Style::default()
                };

                let status_color = match service.status.as_str() {
                    "RUNNING" => Color::Green,
                    "STOPPED" => Color::Red,
                    _ => Color::Yellow,
                };

                let text = format!(
                    "{} - {} ({})",
                    service.name, service.display_name, service.status
                );

                let span = if i == self.ui_state.services.selected_service {
                    vec![Span::styled(
                        text,
                        Style::default()
                            .fg(status_color)
                            .add_modifier(Modifier::BOLD),
                    )]
                } else {
                    vec![Span::styled(text, Style::default().fg(status_color))]
                };

                items.push(ListItem::new(Line::from(span)).style(style));
            }
        }

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Services"))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));

        let mut list_state = ListState::default();
        list_state.select(Some(self.ui_state.services.selected_service));
        f.render_stateful_widget(list, chunks[1], &mut list_state);

        // Status bar
        let status_text = if self.ui_state.services.managing {
            "Managing service..."
        } else if let Some(service) = self
            .ui_state
            .services
            .services
            .get(self.ui_state.services.selected_service)
        {
            format!(
                "Actions: [s] Start/Stop, [r] Restart, [Enter] Properties - {}",
                if service.can_stop {
                    "Can stop"
                } else {
                    "Cannot stop"
                }
            )
        } else {
            "Use arrow keys to navigate, [Esc] to go back"
        };

        let status = Paragraph::new(status_text).block(Block::default().borders(Borders::ALL));
        f.render_widget(status, chunks[2]);
    }

    fn draw_package_managers_page(
        &mut self,
        f: &mut Frame<CrosstermBackend<std::io::Stdout>>,
        area: Rect,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Min(0),    // Content
                Constraint::Length(3), // Status
            ])
            .split(area);

        // Title
        let title = Paragraph::new("Package Managers")
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::Cyan));
        f.render_widget(title, chunks[0]);

        // Content
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[1]);

        // Left side - Package manager list
        let items: Vec<ListItem> = self
            .ui_state
            .package_managers
            .managers
            .iter()
            .enumerate()
            .map(|(i, manager)| {
                let style = if i == self.ui_state.package_managers.selected_manager {
                    Style::default().bg(Color::Blue)
                } else {
                    Style::default()
                };

                let status_color = if manager.installed {
                    Color::Green
                } else {
                    Color::Red
                };

                let size_text = if manager.cache_size > 0 {
                    format!(" ({})", format_bytes(manager.cache_size))
                } else {
                    String::new()
                };

                let text = format!(
                    "{} {}{}",
                    if manager.installed { "✓" } else { "✗" },
                    manager.display_name,
                    size_text
                );

                ListItem::new(Line::from(vec![Span::styled(
                    text,
                    Style::default().fg(status_color),
                )]))
                .style(style)
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Installed Package Managers"),
            )
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));

        let mut list_state = ListState::default();
        list_state.select(Some(self.ui_state.package_managers.selected_manager));
        f.render_stateful_widget(list, content_chunks[0], &mut list_state);

        // Right side - Details and actions
        let mut info_text = Vec::new();

        if let Some(manager) = self
            .ui_state
            .package_managers
            .managers
            .get(self.ui_state.package_managers.selected_manager)
        {
            info_text.push(Line::from(vec![
                Span::styled("Selected: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    &manager.display_name,
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]));

            info_text.push(Line::from(""));
            info_text.push(Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    if manager.installed {
                        "Installed"
                    } else {
                        "Not Installed"
                    },
                    Style::default().fg(if manager.installed {
                        Color::Green
                    } else {
                        Color::Red
                    }),
                ),
            ]));

            if let Some(ref version) = manager.version {
                info_text.push(Line::from(vec![
                    Span::styled("Version: ", Style::default().fg(Color::Gray)),
                    Span::styled(version, Style::default()),
                ]));
            }

            if manager.cache_size > 0 {
                info_text.push(Line::from(""));
                info_text.push(Line::from(vec![
                    Span::styled("Cache Size: ", Style::default().fg(Color::Gray)),
                    Span::styled(format_bytes(manager.cache_size), Style::default()),
                ]));
            }

            info_text.push(Line::from(""));
            info_text.push(Line::from("Actions:"));
            if manager.installed {
                info_text.push(Line::from("  [c] Clean cache"));
                info_text.push(Line::from("  [i] View cache info"));
            } else {
                info_text.push(Line::from("  Package manager not installed"));
            }
        } else {
            info_text.push(Line::from("No package managers detected"));
            info_text.push(Line::from(""));
            info_text.push(Line::from("Install package managers to use this feature"));
        }

        // Show total space freed if available
        if self.ui_state.package_managers.total_space_freed > 0 {
            info_text.push(Line::from(""));
            info_text.push(Line::from(vec![
                Span::styled("Last cleanup freed: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    format_bytes(self.ui_state.package_managers.total_space_freed),
                    Style::default().fg(Color::Green),
                ),
            ]));
        }

        let info = Paragraph::new(Text::from(info_text))
            .block(Block::default().borders(Borders::ALL).title("Details"))
            .wrap(Wrap { trim: true });
        f.render_widget(info, content_chunks[1]);

        // Status bar
        let status_text = if self.ui_state.package_managers.cleaning {
            "Cleaning package manager caches..."
        } else if !self.ui_state.package_managers.status_message.is_empty() {
            &self.ui_state.package_managers.status_message
        } else {
            "Actions: [c] Clean selected, [C] Clean all, [i] Info, [r] Refresh, [Esc] Back"
        };

        let status = Paragraph::new(status_text).block(Block::default().borders(Borders::ALL));
        f.render_widget(status, chunks[2]);
    }

    fn draw_config_page(&self, f: &mut Frame<CrosstermBackend<std::io::Stdout>>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Min(0),    // Content
                Constraint::Length(3), // Status
            ])
            .split(area);

        // Title
        let title = Paragraph::new("Configuration")
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::Cyan));
        f.render_widget(title, chunks[0]);

        // Configuration options
        let mut items = Vec::new();

        // Scanner settings
        items.push(ListItem::new(Line::from(vec![Span::styled(
            "Scanner",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Yellow),
        )])));

        items.push(ListItem::new(Line::from(vec![
            Span::styled("  Parallel Jobs: ", Style::default().fg(Color::Gray)),
            Span::styled(
                self.config.scan.parallel_jobs.to_string(),
                Style::default().fg(Color::White),
            ),
        ])));

        items.push(ListItem::new(Line::from(vec![
            Span::styled("  Include Hidden: ", Style::default().fg(Color::Gray)),
            Span::styled(
                if self.config.scan.include_hidden {
                    "Yes"
                } else {
                    "No"
                },
                Style::default().fg(Color::White),
            ),
        ])));

        items.push(ListItem::new(Line::from(vec![
            Span::styled("  Follow Symlinks: ", Style::default().fg(Color::Gray)),
            Span::styled(
                if self.config.scan.follow_symlinks {
                    "Yes"
                } else {
                    "No"
                },
                Style::default().fg(Color::White),
            ),
        ])));

        items.push(ListItem::new(""));

        // Cleanup settings
        items.push(ListItem::new(Line::from(vec![Span::styled(
            "Cleanup",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Yellow),
        )])));

        items.push(ListItem::new(Line::from(vec![
            Span::styled("  Use Recycle Bin: ", Style::default().fg(Color::Gray)),
            Span::styled(
                if self.config.cleanup.use_recycle_bin {
                    "Yes"
                } else {
                    "No"
                },
                Style::default().fg(Color::White),
            ),
        ])));

        items.push(ListItem::new(Line::from(vec![
            Span::styled("  Create Restore Point: ", Style::default().fg(Color::Gray)),
            Span::styled(
                if self.config.cleanup.create_restore_point {
                    "Yes"
                } else {
                    "No"
                },
                Style::default().fg(Color::White),
            ),
        ])));

        items.push(ListItem::new(""));

        // UI settings
        items.push(ListItem::new(Line::from(vec![Span::styled(
            "UI",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Yellow),
        )])));

        items.push(ListItem::new(Line::from(vec![
            Span::styled("  Theme: ", Style::default().fg(Color::Gray)),
            Span::styled(&self.config.ui.theme, Style::default().fg(Color::White)),
        ])));

        // Create list with selection
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Settings"))
            .highlight_style(
                Style::default()
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            );

        let mut list_state = ListState::default();
        list_state.select(Some(self.ui_state.config.selected_key));
        f.render_stateful_widget(list, chunks[1], &mut list_state);

        // Status bar
        let status_text = if self.ui_state.config.editing {
            "Press [Enter] to save, [Esc] to cancel"
        } else {
            "Actions: [Enter] Edit value, [r] Reload config, [s] Save config, [Esc] Back"
        };

        let status = Paragraph::new(status_text).block(Block::default().borders(Borders::ALL));
        f.render_widget(status, chunks[2]);
    }

    fn draw_about_page(&self, f: &mut Frame<CrosstermBackend<std::io::Stdout>>, area: Rect) {
        let text = Text::from(vec![
            Line::from("WinSweep - Windows Disk Cleaning Tool"),
            Line::from(""),
            Line::from("Phase 0.5 Complete"),
            Line::from("Phase 1 In Progress"),
            Line::from(""),
            Line::from("Features:"),
            Line::from("• Parallel file system scanning"),
            Line::from("• WSL2 management and compaction"),
            Line::from("• Docker cleanup"),
            Line::from("• Windows Update cleanup"),
            Line::from("• Service management"),
            Line::from("• Home edition compatibility"),
        ]);

        let paragraph = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title("About"))
            .wrap(Wrap { trim: true });

        f.render_widget(paragraph, area);
    }
}
