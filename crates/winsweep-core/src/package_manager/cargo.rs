//! Cargo package manager implementation

use super::{
    calculate_directory_size, format_bytes, safe_delete_directory, CacheInfo, PackageCleanResult,
    PackageManager,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::process::Command;
use tracing::{debug, info, warn};
use which::which;

/// Cargo package manager
pub struct CargoManager {
    cargo_path: Option<PathBuf>,
    cargo_home: Option<PathBuf>,
}

impl CargoManager {
    /// Create a new cargo manager
    pub async fn new() -> Result<Self> {
        Ok(Self {
            cargo_path: None,
            cargo_home: None,
        })
    }

    /// Get Cargo home directory
    async fn get_cargo_home(&self) -> Result<PathBuf> {
        if let Some(ref home) = self.cargo_home {
            return Ok(home.clone());
        }

        // Check CARGO_HOME environment variable
        if let Ok(cargo_home) = std::env::var("CARGO_HOME") {
            return Ok(PathBuf::from(cargo_home));
        }

        // Default to .cargo in home directory
        let home_dir = dirs::home_dir().context("Could not find home directory")?;
        Ok(home_dir.join(".cargo"))
    }

    /// Get registry cache path
    async fn get_registry_cache_path(&self) -> Result<PathBuf> {
        let cargo_home = self.get_cargo_home().await?;
        Ok(cargo_home.join("registry"))
    }

    /// Get git cache path
    async fn get_git_cache_path(&self) -> Result<PathBuf> {
        let cargo_home = self.get_cargo_home().await?;
        Ok(cargo_home.join("git"))
    }

    /// Get target directories
    async fn get_target_paths(&self) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();

        // Global target directory
        let cargo_home = self.get_cargo_home().await?;
        paths.push(cargo_home.join("target"));

        // Common project target directories
        if let Ok(current_dir) = std::env::current_dir() {
            // Check for target in current and parent directories
            let mut dir = current_dir.clone();
            for _ in 0..5 {
                // Check up to 5 levels up
                let target_path = dir.join("target");
                if target_path.exists() {
                    paths.push(target_path);
                }

                if !dir.pop() {
                    break;
                }
            }
        }

        Ok(paths)
    }
}

#[async_trait]
impl PackageManager for CargoManager {
    fn name(&self) -> &'static str {
        "cargo"
    }

    fn display_name(&self) -> &'static str {
        "Rust Package Manager (Cargo)"
    }

    async fn is_installed(&self) -> bool {
        // Check if cargo is in PATH
        which("cargo.exe").is_ok() || which("cargo").is_ok()
    }

    async fn get_version(&self) -> Result<Option<String>> {
        if let Some(ref cargo_path) = self.cargo_path {
            let output = Command::new(cargo_path).arg("--version").output();

            match output {
                Ok(result) if result.status.success() => {
                    let version = String::from_utf8_lossy(&result.stdout).trim().to_string();
                    Ok(Some(version))
                }
                _ => Ok(None),
            }
        } else {
            // Try to find cargo and get version
            if let Ok(cargo_path) = which("cargo.exe").or_else(|_| which("cargo")) {
                let output = Command::new(cargo_path).arg("--version").output();

                match output {
                    Ok(result) if result.status.success() => {
                        let version = String::from_utf8_lossy(&result.stdout).trim().to_string();
                        Ok(Some(version))
                    }
                    _ => Ok(None),
                }
            } else {
                Ok(None)
            }
        }
    }

    async fn get_cache_paths(&self) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();

        // Registry cache
        paths.push(self.get_registry_cache_path().await?);

        // Git cache
        paths.push(self.get_git_cache_path().await?);

        // Target directories
        paths.extend(self.get_target_paths().await?);

        Ok(paths)
    }

    async fn calculate_cache_size(&self) -> Result<u64> {
        let paths = self.get_cache_paths().await?;
        let mut total_size = 0u64;

        for path in paths {
            if path.exists() {
                total_size += calculate_directory_size(&path).await?;
            }
        }

        Ok(total_size)
    }

    async fn clean_all_caches(&self) -> Result<PackageCleanResult> {
        let start_time = std::time::Instant::now();
        let mut space_freed = 0u64;
        let mut items_deleted = 0u64;
        let mut errors = Vec::new();

        info!("Cleaning Cargo caches");

        // Use cargo clean if in a project
        if let Ok(current_dir) = std::env::current_dir() {
            let cargo_toml = current_dir.join("Cargo.toml");
            if cargo_toml.exists() {
                debug!("Running cargo clean in project");

                if let Some(ref cargo_path) = self.cargo_path {
                    let output = Command::new(cargo_path)
                        .args(["clean"])
                        .current_dir(&current_dir)
                        .output();

                    match output {
                        Ok(result) => {
                            if result.status.success() {
                                debug!("cargo clean completed successfully");
                            } else {
                                warn!(
                                    "cargo clean failed: {}",
                                    String::from_utf8_lossy(&result.stderr)
                                );
                            }
                        }
                        Err(e) => {
                            warn!("Failed to run cargo clean: {}", e);
                        }
                    }
                }
            }
        }

        // Clean cache directories manually
        let paths = self.get_cache_paths().await?;

        for path in paths {
            if path.exists() {
                // Skip target directories that might be in use
                if path.ends_with("target") {
                    debug!("Skipping target directory: {}", path.display());
                    continue;
                }

                debug!("Cleaning Cargo cache directory: {}", path.display());

                match safe_delete_directory(&path).await {
                    Ok(size) => {
                        space_freed += size;
                        items_deleted += 1;
                        debug!(
                            "Deleted Cargo cache: {} (freed {})",
                            path.display(),
                            format_bytes(size)
                        );
                    }
                    Err(e) => {
                        let error = format!("Failed to delete {}: {}", path.display(), e);
                        warn!("{}", error);
                        errors.push(error);
                    }
                }
            }
        }

        Ok(PackageCleanResult {
            package_manager: self.name().to_string(),
            space_freed,
            items_deleted,
            errors,
            duration_ms: start_time.elapsed().as_millis() as u64,
        })
    }

    async fn clean_paths(&self, paths: &[PathBuf]) -> Result<PackageCleanResult> {
        let start_time = std::time::Instant::now();
        let mut space_freed = 0u64;
        let mut items_deleted = 0u64;
        let mut errors = Vec::new();

        for path in paths {
            if path.exists() {
                // Skip target directories
                if path.ends_with("target") {
                    debug!("Skipping target directory: {}", path.display());
                    continue;
                }

                match safe_delete_directory(path).await {
                    Ok(size) => {
                        space_freed += size;
                        items_deleted += 1;
                    }
                    Err(e) => {
                        errors.push(format!("Failed to delete {}: {}", path.display(), e));
                    }
                }
            }
        }

        Ok(PackageCleanResult {
            package_manager: self.name().to_string(),
            space_freed,
            items_deleted,
            errors,
            duration_ms: start_time.elapsed().as_millis() as u64,
        })
    }

    async fn get_cache_info(&self) -> Result<Vec<CacheInfo>> {
        let mut cache_info = Vec::new();

        // Registry cache
        let registry_cache = self.get_registry_cache_path().await?;
        if registry_cache.exists() {
            let size = calculate_directory_size(&registry_cache).await?;
            cache_info.push(CacheInfo {
                path: registry_cache.clone(),
                size_bytes: size,
                description: "Cargo registry cache".to_string(),
                can_delete: true,
            });
        }

        // Git cache
        let git_cache = self.get_git_cache_path().await?;
        if git_cache.exists() {
            let size = calculate_directory_size(&git_cache).await?;
            cache_info.push(CacheInfo {
                path: git_cache.clone(),
                size_bytes: size,
                description: "Cargo git cache".to_string(),
                can_delete: true,
            });
        }

        // Target directories
        for target_path in self.get_target_paths().await? {
            if target_path.exists() {
                let size = calculate_directory_size(&target_path).await?;
                cache_info.push(CacheInfo {
                    path: target_path,
                    size_bytes: size,
                    description: "Cargo build artifacts".to_string(),
                    can_delete: false, // Don't auto-delete target dirs
                });
            }
        }

        Ok(cache_info)
    }
}

impl Default for CargoManager {
    fn default() -> Self {
        Self::new()
    }
}
