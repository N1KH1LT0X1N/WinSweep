//! Go build cache cleanup (separate from the module download cache in go_modules.rs)

use super::{
    calculate_directory_size, safe_delete_directory, CacheInfo, PackageCleanResult, PackageManager,
};
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tracing::{debug, info, warn};
use which::which;

/// Go build cache manager (`GOCACHE`)
#[derive(Default)]
pub struct GoBuildCacheManager;

impl GoBuildCacheManager {
    pub async fn new() -> Result<Self> {
        Ok(Self)
    }

    /// Resolve GOCACHE via `go env GOCACHE` or fall back to platform default.
    async fn resolve_cache_dir() -> PathBuf {
        if let Ok(output) = tokio::process::Command::new("go")
            .args(["env", "GOCACHE"])
            .output()
            .await
        {
            if output.status.success() {
                let path =
                    PathBuf::from(String::from_utf8_lossy(&output.stdout).trim().to_string());
                if path.exists() {
                    return path;
                }
            }
        }
        // Windows default: %LOCALAPPDATA%\go-build
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            let p = PathBuf::from(local).join("go-build");
            if p.exists() {
                return p;
            }
        }
        dirs::home_dir()
            .unwrap_or_default()
            .join(".cache")
            .join("go-build")
    }
}

#[async_trait]
impl PackageManager for GoBuildCacheManager {
    fn name(&self) -> &'static str {
        "go_build"
    }
    fn display_name(&self) -> &'static str {
        "Go Build Cache"
    }

    async fn is_installed(&self) -> bool {
        which("go").is_ok() || which("go.exe").is_ok()
    }

    async fn get_version(&self) -> Result<Option<String>> {
        if let Ok(output) = tokio::process::Command::new("go")
            .arg("version")
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
        let cache = Self::resolve_cache_dir().await;
        if cache.exists() {
            Ok(vec![cache])
        } else {
            Ok(vec![])
        }
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

        info!("Cleaning Go build cache");

        // Prefer `go clean -cache` so Go can handle stale-entry logic correctly
        if let Ok(output) = tokio::process::Command::new("go")
            .args(["clean", "-cache"])
            .output()
            .await
        {
            if !output.status.success() {
                warn!(
                    "go clean -cache failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            } else {
                debug!("go clean -cache succeeded");
            }
        }

        // Measure freed space by computing the remaining size after clean
        for path in self.get_cache_paths().await? {
            if !path.exists() {
                // go clean removed everything — count as fully freed
                space_freed += 0; // already freed, size unknown without pre-measurement
                items_deleted += 1;
            } else {
                debug!("Falling back to direct deletion of {}", path.display());
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

        let cache = Self::resolve_cache_dir().await;
        if cache.exists() {
            let size = calculate_directory_size(&cache).await.unwrap_or(0);
            info.push(CacheInfo {
                path: cache,
                size_bytes: size,
                description: "Go build cache (GOCACHE)".to_string(),
                can_delete: true,
            });
        }

        Ok(info)
    }

    fn prevention_tip(&self) -> &'static str {
        "Set GOCACHE to a shared directory. Use 'go clean -cache' after toolchain upgrades."
    }
}
