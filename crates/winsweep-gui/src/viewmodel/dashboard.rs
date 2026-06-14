//! Dashboard view model

use serde::{Deserialize, Serialize};
use sysinfo::{Disks, System};

/// Information about a single drive / volume
#[derive(Serialize, Deserialize, Clone)]
pub struct DriveInfo {
    /// Mount point (e.g. "C:\\")
    pub mount_point: String,
    /// Human-readable volume name (may be empty)
    pub name: String,
    /// File system (e.g. "NTFS")
    pub file_system: String,
    /// Total capacity in bytes
    pub total_bytes: u64,
    /// Available bytes
    pub free_bytes: u64,
    /// Used bytes
    pub used_bytes: u64,
}

/// Dashboard view model
#[derive(Serialize, Deserialize)]
pub struct DashboardViewModel {
    /// System information
    pub system_info: SystemInfo,
    /// Quick stats
    pub quick_stats: QuickStats,
    /// Recent operations
    pub recent_operations: Vec<RecentOperation>,
    /// Per-category reclaimable bytes breakdown
    pub category_breakdown: CategoryBreakdown,
    /// All mounted drives / volumes
    pub drives: Vec<DriveInfo>,
    /// Sysinfo handle (not persisted)
    #[serde(skip, default = "default_system")]
    sys: System,
    /// Tracks when we last polled sysinfo (not persisted)
    #[serde(skip)]
    last_refresh: Option<std::time::Instant>,
    /// Whether we have already notified about low disk space (cleared when space recovers)
    #[serde(skip)]
    pub low_disk_notified: bool,
    /// ISO-8601 timestamp of the last automatic cleanup run
    pub last_auto_cleanup: Option<String>,
}

fn default_system() -> System {
    System::new()
}

/// System information
#[derive(Serialize, Deserialize)]
pub struct SystemInfo {
    pub windows_version: String,
    pub windows_edition: String,
    pub total_disk_space: u64,
    pub free_disk_space: u64,
    /// Memory utilisation 0-100 %
    pub memory_usage: f32,
    /// CPU utilisation 0-100 %
    pub cpu_usage: f32,
}

/// Quick statistics
#[derive(Serialize, Deserialize)]
pub struct QuickStats {
    pub temp_files_size: u64,
    pub recycle_bin_size: u64,
    pub docker_cache_size: u64,
    pub package_cache_size: u64,
}

/// Reclaimable bytes split by category (populated by scan results)
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct CategoryBreakdown {
    pub artifact_bytes: u64,
    pub temp_bytes: u64,
    pub package_cache_bytes: u64,
    pub recycle_bin_bytes: u64,
    pub other_bytes: u64,
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
                windows_version: String::new(),
                windows_edition: String::new(),
                total_disk_space: 0,
                free_disk_space: 0,
                memory_usage: 0.0,
                cpu_usage: 0.0,
            },
            quick_stats: QuickStats {
                temp_files_size: 0,
                recycle_bin_size: 0,
                docker_cache_size: 0,
                package_cache_size: 0,
            },
            recent_operations: Vec::new(),
            category_breakdown: CategoryBreakdown::default(),
            drives: Vec::new(),
            sys: System::new(),
            last_refresh: None,
            low_disk_notified: false,
            last_auto_cleanup: None,
        }
    }

    /// Append a completed operation to the recent-activity log (capped at 50)
    pub fn record_operation(&mut self, operation: String, space_freed: u64, success: bool) {
        let now = chrono::Local::now();
        self.recent_operations.push(RecentOperation {
            operation,
            timestamp: now.format("%Y-%m-%d %H:%M").to_string(),
            space_freed,
            success,
        });
        if self.recent_operations.len() > 50 {
            self.recent_operations.remove(0);
        }
    }

    /// Update the dashboard (called every frame; real I/O is rate-limited to 5 s)
    pub fn update(&mut self) {
        let now = std::time::Instant::now();
        let stale = self
            .last_refresh
            .map(|t| now.duration_since(t) > std::time::Duration::from_secs(5))
            .unwrap_or(true);

        if !stale {
            return;
        }
        self.last_refresh = Some(now);

        // --- memory & CPU ---
        self.sys.refresh_memory();
        self.sys.refresh_cpu_all();

        let total_mem = self.sys.total_memory();
        if total_mem > 0 {
            self.system_info.memory_usage =
                self.sys.used_memory() as f32 / total_mem as f32 * 100.0;
        }
        self.system_info.cpu_usage = self.sys.global_cpu_usage();

        // --- disk (pick the largest / primary volume) ---
        let disks = Disks::new_with_refreshed_list();
        let mut best_total = 0u64;
        let mut best_free = 0u64;
        for disk in disks.iter() {
            if disk.total_space() > best_total {
                best_total = disk.total_space();
                best_free = disk.available_space();
            }
        }
        if best_total > 0 {
            self.system_info.total_disk_space = best_total;
            self.system_info.free_disk_space = best_free;
        }

        // --- enumerate all drives ---
        self.drives = disks
            .iter()
            .map(|d| {
                let total = d.total_space();
                let free = d.available_space();
                DriveInfo {
                    mount_point: d.mount_point().display().to_string(),
                    name: d.name().to_string_lossy().to_string(),
                    file_system: d.file_system().to_string_lossy().to_string(),
                    total_bytes: total,
                    free_bytes: free,
                    used_bytes: total.saturating_sub(free),
                }
            })
            .collect();

        // --- OS version (one-time; cheap after first call) ---
        if self.system_info.windows_version.is_empty() {
            self.system_info.windows_version =
                System::os_version().unwrap_or_else(|| "Unknown".to_string());
            self.system_info.windows_edition =
                System::name().unwrap_or_else(|| "Unknown".to_string());
        }

        // --- Recycle Bin size ---
        self.quick_stats.recycle_bin_size = crate::util::recycle_bin_size();
    }
}
