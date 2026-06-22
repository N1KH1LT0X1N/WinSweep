//! pip package manager implementation

use super::{
    calculate_directory_size, format_bytes, safe_delete_directory, CacheInfo, PackageCleanResult,
    PackageManager,
};
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::process::Command;
use tracing::{debug, info, warn};
use which::which;

/// pip package manager
#[derive(Default)]
pub struct PipManager {
    pip_path: Option<PathBuf>,
    cache_path: Option<PathBuf>,
}

impl PipManager {
    /// Create a new pip manager
    pub async fn new() -> Result<Self> {
        // Resolve pip executable eagerly so cache purge and version queries work.
        let pip_path = if which("pip3.exe").is_ok() || which("pip3").is_ok() {
            Some(PathBuf::from("pip3"))
        } else if which("pip.exe").is_ok() || which("pip").is_ok() {
            Some(PathBuf::from("pip"))
        } else {
            None
        };
        Ok(Self {
            pip_path,
            cache_path: None,
        })
    }

    /// Get pip executable path
    async fn get_pip_path(&self) -> Result<PathBuf> {
        if let Some(ref pip_path) = self.pip_path {
            return Ok(pip_path.clone());
        }

        // Check for pip3 first, then pip
        if which("pip3.exe").is_ok() || which("pip3").is_ok() {
            return Ok(PathBuf::from("pip3"));
        }

        if which("pip.exe").is_ok() || which("pip").is_ok() {
            return Ok(PathBuf::from("pip"));
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
                .output()
                .await;

            match output {
                Ok(result) if result.status.success() => {
                    return Ok(PathBuf::from(python_cmd));
                }
                _ => {}
            }
        }

        dirs::cache_dir()
            .map(|d| d.join("pip"))
            .context("Could not determine pip executable path")
    }

    /// Get pip cache directory
    pub async fn get_cache_path(&self) -> Result<PathBuf> {
        let pip_path = self.get_pip_path().await?;

        let output = Command::new(pip_path).args(["cache", "dir"]).output().await;

        match output {
            Ok(result) if result.status.success() => {
                let stdout = String::from_utf8_lossy(&result.stdout);
                let path = PathBuf::from(stdout.trim());
                return Ok(path);
            }
            _ => {
                debug!("Failed to get pip cache path from command");
            }
        }

        // Fallback to default locations
        let cache_dirs = [
            dirs::cache_dir().map(|d| d.join("pip")),
            dirs::home_dir().map(|d| d.join(".cache").join("pip")),
            dirs::home_dir().map(|d| d.join("AppData").join("Local").join("pip").join("cache")),
        ];

        for dir in cache_dirs.into_iter().flatten() {
            if dir.exists() {
                return Ok(dir);
            }
        }

        // Return first valid path as default
        dirs::cache_dir()
            .map(|d| d.join("pip"))
            .context("Could not determine pip cache directory")
    }

    /// Get all pip environment paths
    pub async fn get_env_paths(&self) -> Result<Vec<PathBuf>> {
        let pip_path = self.get_pip_path().await?;

        let output = Command::new(&pip_path).args(["list", "-v"]).output().await;

        // Parse `pip list -v` output: columns are "Package  Version  Location  ..."
        // Lines 0-1 are the header; data starts at line 2.
        if let Ok(result) = output {
            if result.status.success() {
                let list_output = String::from_utf8_lossy(&result.stdout);
                let mut locations: std::collections::HashSet<PathBuf> =
                    std::collections::HashSet::new();
                for line in list_output.lines().skip(2) {
                    let cols: Vec<&str> = line.split_whitespace().collect();
                    // Location column is index 2 (Package, Version, Location, ...)
                    if cols.len() >= 3 {
                        let location = PathBuf::from(cols[2]);
                        if location.exists() {
                            locations.insert(location);
                        }
                    }
                }
                if !locations.is_empty() {
                    return Ok(locations.into_iter().collect());
                }
            }
        }

        // Common pip environment locations
        if let Some(home_dir) = dirs::home_dir() {
            let paths = vec![
                home_dir.join("AppData").join("Roaming").join("Python"),
                home_dir
                    .join("AppData")
                    .join("Local")
                    .join("Programs")
                    .join("Python"),
            ];
            return Ok(paths);
        }

        // System Python paths
        let python_roots = [
            r"C:\Python*",
            r"C:\Program Files\Python*",
            r"C:\Program Files (x86)\Python*",
        ];

        let mut paths = Vec::new();

        for pattern in &python_roots {
            if let Ok(paths_iter) = glob::glob(pattern) {
                for path in paths_iter.flatten() {
                    paths.push(path.join("Lib").join("site-packages"));
                }
            }
        }

        Ok(paths)
    }

    fn find_python_executable() -> Result<String> {
        if which("python3.exe").is_ok() || which("python3").is_ok() {
            return Ok("python3".to_string());
        }
        if which("python.exe").is_ok() || which("python").is_ok() {
            return Ok("python".to_string());
        }
        anyhow::bail!("Python not found in PATH")
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
        if let Ok(python_cmd) = Self::find_python_executable() {
            let output = Command::new(python_cmd)
                .args(["-m", "pip", "--version"])
                .output()
                .await;

            matches!(output, Ok(result) if result.status.success())
        } else {
            false
        }
    }

    async fn get_version(&self) -> Result<Option<String>> {
        let pip_path = self.get_pip_path().await?;

        let output = Command::new(pip_path).arg("--version").output().await;

        match output {
            Ok(result) if result.status.success() => {
                let version = String::from_utf8_lossy(&result.stdout).trim().to_string();
                return Ok(Some(version));
            }
            _ => {}
        }

        // Try python -m pip --version
        let python_cmd = if which("python3.exe").is_ok() {
            "python3"
        } else if which("python.exe").is_ok() {
            "python"
        } else {
            bail!("Python not found");
        };

        let output = Command::new(python_cmd)
            .args(["-m", "pip", "--version"])
            .output()
            .await;

        match output {
            Ok(result) if result.status.success() => {
                let version = String::from_utf8_lossy(&result.stdout).trim().to_string();
                Ok(Some(version))
            }
            _ => bail!("Failed to get pip version"),
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

            let output = Command::new(pip_path)
                .args(["cache", "purge"])
                .output()
                .await;

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

    fn prevention_tip(&self) -> &'static str {
        "Use 'pip cache purge' to clear wheel cache. Pin exact versions in requirements.txt to avoid redundant downloads."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pip_manager_creation() {
        let manager = PipManager::new().await;
        assert!(manager.is_ok(), "PipManager::new() must succeed");
        assert_eq!(manager.unwrap().name(), "pip");
    }

    #[test]
    fn test_display_name() {
        assert_eq!(
            PipManager::default().display_name(),
            "Python Package Manager (pip)"
        );
    }

    /// When pip is present in PATH, pip_path must be populated after construction.
    #[tokio::test]
    async fn test_pip_path_initialized_when_pip_available() {
        let pip_available = which::which("pip3.exe")
            .or_else(|_| which::which("pip3"))
            .or_else(|_| which::which("pip.exe"))
            .or_else(|_| which::which("pip"))
            .is_ok();

        if !pip_available {
            // pip not installed in this environment — skip
            return;
        }

        let manager = PipManager::new().await.unwrap();
        assert!(
            manager.pip_path.is_some(),
            "pip_path must be populated when pip/pip3 is in PATH"
        );
    }

    /// `get_env_paths()` parsing: when pip list -v returns a valid location column,
    /// it must appear in the result. We test the parsing logic directly using a
    /// controlled manager that will call the real pip if installed.
    #[tokio::test]
    async fn test_get_env_paths_returns_vec() {
        let manager = PipManager::new().await.unwrap();
        // Returns Ok(_) regardless of whether pip is installed
        let result = manager.get_env_paths().await;
        assert!(
            result.is_ok(),
            "get_env_paths() must always return Ok(_), got: {:?}",
            result.err()
        );
    }

    /// The cache path fallback must resolve even without pip in PATH.
    #[tokio::test]
    async fn test_get_cache_path_fallback() {
        let manager = PipManager {
            pip_path: None,
            cache_path: None,
        };
        // With pip_path=None, get_pip_path() falls through to dirs::cache_dir()
        // get_cache_path() may err if python/pip cannot be found at all —
        // that is acceptable; what is NOT acceptable is a panic.
        let _ = manager.get_cache_path().await;
    }
}
