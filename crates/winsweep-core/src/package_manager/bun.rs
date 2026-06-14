//! Bun JavaScript runtime / package manager cache cleanup

use super::{
    calculate_directory_size, safe_delete_directory, CacheInfo, PackageCleanResult, PackageManager,
};
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tracing::{debug, info, warn};
use which::which;

/// Bun package manager
#[derive(Default)]
pub struct BunManager;

impl BunManager {
    pub async fn new() -> Result<Self> {
        Ok(Self)
    }

    fn cache_dir() -> PathBuf {
        // BUN_INSTALL env var overrides default install path
        if let Ok(install) = std::env::var("BUN_INSTALL") {
            let p = PathBuf::from(install).join("install").join("cache");
            if p.exists() {
                return p;
            }
        }
        // Windows: %USERPROFILE%\.bun
        dirs::home_dir()
            .unwrap_or_default()
            .join(".bun")
            .join("install")
            .join("cache")
    }
}

#[async_trait]
impl PackageManager for BunManager {
    fn name(&self) -> &'static str {
        "bun"
    }
    fn display_name(&self) -> &'static str {
        "Bun"
    }

    async fn is_installed(&self) -> bool {
        which("bun").is_ok() || which("bun.exe").is_ok()
    }

    async fn get_version(&self) -> Result<Option<String>> {
        if let Ok(output) = tokio::process::Command::new("bun")
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

        info!("Cleaning Bun install cache");
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
                description: "Bun install cache".to_string(),
                can_delete: true,
            });
        }

        Ok(info)
    }

    fn prevention_tip(&self) -> &'static str {
        "Bun's global cache is shared; use 'bun pm cache rm' to prune. Avoid duplicating node_modules across projects."
    }
}
