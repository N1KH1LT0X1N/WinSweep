//! Package managers view model

use serde::{Deserialize, Serialize};
use winsweep_core::{CacheInfo, PackageCleanResult, PackageManager, PackageManagerRegistry};

/// Package managers view model
#[derive(Serialize, Deserialize)]
pub struct PackageManagersViewModel {
    /// Package managers
    pub managers: Vec<PackageManagerInfo>,
    /// Selected manager
    pub selected_manager: Option<usize>,
    /// Operation in progress
    pub operation_in_progress: bool,
    /// Operation progress (0.0 to 1.0)
    pub operation_progress: f32,
    /// Status message
    pub status_message: Option<String>,
}

/// Package manager information
#[derive(Serialize, Deserialize)]
pub struct PackageManagerInfo {
    pub name: String,
    pub display_name: String,
    pub version: Option<String>,
    pub cache_size: u64,
    pub installed: bool,
    pub cache_paths: Vec<String>,
}

impl PackageManagersViewModel {
    /// Create a new package managers view model
    pub fn new() -> Self {
        Self {
            managers: Vec::new(),
            selected_manager: None,
            operation_in_progress: false,
            operation_progress: 0.0,
            status_message: None,
        }
    }

    /// Update the package managers view model
    pub fn update(&mut self) {
        // TODO: Update manager status
    }

    /// Refresh package managers
    pub async fn refresh_managers(
        &mut self,
        registry: &PackageManagerRegistry,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.managers.clear();

        // TODO: Get managers from registry and update info

        self.status_message = Some("Package managers refreshed".to_string());
        Ok(())
    }

    /// Clean selected manager cache
    pub async fn clean_selected(
        &mut self,
        registry: &PackageManagerRegistry,
    ) -> Result<PackageCleanResult, Box<dyn std::error::Error>> {
        if let Some(index) = self.selected_manager {
            if index < self.managers.len() {
                self.operation_in_progress = true;
                self.operation_progress = 0.0;

                let manager_name = &self.managers[index].name;
                self.status_message = Some(format!(
                    "Cleaning {} cache...",
                    self.managers[index].display_name
                ));

                // TODO: Implement actual cleanup

                self.operation_in_progress = false;
                self.operation_progress = 0.0;

                // Return dummy result for now
                Ok(PackageCleanResult {
                    package_manager: manager_name.clone(),
                    space_freed: 0,
                    items_deleted: 0,
                    errors: vec![],
                    duration_ms: 0,
                })
            }
        }

        Err("No package manager selected".into())
    }

    /// Clean all manager caches
    pub async fn clean_all(
        &mut self,
        registry: &PackageManagerRegistry,
    ) -> Result<Vec<PackageCleanResult>, Box<dyn std::error::Error>> {
        self.operation_in_progress = true;
        self.operation_progress = 0.0;
        self.status_message = Some("Cleaning all package manager caches...".to_string());

        let mut results = Vec::new();

        // TODO: Implement cleanup for all managers

        self.operation_in_progress = false;
        self.operation_progress = 0.0;

        Ok(results)
    }
}
