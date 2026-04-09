//! Windows Update view model

use serde::{Deserialize, Serialize};

/// Windows Update view model
#[derive(Serialize, Deserialize)]
pub struct WindowsUpdateViewModel {
    /// Update status
    pub update_status: UpdateStatus,
    /// Available updates
    pub available_updates: Vec<UpdateInfo>,
    /// Selected update
    pub selected_update: Option<usize>,
    /// Cleanup in progress
    pub cleanup_in_progress: bool,
    /// Cleanup progress (0.0 to 1.0)
    pub cleanup_progress: f32,
    /// Status message
    pub status_message: Option<String>,
    /// Cleanup options
    pub cleanup_options: CleanupOptions,
}

/// Cleanup options
#[derive(Serialize, Deserialize)]
pub struct CleanupOptions {
    pub remove_downloads: bool,
    pub compress_backups: bool,
    pub remove_old_versions: bool,
}

/// Update status
#[derive(Serialize, Deserialize)]
pub struct UpdateStatus {
    pub last_check: String,
    pub pending_updates: u32,
    pub download_size: u64,
    pub service_running: bool,
}

/// Update information
#[derive(Serialize, Deserialize)]
pub struct UpdateInfo {
    pub id: String,
    pub title: String,
    pub description: String,
    pub size: u64,
    pub category: UpdateCategory,
    pub installed: bool,
}

/// Update category
#[derive(Serialize, Deserialize)]
pub enum UpdateCategory {
    Critical,
    Important,
    Optional,
    Driver,
}

impl WindowsUpdateViewModel {
    /// Create a new Windows Update view model
    pub fn new() -> Self {
        Self {
            update_status: UpdateStatus {
                last_check: "Unknown".to_string(),
                pending_updates: 0,
                download_size: 0,
                service_running: false,
            },
            available_updates: Vec::new(),
            selected_update: None,
            cleanup_in_progress: false,
            cleanup_progress: 0.0,
            status_message: None,
            cleanup_options: CleanupOptions {
                remove_downloads: true,
                compress_backups: false,
                remove_old_versions: false,
            },
        }
    }
    
    /// Update the Windows Update view model
    pub fn update(&mut self) {
        // TODO: Update status and check for new updates
    }
    
    /// Check for updates
    pub fn check_for_updates(&mut self) {
        self.status_message = Some("Checking for updates...".to_string());
        // TODO: Implement update checking
    }
    
    /// Start cleanup
    pub fn start_cleanup(&mut self) {
        self.cleanup_in_progress = true;
        self.cleanup_progress = 0.0;
        self.status_message = Some("Cleaning Windows Update cache...".to_string());
        
        // TODO: Implement cleanup
    }
    
    /// Stop cleanup
    pub fn stop_cleanup(&mut self) {
        self.cleanup_in_progress = false;
        self.cleanup_progress = 0.0;
        self.status_message = Some("Cleanup stopped".to_string());
    }
}
