//! Dashboard view model

use serde::{Deserialize, Serialize};

/// Dashboard view model
#[derive(Serialize, Deserialize)]
pub struct DashboardViewModel {
    /// System information
    pub system_info: SystemInfo,
    /// Quick stats
    pub quick_stats: QuickStats,
    /// Recent operations
    pub recent_operations: Vec<RecentOperation>,
}

/// System information
#[derive(Serialize, Deserialize)]
pub struct SystemInfo {
    pub windows_version: String,
    pub windows_edition: String,
    pub total_disk_space: u64,
    pub free_disk_space: u64,
    pub memory_usage: f32,
}

/// Quick statistics
#[derive(Serialize, Deserialize)]
pub struct QuickStats {
    pub temp_files_size: u64,
    pub recycle_bin_size: u64,
    pub docker_cache_size: u64,
    pub package_cache_size: u64,
}

/// Recent operation
#[derive(Serialize, Deserialize)]
pub struct RecentOperation {
    pub operation: String,
    pub timestamp: String,
    pub space_freed: u64,
    pub success: bool,
}

impl DashboardViewModel {
    /// Create a new dashboard view model
    pub fn new() -> Self {
        Self {
            system_info: SystemInfo {
                windows_version: "Unknown".to_string(),
                windows_edition: "Unknown".to_string(),
                total_disk_space: 0,
                free_disk_space: 0,
                memory_usage: 0.0,
            },
            quick_stats: QuickStats {
                temp_files_size: 0,
                recycle_bin_size: 0,
                docker_cache_size: 0,
                package_cache_size: 0,
            },
            recent_operations: Vec::new(),
        }
    }

    /// Update the dashboard
    pub fn update(&mut self) {
        // TODO: Update system information and stats
    }
}
