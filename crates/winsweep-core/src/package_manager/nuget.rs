//! NuGet package manager implementation

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

/// NuGet package manager
#[derive(Default)]
pub struct NugetManager;

impl NugetManager {
    /// Create a new NuGet manager
    pub async fn new() -> Result<Self> {
        Ok(Self)
    }

    /// Get NuGet cache paths
    async fn get_global_cache_paths(&self) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();

        // Get local app data
        let local_app_data =
            dirs::data_local_dir().context("Could not find LocalAppData directory")?;

        // NuGet v3 cache
        let v3_cache = local_app_data.join("NuGet").join("v3-cache");
        if v3_cache.exists() {
            paths.push(v3_cache);
        }

        // NuGet packages folder
        let packages_folder = local_app_data.join("NuGet").join("packages");
        if packages_folder.exists() {
            paths.push(packages_folder);
        }

        // NuGet fallback cache
        let fallback_cache = local_app_data.join("NuGet").join("FallbackCache");
        if fallback_cache.exists() {
            paths.push(fallback_cache);
        }

        // .NET NuGet cache
        let dotnet_cache = local_app_data
            .join("Microsoft")
            .join("dotnet")
            .join("package-cache");
        if dotnet_cache.exists() {
            paths.push(dotnet_cache);
        }

        // NuGet plugin cache
        let plugin_cache = local_app_data.join("NuGet").join("plugins-cache");
        if plugin_cache.exists() {
            paths.push(plugin_cache);
        }

        Ok(paths)
    }

    /// Get project-level package folders
    async fn get_project_package_paths(&self) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();

        // Check for .csproj files in current and parent directories
        if let Ok(current_dir) = std::env::current_dir() {
            let mut dir = current_dir.clone();
            for _ in 0..5 {
                // Check up to 5 levels up
                // Check for packages folder
                let packages_path = dir.join("packages");
                if packages_path.exists() {
                    paths.push(packages_path);
                }

                // Check for obj folder with NuGet artifacts
                let obj_path = dir.join("obj");
                if obj_path.exists() {
                    let nuget_artifacts = obj_path.join("project.nuget.cache");
                    if nuget_artifacts.exists() || obj_path.join("NuGet").exists() {
                        paths.push(obj_path);
                    }
                }

                // Check for .csproj files
                for entry in std::fs::read_dir(&dir)
                    .unwrap_or_else(|_| std::fs::read_dir(".").unwrap())
                    .flatten()
                {
                    let path = entry.path();
                    if path.is_file() {
                        if let Some(filename) = path.file_name() {
                            if let Some(name) = filename.to_str() {
                                if name.ends_with(".csproj")
                                    || name.ends_with(".fsproj")
                                    || name.ends_with(".vbproj")
                                {
                                    // Found a project file
                                    debug!("Found project file: {}", path.display());
                                    break;
                                }
                            }
                        }
                    }
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
impl PackageManager for NugetManager {
    fn name(&self) -> &'static str {
        "nuget"
    }

    fn display_name(&self) -> &'static str {
        ".NET Package Manager (NuGet)"
    }

    async fn is_installed(&self) -> bool {
        // Check for nuget.exe
        if which("nuget.exe").is_ok() || which("nuget").is_ok() {
            return true;
        }

        // Check for dotnet CLI (includes NuGet functionality)
        if which("dotnet.exe").is_ok() || which("dotnet").is_ok() {
            return true;
        }

        // Check common installation locations
        let common_paths = [
            r"C:\Program Files\NuGet\nuget.exe",
            r"C:\Program Files (x86)\NuGet\nuget.exe",
            r"%LOCALAPPDATA%\Microsoft\dotnet\dotnet.exe",
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
        // Try nuget.exe first
        if let Ok(nuget_path) = which("nuget.exe") {
            let output = Command::new(nuget_path).arg("-Version").output().await;

            match output {
                Ok(result) if result.status.success() => {
                    let version = String::from_utf8_lossy(&result.stdout).trim().to_string();
                    return Ok(Some(version));
                }
                _ => {}
            }
        }

        // Try dotnet --version
        if which("dotnet.exe").is_ok() {
            let output = Command::new("dotnet").arg("--version").output().await;

            match output {
                Ok(result) if result.status.success() => {
                    let version = String::from_utf8_lossy(&result.stdout).trim().to_string();
                    return Ok(Some(format!("dotnet {}", version)));
                }
                _ => {}
            }
        }

        Ok(None)
    }

    async fn get_cache_paths(&self) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();

        // Global cache paths
        paths.extend(self.get_global_cache_paths().await?);

        // Project-specific paths
        paths.extend(self.get_project_package_paths().await?);

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

        info!("Cleaning nuget caches");

        // Clean different cache types
        let cache_types = [
            "all",
            "http-cache",
            "global-packages",
            "temp",
            "plugins-cache",
        ];

        for cache_type in &cache_types {
            debug!("Cleaning nuget {} cache", cache_type);

            let output = Command::new("dotnet")
                .args(["nuget", "locals", cache_type, "--clear"])
                .output()
                .await;

            match output {
                Ok(result) => {
                    if result.status.success() {
                        debug!("Cleared NuGet {} cache", cache_type);
                    } else {
                        warn!(
                            "Failed to clear NuGet {} cache: {}",
                            cache_type,
                            String::from_utf8_lossy(&result.stderr)
                        );
                    }
                }
                Err(e) => {
                    warn!("Failed to run dotnet nuget locals: {}", e);
                }
            }
        }

        // Clean cache directories manually
        let paths = self.get_cache_paths().await?;

        for path in paths {
            if path.exists() {
                // Skip project packages folders by default
                if path.ends_with("packages") && path.to_string_lossy().contains("packages") {
                    debug!("Skipping packages folder: {}", path.display());
                    continue;
                }

                debug!("Cleaning NuGet cache directory: {}", path.display());

                match safe_delete_directory(&path).await {
                    Ok(size) => {
                        space_freed += size;
                        items_deleted += 1;
                        debug!(
                            "Deleted NuGet cache: {} (freed {})",
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
                // Skip project packages folders
                if path.ends_with("packages") {
                    debug!("Skipping packages folder: {}", path.display());
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

        // Global cache paths
        for path in self.get_cache_paths().await? {
            if path.exists() {
                let size = calculate_directory_size(&path).await?;
                let description = if path.ends_with("v3-cache") {
                    "NuGet v3 cache".to_string()
                } else if path.ends_with("packages") {
                    "NuGet packages".to_string()
                } else if path.ends_with("FallbackCache") {
                    "NuGet fallback cache".to_string()
                } else if path.to_string_lossy().contains("dotnet") {
                    ".NET package cache".to_string()
                } else if path.ends_with("plugins-cache") {
                    "NuGet plugin cache".to_string()
                } else {
                    "NuGet cache".to_string()
                };

                cache_info.push(CacheInfo {
                    path: path.clone(),
                    size_bytes: size,
                    description,
                    can_delete: !path.ends_with("packages"), // Don't auto-delete packages
                });
            }
        }

        // Project paths
        for path in self.get_project_package_paths().await? {
            if path.exists() {
                let size = calculate_directory_size(&path).await?;
                cache_info.push(CacheInfo {
                    path: path.clone(),
                    size_bytes: size,
                    description: if path.ends_with("packages") {
                        "Project packages".to_string()
                    } else {
                        "Project build artifacts".to_string()
                    },
                    can_delete: false, // Don't auto-delete project artifacts
                });
            }
        }

        Ok(cache_info)
    }

    fn prevention_tip(&self) -> &'static str {
        "Use 'dotnet nuget locals all --clear' periodically. Set globalPackagesFolder to a shared path in NuGet.config."
    }
}
