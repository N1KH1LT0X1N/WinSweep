//! Package Manager trait and implementations
//!
//! This module provides a unified interface for managing different package managers
//! and their cache cleanup operations.

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{debug, info, warn};

/// Package manager trait for unified operations
#[async_trait]
pub trait PackageManager: Send + Sync {
    /// Get the name of the package manager
    fn name(&self) -> &'static str;

    /// Get the display name of the package manager
    fn display_name(&self) -> &'static str;

    /// Check if the package manager is installed
    async fn is_installed(&self) -> bool;

    /// Get the version of the package manager
    async fn get_version(&self) -> Result<Option<String>>;

    /// Get all cache paths for this package manager
    async fn get_cache_paths(&self) -> Result<Vec<PathBuf>>;

    /// Calculate total size of all caches
    async fn calculate_cache_size(&self) -> Result<u64>;

    /// Clean all caches
    async fn clean_all_caches(&self) -> Result<PackageCleanResult>;

    /// Clean specific cache paths
    async fn clean_paths(&self, paths: &[PathBuf]) -> Result<PackageCleanResult>;

    /// Get detailed cache information
    async fn get_cache_info(&self) -> Result<Vec<CacheInfo>>;

    /// Get a prevention tip explaining why this cache grows and how to avoid it
    fn prevention_tip(&self) -> &'static str {
        ""
    }
}

/// Result of a package manager cleanup operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageCleanResult {
    pub package_manager: String,
    pub space_freed: u64,
    pub items_deleted: u64,
    pub errors: Vec<String>,
    pub duration_ms: u64,
}

/// Information about a cache location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheInfo {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub description: String,
    pub can_delete: bool,
}

/// Package manager registry for managing multiple package managers
#[derive(Default)]
pub struct PackageManagerRegistry {
    managers: Vec<Box<dyn PackageManager>>,
}

impl PackageManagerRegistry {
    /// Create a new registry with all supported package managers
    pub async fn new() -> Self {
        let mut managers: Vec<Box<dyn PackageManager>> = Vec::new();

        // Add npm manager
        if let Ok(manager) = crate::package_manager::npm::NpmManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add pnpm manager
        if let Ok(manager) = crate::package_manager::pnpm::PnpmManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add yarn manager
        if let Ok(manager) = crate::package_manager::yarn::YarnManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add pip manager
        if let Ok(manager) = crate::package_manager::pip::PipManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add poetry manager
        if let Ok(manager) = crate::package_manager::poetry::PoetryManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add cargo manager
        if let Ok(manager) = crate::package_manager::cargo::CargoManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add go modules manager
        if let Ok(manager) = crate::package_manager::go_modules::GoModulesManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add nuget manager
        if let Ok(manager) = crate::package_manager::nuget::NugetManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add gradle manager
        if let Ok(manager) = crate::package_manager::gradle::GradleManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add maven manager
        if let Ok(manager) = crate::package_manager::maven::MavenManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add flutter/pub manager
        if let Ok(manager) = crate::package_manager::flutter::FlutterManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add bun manager
        if let Ok(manager) = crate::package_manager::bun::BunManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add pixi manager
        if let Ok(manager) = crate::package_manager::pixi::PixiManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add PHP Composer manager
        if let Ok(manager) = crate::package_manager::composer::ComposerManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add vcpkg manager
        if let Ok(manager) = crate::package_manager::vcpkg::VcpkgManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add Conan manager
        if let Ok(manager) = crate::package_manager::conan::ConanManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add sbt manager
        if let Ok(manager) = crate::package_manager::sbt::SbtManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add Go build cache manager
        if let Ok(manager) = crate::package_manager::go_build::GoBuildCacheManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add Android SDK manager
        if let Ok(manager) = crate::package_manager::android_sdk::AndroidSdkManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add Git LFS manager
        if let Ok(manager) = crate::package_manager::git_lfs::GitLfsManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add Playwright manager
        if let Ok(manager) = crate::package_manager::playwright::PlaywrightManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add Cypress manager
        if let Ok(manager) = crate::package_manager::cypress::CypressManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add browser cache managers (Chrome, Edge, Firefox)
        if let Ok(manager) = crate::package_manager::browser::ChromeManager::new().await {
            managers.push(Box::new(manager));
        }
        if let Ok(manager) = crate::package_manager::browser::EdgeManager::new().await {
            managers.push(Box::new(manager));
        }
        if let Ok(manager) = crate::package_manager::browser::FirefoxManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add Ruby gems manager
        if let Ok(manager) = crate::package_manager::ruby_gems::RubyGemsManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add Bundler manager
        if let Ok(manager) = crate::package_manager::bundler::BundlerManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add uv manager
        if let Ok(manager) = crate::package_manager::uv::UvManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add conda/mamba manager
        if let Ok(manager) = crate::package_manager::conda::CondaManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add VS Code cache manager
        if let Ok(manager) = crate::package_manager::vscode::VsCodeManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add JetBrains cache manager
        if let Ok(manager) = crate::package_manager::jetbrains::JetBrainsManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add Vagrant manager
        if let Ok(manager) = crate::package_manager::vagrant::VagrantManager::new().await {
            managers.push(Box::new(manager));
        }

        // Add Minikube manager
        if let Ok(manager) = crate::package_manager::minikube::MinikubeManager::new().await {
            managers.push(Box::new(manager));
        }

        Self { managers }
    }

    /// Get all package managers
    pub fn get_managers(&self) -> &[Box<dyn PackageManager>] {
        &self.managers
    }

    /// Get only installed package managers
    pub async fn get_installed(&self) -> Result<Vec<&dyn PackageManager>> {
        let mut installed = Vec::new();

        for manager in &self.managers {
            if manager.is_installed().await {
                installed.push(manager.as_ref());
            }
        }

        Ok(installed)
    }

    /// Get a package manager by name
    pub fn get_by_name(&self, name: &str) -> Option<&dyn PackageManager> {
        self.managers
            .iter()
            .find(|m| m.name() == name)
            .map(|m| m.as_ref())
    }

    /// Clean all caches from all installed package managers
    pub async fn clean_all(&self) -> Result<Vec<PackageCleanResult>> {
        let mut results = Vec::new();

        for manager in &self.managers {
            if manager.is_installed().await {
                info!("Cleaning caches for {}", manager.name());
                match manager.clean_all_caches().await {
                    Ok(result) => results.push(result),
                    Err(e) => {
                        warn!("Failed to clean {} caches: {}", manager.name(), e);
                        results.push(PackageCleanResult {
                            package_manager: manager.name().to_string(),
                            space_freed: 0,
                            items_deleted: 0,
                            errors: vec![e.to_string()],
                            duration_ms: 0,
                        });
                    }
                }
            }
        }

        Ok(results)
    }

    /// Get total cache size from all installed package managers
    pub async fn get_total_cache_size(&self) -> Result<u64> {
        let mut total = 0u64;

        for manager in &self.managers {
            if manager.is_installed().await {
                total += manager.calculate_cache_size().await.unwrap_or(0);
            }
        }

        Ok(total)
    }
}

/// Helper function to calculate directory size recursively
pub async fn calculate_directory_size(path: &PathBuf) -> Result<u64> {
    let mut total_size = 0u64;

    if !path.exists() {
        return Ok(0);
    }

    let mut entries = tokio::fs::read_dir(path).await?;

    while let Some(entry) = entries.next_entry().await? {
        let entry_path = entry.path();

        if entry_path.is_dir() {
            total_size += Box::pin(calculate_directory_size(&entry_path)).await?;
        } else {
            total_size += entry.metadata().await?.len();
        }
    }

    Ok(total_size)
}

/// Helper function to safely delete a directory
pub async fn safe_delete_directory(path: &PathBuf) -> Result<u64> {
    if !path.exists() {
        return Ok(0);
    }

    let size = calculate_directory_size(path).await?;

    // Use RestartManager if available to handle file locks
    if let Ok(mut restart_manager) = crate::restart_manager::RestartManager::new() {
        let files = vec![path.clone()];

        // Try to shutdown applications using the files
        if restart_manager.register_files(&files).is_ok() {
            if let Ok(apps) = restart_manager.get_applications() {
                if !apps.is_empty() {
                    debug!("Applications using cache files: {:?}", apps);
                    restart_manager.shutdown_applications()?;

                    // Delete after shutdown
                    tokio::fs::remove_dir_all(path).await?;

                    // Restart applications
                    let _ = restart_manager.restart_applications();
                    return Ok(size);
                }
            }
        }
    }

    // Fallback to direct deletion
    tokio::fs::remove_dir_all(path).await?;
    Ok(size)
}

/// Helper function to format bytes to human readable format
pub fn format_bytes(bytes: u64) -> String {
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

// Re-export package manager implementations
pub mod android_sdk;
pub mod browser;
pub mod bun;
pub mod bundler;
pub mod cargo;
pub mod composer;
pub mod conan;
pub mod conda;
pub mod cypress;
pub mod flutter;
pub mod git_lfs;
pub mod go_build;
pub mod go_modules;
pub mod gradle;
pub mod jetbrains;
pub mod maven;
pub mod minikube;
pub mod npm;
pub mod nuget;
pub mod pip;
pub mod pixi;
pub mod playwright;
pub mod pnpm;
pub mod poetry;
pub mod ruby_gems;
pub mod sbt;
pub mod uv;
pub mod vagrant;
pub mod vcpkg;
pub mod vscode;
pub mod yarn;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_all_package_managers_have_prevention_tip() {
        let registry = PackageManagerRegistry::new().await;
        let managers = registry.get_managers();
        assert!(
            !managers.is_empty(),
            "Registry should contain at least one manager"
        );
        for manager in managers {
            let tip = manager.prevention_tip();
            assert!(
                !tip.is_empty(),
                "{} must have a non-empty prevention_tip",
                manager.name()
            );
        }
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GB");
    }
}
