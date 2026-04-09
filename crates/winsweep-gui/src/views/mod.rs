//! View layer for WinSweep GUI
//! 
//! This module contains the UI views that render the application.

pub mod dashboard;
pub mod scan;
pub mod wsl;
pub mod docker;
pub mod package_managers;
pub mod windows_update;
pub mod services;
pub mod settings;

use eframe::egui;
use crate::viewmodel::WinSweepViewModel;

// Re-export view functions
pub use dashboard::show_dashboard;
pub use scan::show_scan;
pub use wsl::show_wsl;
pub use docker::show_docker;
pub use package_managers::show_package_managers;
pub use windows_update::show_windows_update;
pub use services::show_services;
pub use settings::show_settings;
