//! Package managers view model

use serde::{Deserialize, Serialize};
use winsweep_core::{PackageCleanResult, PackageManagerRegistry};

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
#[derive(Serialize, Deserialize, Clone)]
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
        // No per-frame updates required
    }

    /// Refresh package managers
    pub async fn refresh_managers(
        &mut self,
        registry: &PackageManagerRegistry,
    ) -> Result<Vec<PackageManagerInfo>, Box<dyn std::error::Error>> {
        self.managers.clear();

        for manager in registry.get_managers() {
            if manager.is_installed().await {
                let cache_paths = manager
                    .get_cache_paths()
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|p| p.display().to_string())
                    .collect();
                let cache_size = manager.calculate_cache_size().await.unwrap_or(0);
                self.managers.push(PackageManagerInfo {
                    name: manager.name().to_string(),
                    display_name: manager.display_name().to_string(),
                    version: manager.get_version().await.ok().flatten(),
                    cache_size,
                    installed: true,
                    cache_paths,
                });
            }
        }

        self.status_message = Some("Package managers refreshed".to_string());
        Ok(self.managers.clone())
    }

    /// Clean selected manager cache
    pub async fn clean_selected(
        &mut self,
        registry: &PackageManagerRegistry,
    ) -> Result<PackageCleanResult, Box<dyn std::error::Error>> {
        let Some(index) = self.selected_manager else {
            return Err("No package manager selected".into());
        };
        if index >= self.managers.len() {
            return Err("Invalid package manager index".into());
        }

        self.operation_in_progress = true;
        self.operation_progress = 0.0;

        let manager_name = self.managers[index].name.clone();
        self.status_message = Some(format!(
            "Cleaning {} cache...",
            self.managers[index].display_name
        ));

        let result = if let Some(manager) = registry.get_by_name(&manager_name) {
            manager
                .clean_all_caches()
                .await
                .map_err(|e| e.to_string())?
        } else {
            PackageCleanResult {
                package_manager: manager_name.clone(),
                space_freed: 0,
                items_deleted: 0,
                errors: vec!["Manager not found in registry".to_string()],
                duration_ms: 0,
            }
        };

        self.operation_in_progress = false;
        self.operation_progress = 0.0;
        Ok(result)
    }

    /// Clean all manager caches
    pub async fn clean_all(
        &mut self,
        registry: &PackageManagerRegistry,
    ) -> Result<Vec<PackageCleanResult>, Box<dyn std::error::Error>> {
        self.operation_in_progress = true;
        self.operation_progress = 0.0;
        self.status_message = Some("Cleaning all package manager caches...".to_string());

        let results = registry.clean_all().await.map_err(|e| e.to_string())?;

        self.operation_in_progress = false;
        self.operation_progress = 0.0;

        Ok(results)
    }
}
