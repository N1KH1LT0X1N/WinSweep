//! Pixi (conda-based) package manager cache cleanup

use super::{
    calculate_directory_size, safe_delete_directory, CacheInfo, PackageCleanResult, PackageManager,
};
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tracing::{debug, info, warn};
use which::which;

/// Pixi package manager
#[derive(Default)]
pub struct PixiManager;

impl PixiManager {
    pub async fn new() -> Result<Self> {
        Ok(Self)
    }

    fn cache_dir() -> PathBuf {
        // Pixi stores its cache under %LOCALAPPDATA%\rattler\cache on Windows
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            let p = PathBuf::from(local).join("rattler").join("cache");
            if p.exists() {
                return p;
            }
        }
        dirs::home_dir()
            .unwrap_or_default()
            .join(".pixi")
            .join("cache")
    }

    fn pixi_home() -> PathBuf {
        dirs::home_dir().unwrap_or_default().join(".pixi")
    }
}

#[async_trait]
impl PackageManager for PixiManager {
    fn name(&self) -> &'static str {
        "pixi"
    }
    fn display_name(&self) -> &'static str {
        "Pixi (conda/rattler)"
    }

    async fn is_installed(&self) -> bool {
        which("pixi").is_ok() || which("pixi.exe").is_ok()
    }

    async fn get_version(&self) -> Result<Option<String>> {
        if let Ok(output) = tokio::process::Command::new("pixi")
            .arg("--version")
            .output()
            .await
        {
            if output.status.success() {
                return Ok(Some(
                    String::from_utf8_lossy(&output.stdout).trim().to_string(),
                ));
            }
        }
        Ok(None)
    }

    async fn get_cache_paths(&self) -> Result<Vec<PathBuf>> {
        let mut paths = vec![Self::cache_dir()];
        // Also include the pixi home envs directory which may be large
        let envs = Self::pixi_home().join("envs");
        if envs.exists() {
            paths.push(envs);
        }
        paths.retain(|p| p.exists());
        Ok(paths)
    }

    async fn calculate_cache_size(&self) -> Result<u64> {
        let mut total = 0u64;
        for p in self.get_cache_paths().await? {
            total += calculate_directory_size(&p).await.unwrap_or(0);
        }
        Ok(total)
    }

    async fn clean_all_caches(&self) -> Result<PackageCleanResult> {
        let start = std::time::Instant::now();
        let mut space_freed = 0u64;
        let mut items_deleted = 0u64;
        let mut errors = Vec::new();

        info!("Cleaning Pixi/rattler caches");
        for path in self.get_cache_paths().await? {
            debug!("Deleting {}", path.display());
            match safe_delete_directory(&path).await {
                Ok(size) => {
                    space_freed += size;
                    items_deleted += 1;
                }
                Err(e) => {
                    let msg = format!("Failed to delete {}: {}", path.display(), e);
                    warn!("{}", msg);
                    errors.push(msg);
                }
            }
        }

        Ok(PackageCleanResult {
            package_manager: self.name().to_string(),
            space_freed,
            items_deleted,
            errors,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn clean_paths(&self, paths: &[PathBuf]) -> Result<PackageCleanResult> {
        let start = std::time::Instant::now();
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
                    Err(e) => errors.push(format!("Failed to delete {}: {}", path.display(), e)),
                }
            }
        }

        Ok(PackageCleanResult {
            package_manager: self.name().to_string(),
            space_freed,
            items_deleted,
            errors,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn get_cache_info(&self) -> Result<Vec<CacheInfo>> {
        let mut info = Vec::new();

        let cache = Self::cache_dir();
        if cache.exists() {
            let size = calculate_directory_size(&cache).await.unwrap_or(0);
            info.push(CacheInfo {
                path: cache,
                size_bytes: size,
                description: "Pixi/rattler package cache".to_string(),
                can_delete: true,
            });
        }

        let envs = Self::pixi_home().join("envs");
        if envs.exists() {
            let size = calculate_directory_size(&envs).await.unwrap_or(0);
            info.push(CacheInfo {
                path: envs,
                size_bytes: size,
                description: "Pixi project environments".to_string(),
                can_delete: false,
            });
        }

        Ok(info)
    }

    fn prevention_tip(&self) -> &'static str {
        "Use 'pixi clean cache' after updating lockfiles. Share a global pixi home across projects."
    }
}
