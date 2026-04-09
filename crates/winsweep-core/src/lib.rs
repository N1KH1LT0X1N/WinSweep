//! WinSweep Core Library
//! 
//! This crate contains the core scanning and cleanup logic for WinSweep.

pub mod scanner;
pub mod ipc;
pub mod cleanup;
pub mod windows_api;
pub mod junction_detector;
pub mod audit_logger;
pub mod windows_edition;
pub mod wsl_detector;
pub mod home_edition_compat;
pub mod tool_detector;
pub mod service_manager;
pub mod restart_manager;
pub mod package_manager;
pub mod docker;

// Re-export commonly used items
pub use scanner::{Scanner, ScannerHandle};
pub use ipc::{IpcServer, IpcClient};
pub use cleanup::CleanupManager;
pub use windows_api::WindowsApi;
pub use junction_detector::JunctionDetector;
pub use audit_logger::AuditLogger;
pub use windows_edition::{WindowsEditionDetector, WindowsEdition, WindowsFeatures, WindowsCompatibilityReport};
pub use wsl_detector::{WslDetector, WslDistribution, WslVersion, WslState};
pub use home_edition_compat::{HomeEditionCompat, WslCompactResult, WslCompactMethod};
pub use tool_detector::{ToolDetector, ToolInfo};
pub use service_manager::{ServiceManager, ServiceStatus};
pub use restart_manager::{RestartManager, RestartSession};
pub use package_manager::{PackageManager, PackageManagerRegistry, PackageCleanResult, CacheInfo};
pub use docker::{DockerClient, ContainerInfo, ImageInfo, VolumeInfo, NetworkInfo, DockerCleanupResult, CleanupOptions};
