//! pip package manager implementation

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

/// pip package manager
pub struct PipManager {
    pip_path: Option<PathBuf>,
    cache_path: Option<PathBuf>,
}

impl PipManager {
    /// Create a new pip manager
    pub async fn new() -> Result<Self> {
        Ok(Self {
            pip_path: None,
            cache_path: None,
        })
    }

    /// Get pip cache path
    async fn get_cache_path(&self) -> Result<PathBuf> {
        if let Some(ref cache) = self.cache_path {
            return Ok(cache.clone());
        }

        // Try pip cache dir command
        if let Some(ref pip_path) = self.pip_path {
            let output = Command::new(pip_path).args(["cache", "dir"]).output();

            match output {
                Ok(result) if result.status.success() => {
                    let path_str = String::from_utf8_lossy(&result.stdout).trim();
                    let path = PathBuf::from(path_str);
                    return Ok(path);
                }
                _ => {
                    debug!("Failed to get pip cache path from command");
                }
            }
        }

        // Fallback to default locations
        let cache_dirs = [
            dirs::cache_dir().map(|d| d.join("pip")),
            dirs::home_dir().map(|d| d.join(".cache").join("pip")),
            dirs::home_dir().map(|d| d.join("AppData").join("Local").join("pip").join("cache")),
        ];

        for cache_dir in cache_dirs {
            if let Some(dir) = cache_dir {
                if dir.exists() {
                    return Ok(dir);
                }
            }
        }

        // Return first valid path as default
        dirs::cache_dir()
            .map(|d| d.join("pip"))
            .context("Could not determine pip cache directory")
    }

    /// Get pip environment paths
    async fn get_env_paths(&self) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();

        // Get pip list to find installed packages
        if let Some(ref pip_path) = self.pip_path {
            let output = Command::new(pip_path)
                .args(["list", "--format=freeze"])
                .output();

            match output {
                Ok(result) if result.status.success() => {
                    // Parse pip list output to find package locations
                    let _list_output = String::from_utf8_lossy(&result.stdout);
                    // In a real implementation, we'd parse this to find package locations
                }
                _ => {}
            }
        }

        // Common pip environment locations
        if let Some(home_dir) = dirs::home_dir() {
            paths.push(home_dir.join("AppData").join("Roaming").join("Python"));
            paths.push(
                home_dir
                    .join("AppData")
                    .join("Local")
                    .join("Programs")
                    .join("Python"),
            );
        }

        // System Python paths
        let python_roots = [
            r"C:\Python*",
            r"C:\Program Files\Python*",
            r"C:\Program Files (x86)\Python*",
        ];

        for pattern in &python_roots {
            if let Ok(paths_iter) = glob::glob(pattern) {
                for path in paths_iter.flatten() {
                    paths.push(path.join("Lib").join("site-packages"));
                }
            }
        }

        Ok(paths)
    }
}

#[async_trait]
impl PackageManager for PipManager {
    fn name(&self) -> &'static str {
        "pip"
    }

    fn display_name(&self) -> &'static str {
        "Python Package Manager (pip)"
    }

    async fn is_installed(&self) -> bool {
        // Check for pip3 first, then pip
        if which("pip3.exe").is_ok() || which("pip3").is_ok() {
            return true;
        }

        if which("pip.exe").is_ok() || which("pip").is_ok() {
            return true;
        }

        // Check python -m pip
        if which("python.exe").is_ok() || which("python3.exe").is_ok() {
            let python_cmd = if which("python3.exe").is_ok() {
                "python3"
            } else {
                "python"
            };

            let output = Command::new(python_cmd)
                .args(["-m", "pip", "--version"])
                .output();

            matches!(output, Ok(result) if result.status.success())
        } else {
            false
        }
    }

    async fn get_version(&self) -> Result<Option<String>> {
        // Try pip --version
        if let Some(ref pip_path) = self.pip_path {
            let output = Command::new(pip_path).arg("--version").output();

            match output {
                Ok(result) if result.status.success() => {
                    let version = String::from_utf8_lossy(&result.stdout).trim().to_string();
                    return Ok(Some(version));
                }
                _ => {}
            }
        }

        // Try python -m pip --version
        let python_cmd = if which("python3.exe").is_ok() {
            "python3"
        } else if which("python.exe").is_ok() {
            "python"
        } else {
            return Ok(None);
        };

        let output = Command::new(python_cmd)
            .args(["-m", "pip", "--version"])
            .output();

        match output {
            Ok(result) if result.status.success() => {
                let version = String::from_utf8_lossy(&result.stdout).trim().to_string();
                Ok(Some(version))
            }
            _ => Ok(None),
        }
    }

    async fn get_cache_paths(&self) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();

        // Main cache directory
        let cache_path = self.get_cache_path().await?;
        paths.push(cache_path);

        // HTTP cache
        if let Some(ref base_cache) = self.cache_path {
            paths.push(base_cache.join("http"));
        }

        // Wheels cache
        if let Some(ref base_cache) = self.cache_path {
            paths.push(base_cache.join("wheels"));
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

        info!("Cleaning pip caches");

        // Use pip cache purge if available
        if let Some(ref pip_path) = self.pip_path {
            debug!("Using pip cache purge command");

            let output = Command::new(pip_path).args(["cache", "purge"]).output();

            match output {
                Ok(result) => {
                    if result.status.success() {
                        debug!("pip cache purge completed successfully");
                    } else {
                        warn!(
                            "pip cache purge failed: {}",
                            String::from_utf8_lossy(&result.stderr)
                        );
                    }
                }
                Err(e) => {
                    warn!("Failed to run pip cache purge: {}", e);
                }
            }
        }

        // Clean cache directories manually
        let paths = self.get_cache_paths().await?;

        for path in paths {
            if path.exists() {
                debug!("Cleaning pip cache directory: {}", path.display());

                match safe_delete_directory(&path).await {
                    Ok(size) => {
                        space_freed += size;
                        items_deleted += 1;
                        debug!(
                            "Deleted pip cache: {} (freed {})",
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
                description: "pip cache directory".to_string(),
                can_delete: true,
            });
        }

        // HTTP cache
        let http_cache = cache_path.join("http");
        if http_cache.exists() {
            let size = calculate_directory_size(&http_cache).await?;
            cache_info.push(CacheInfo {
                path: http_cache,
                size_bytes: size,
                description: "pip HTTP cache".to_string(),
                can_delete: true,
            });
        }

        // Wheels cache
        let wheels_cache = cache_path.join("wheels");
        if wheels_cache.exists() {
            let size = calculate_directory_size(&wheels_cache).await?;
            cache_info.push(CacheInfo {
                path: wheels_cache,
                size_bytes: size,
                description: "pip wheels cache".to_string(),
                can_delete: true,
            });
        }

        Ok(cache_info)
    }
}

impl Default for PipManager {
    fn default() -> Self {
        Self::new()
    }
}
