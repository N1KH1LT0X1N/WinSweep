//! Flutter / Dart pub cache cleanup

use super::{
    calculate_directory_size, safe_delete_directory, CacheInfo, PackageCleanResult, PackageManager,
};
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tracing::{debug, info, warn};
use which::which;

/// Flutter / Dart pub package manager
#[derive(Default)]
pub struct FlutterManager;

impl FlutterManager {
    pub async fn new() -> Result<Self> {
        Ok(Self)
    }

    fn pub_cache() -> PathBuf {
        if let Ok(cache) = std::env::var("PUB_CACHE") {
            return PathBuf::from(cache);
        }
        // Windows default
        if let Ok(appdata) = std::env::var("APPDATA") {
            let p = PathBuf::from(appdata).join("Pub").join("Cache");
            if p.exists() {
                return p;
            }
        }
        dirs::home_dir().unwrap_or_default().join(".pub-cache")
    }
}

#[async_trait]
impl PackageManager for FlutterManager {
    fn name(&self) -> &'static str {
        "flutter"
    }
    fn display_name(&self) -> &'static str {
        "Flutter / Dart pub"
    }

    async fn is_installed(&self) -> bool {
        which("flutter").is_ok() || which("dart").is_ok() || Self::pub_cache().exists()
    }

    async fn get_version(&self) -> Result<Option<String>> {
        if let Ok(output) = tokio::process::Command::new("flutter")
            .arg("--version")
            .output()
            .await
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                return Ok(text.lines().next().map(|l| l.trim().to_string()));
            }
        }
        Ok(None)
    }

    async fn get_cache_paths(&self) -> Result<Vec<PathBuf>> {
        let mut paths = vec![Self::pub_cache()];
        // Flutter engine/tool caches
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            let flutter_cache = PathBuf::from(&local).join("flutter");
            if flutter_cache.exists() {
                paths.push(flutter_cache);
            }
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

        info!("Cleaning Flutter/Dart pub caches");
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

        let pub_cache = Self::pub_cache();
        if pub_cache.exists() {
            let size = calculate_directory_size(&pub_cache).await.unwrap_or(0);
            info.push(CacheInfo {
                path: pub_cache,
                size_bytes: size,
                description: "Dart pub package cache".to_string(),
                can_delete: true,
            });
        }

        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            let flutter_cache = PathBuf::from(&local).join("flutter");
            if flutter_cache.exists() {
                let size = calculate_directory_size(&flutter_cache).await.unwrap_or(0);
                info.push(CacheInfo {
                    path: flutter_cache,
                    size_bytes: size,
                    description: "Flutter engine/tool cache".to_string(),
                    can_delete: true,
                });
            }
        }

        Ok(info)
    }

    fn prevention_tip(&self) -> &'static str {
        "Run 'flutter clean' after builds. Use 'pub cache repair' to consolidate duplicate packages."
    }
}
