//! npm package manager implementation

use super::{
    calculate_directory_size, format_bytes, safe_delete_directory, CacheInfo, PackageCleanResult,
    PackageManager,
};
use anyhow::Context;
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::process::Command;
use tracing::{debug, info, warn};
use which::which;

/// npm package manager
#[derive(Default)]
pub struct NpmManager {
    npm_path: Option<PathBuf>,
    cache_path: Option<PathBuf>,
}

impl NpmManager {
    /// Create a new npm manager
    pub async fn new() -> Result<Self> {
        // Resolve npm executable eagerly so cache clean and version queries work.
        let npm_path = which("npm.cmd").or_else(|_| which("npm")).ok();
        Ok(Self {
            npm_path,
            cache_path: None,
        })
    }

    /// Get npm cache path
    async fn get_cache_path(&self) -> Result<PathBuf> {
        if let Some(ref cache) = self.cache_path {
            return Ok(cache.clone());
        }

        // Try npm config get cache
        if let Some(ref npm_path) = self.npm_path {
            let output = Command::new(npm_path)
                .args(["config", "get", "cache"])
                .output()
                .await;

            match output {
                Ok(result) if result.status.success() => {
                    let stdout = String::from_utf8_lossy(&result.stdout);
                    let path = PathBuf::from(stdout.trim());
                    return Ok(path);
                }
                _ => {
                    warn!("Failed to get npm cache path from config");
                }
            }
        }

        // Fallback to default location
        let home_dir = dirs::home_dir().context("Could not find home directory")?;
        let cache_path = home_dir.join(".npm");

        // Check if npm is using the new cache location
        let new_cache_path = home_dir.join("AppData").join("Local").join("npm-cache");
        if new_cache_path.exists() {
            return Ok(new_cache_path);
        }

        Ok(cache_path)
    }

    /// Get npm global modules path
    async fn get_global_modules_path(&self) -> Result<PathBuf> {
        let home_dir = dirs::home_dir().context("Could not find home directory")?;

        // Try npm config get prefix
        if let Some(ref npm_path) = self.npm_path {
            let output = Command::new(npm_path)
                .args(["config", "get", "prefix"])
                .output()
                .await;

            match output {
                Ok(result) if result.status.success() => {
                    let stdout = String::from_utf8_lossy(&result.stdout);
                    let path = PathBuf::from(stdout.trim());
                    return Ok(path.join("node_modules"));
                }
                _ => {}
            }
        }

        // Fallback to default location
        Ok(home_dir
            .join("AppData")
            .join("Roaming")
            .join("npm")
            .join("node_modules"))
    }
}

#[async_trait]
impl PackageManager for NpmManager {
    fn name(&self) -> &'static str {
        "npm"
    }

    fn display_name(&self) -> &'static str {
        "Node Package Manager (npm)"
    }

    async fn is_installed(&self) -> bool {
        // Check if npm is in PATH
        if which("npm.cmd").is_ok() || which("npm").is_ok() {
            return true;
        }

        // Check common installation locations
        let common_paths = [
            r"C:\Program Files\nodejs\npm.cmd",
            r"C:\Program Files (x86)\nodejs\npm.cmd",
            r"%APPDATA%\npm\npm.cmd",
        ];

        for path in &common_paths {
            let expanded_path = shellexpand::full(path).unwrap_or_default().into_owned();
            if PathBuf::from(&expanded_path).exists() {
                return true;
            }
        }

        false
    }

    async fn get_version(&self) -> Result<Option<String>> {
        if let Some(ref npm_path) = self.npm_path {
            let output = Command::new(npm_path).arg("--version").output().await;

            match output {
                Ok(result) if result.status.success() => {
                    let version = String::from_utf8_lossy(&result.stdout).trim().to_string();
                    Ok(Some(version))
                }
                _ => Ok(None),
            }
        } else {
            // Try to find npm and get version
            if let Ok(npm_path) = which("npm.cmd").or_else(|_| which("npm")) {
                let output = Command::new(npm_path).arg("--version").output().await;

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

        // Main cache directory
        let cache_path = self.get_cache_path().await?;
        paths.push(cache_path);

        // Global modules (can contain cached packages)
        if let Ok(global_path) = self.get_global_modules_path().await {
            paths.push(global_path);
        }

        // npm logs
        if let Some(home_dir) = dirs::home_dir() {
            let npm_log_path = home_dir.join(".npm").join("_logs");
            if npm_log_path.exists() {
                paths.push(npm_log_path);
            }
        }

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

        info!("Cleaning npm caches");

        // Use npm cache clean --force if available
        if let Some(ref npm_path) = self.npm_path {
            debug!("Using npm cache clean command");

            let output = Command::new(npm_path)
                .args(["cache", "clean", "--force"])
                .output()
                .await;

            match output {
                Ok(result) => {
                    if result.status.success() {
                        debug!("npm cache clean completed successfully");
                    } else {
                        warn!(
                            "npm cache clean failed: {}",
                            String::from_utf8_lossy(&result.stderr)
                        );
                    }
                }
                Err(e) => {
                    warn!("Failed to run npm cache clean: {}", e);
                }
            }
        }

        // Clean cache directories manually
        let paths = self.get_cache_paths().await?;

        for path in paths {
            if path.exists() {
                debug!("Cleaning npm cache directory: {}", path.display());

                match safe_delete_directory(&path).await {
                    Ok(size) => {
                        space_freed += size;
                        items_deleted += 1;
                        debug!(
                            "Deleted npm cache: {} (freed {})",
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

        // Main cache
        let cache_path = self.get_cache_path().await?;
        if cache_path.exists() {
            let size = calculate_directory_size(&cache_path).await?;
            cache_info.push(CacheInfo {
                path: cache_path.clone(),
                size_bytes: size,
                description: "npm cache directory".to_string(),
                can_delete: true,
            });
        }

        // Global modules
        if let Ok(global_path) = self.get_global_modules_path().await {
            if global_path.exists() {
                let size = calculate_directory_size(&global_path).await?;
                cache_info.push(CacheInfo {
                    path: global_path,
                    size_bytes: size,
                    description: "npm global modules".to_string(),
                    can_delete: true,
                });
            }
        }

        // Logs
        if let Some(home_dir) = dirs::home_dir() {
            let npm_log_path = home_dir.join(".npm").join("_logs");
            if npm_log_path.exists() {
                let size = calculate_directory_size(&npm_log_path).await?;
                cache_info.push(CacheInfo {
                    path: npm_log_path,
                    size_bytes: size,
                    description: "npm logs".to_string(),
                    can_delete: true,
                });
            }
        }

        Ok(cache_info)
    }

    fn prevention_tip(&self) -> &'static str {
        "Use 'npm config set cache-max 86400000' to limit cache TTL. Periodically run 'npm cache clean --force'."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_npm_manager_creation() {
        let manager = NpmManager::new().await;
        assert!(manager.is_ok(), "NpmManager::new() must succeed");
        assert_eq!(manager.unwrap().name(), "npm");
    }

    #[test]
    fn test_display_name() {
        assert_eq!(
            NpmManager::default().display_name(),
            "Node Package Manager (npm)"
        );
    }

    /// When npm is present in PATH the manager must eagerly resolve its path in
    /// `new()` so that `npm cache clean --force` is actually executed.
    #[tokio::test]
    async fn test_npm_path_initialized_when_npm_available() {
        if which::which("npm.cmd")
            .or_else(|_| which::which("npm"))
            .is_err()
        {
            // npm not installed in this environment — skip
            return;
        }
        let manager = NpmManager::new().await.unwrap();
        assert!(
            manager.npm_path.is_some(),
            "npm_path must be populated when npm is in PATH"
        );
    }

    /// Ensure the fallback cache path logic resolves to some path even without npm.
    #[tokio::test]
    async fn test_get_cache_path_fallback() {
        // Create a manager with npm_path forced to None
        let manager = NpmManager {
            npm_path: None,
            cache_path: None,
        };
        // Should fall back to ~/.npm or AppData\Local\npm-cache without panicking
        let result = manager.get_cache_path().await;
        assert!(result.is_ok(), "get_cache_path fallback must not error");
    }
}
