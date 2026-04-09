//! View layer for WinSweep GUI
//!
//! This module contains the UI views that render the application.

pub mod dashboard;
pub mod docker;
pub mod package_managers;
pub mod scan;
pub mod services;
pub mod settings;
pub mod windows_update;
pub mod wsl;

use crate::viewmodel::WinSweepViewModel;
use eframe::egui;

// Re-export view functions
pub use dashboard::show_dashboard;
pub use docker::show_docker;
pub use package_managers::show_package_managers;
pub use scan::show_scan;
pub use services::show_services;
pub use settings::show_settings;
pub use windows_update::show_windows_update;
pub use wsl::show_wsl;
