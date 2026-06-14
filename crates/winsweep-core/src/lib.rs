//! WinSweep Core Library
//!
//! This crate contains the core scanning and cleanup logic for WinSweep.

pub mod audit_logger;
pub mod cleanup;
pub mod docker;
pub mod home_edition_compat;
pub mod ipc;
pub mod junction_detector;
pub mod package_manager;
pub mod restart_manager;
pub mod scanner;
pub mod service_manager;
pub mod tool_detector;
pub mod updater;
pub mod windows_api;
pub mod windows_edition;
pub mod wsl_detector;

// Re-export commonly used items
pub use audit_logger::AuditLogger;
pub use cleanup::CleanupManager;
pub use docker::{
    CleanupOptions, ContainerInfo, DockerCleanupResult, DockerClient, ImageInfo, NetworkInfo,
    VolumeInfo,
};
pub use home_edition_compat::{HomeEditionCompat, WslCompactMethod, WslCompactResult};
pub use ipc::{IpcClient, IpcServer};
pub use junction_detector::JunctionDetector;
pub use package_manager::{CacheInfo, PackageCleanResult, PackageManager, PackageManagerRegistry};
pub use restart_manager::{RestartManager, RestartSession};
pub use scanner::{Scanner, ScannerHandle};
pub use service_manager::{ServiceManager, ServiceState, ServiceStatus};
pub use tool_detector::{ToolDetector, ToolInfo};
pub use windows_api::WindowsApi;
pub use windows_edition::{
    WindowsCompatibilityReport, WindowsEdition, WindowsEditionDetector, WindowsFeatures,
};
pub use wsl_detector::{WslDetector, WslDistribution, WslState, WslVersion};
