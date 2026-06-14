//! Cypress binary cache manager
//!
//! Cypress downloads its test runner binary to a shared cache directory.
//! This module reports the size and offers cleanup via `cypress install` / direct deletion.

use super::{
    calculate_directory_size, safe_delete_directory, CacheInfo, PackageCleanResult, PackageManager,
};
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tracing::{debug, info, warn};
use which::which;

/// Cypress binary cache manager
#[derive(Default)]
pub struct CypressManager;

impl CypressManager {
    pub async fn new() -> Result<Self> {
        Ok(Self)
    }

    /// Resolve the Cypress binary cache directory.
    ///
    /// `CYPRESS_CACHE_FOLDER` overrides the default.  Default on Windows is
    /// `%APPDATA%\Cypress\Cache`.
    fn binary_cache() -> PathBuf {
        if let Ok(path) = std::env::var("CYPRESS_CACHE_FOLDER") {
            return PathBuf::from(path);
        }
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("Cypress").join("Cache");
        }
        dirs::home_dir()
            .unwrap_or_default()
            .join("AppData")
            .join("Roaming")
            .join("Cypress")
            .join("Cache")
    }
}

#[async_trait]
impl PackageManager for CypressManager {
    fn name(&self) -> &'static str {
        "cypress"
    }
    fn display_name(&self) -> &'static str {
        "Cypress (binary cache)"
    }

    async fn is_installed(&self) -> bool {
        which("cypress").is_ok() || Self::binary_cache().exists()
    }

    async fn get_version(&self) -> Result<Option<String>> {
        if let Ok(output) = tokio::process::Command::new("npx")
            .args(["cypress", "--version"])
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
        let cache = Self::binary_cache();
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

        info!("Cleaning Cypress binary cache");

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

        let cache = Self::binary_cache();
        if cache.exists() {
            // List each version sub-directory
            if let Ok(entries) = std::fs::read_dir(&cache) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        let size = calculate_directory_size(&p).await.unwrap_or(0);
                        info.push(CacheInfo {
                            path: p.clone(),
                            size_bytes: size,
                            description: format!(
                                "Cypress binary v{}",
                                p.file_name().unwrap_or_default().to_string_lossy()
                            ),
                            can_delete: true,
                        });
                    }
                }
            }
        }

        Ok(info)
    }

    fn prevention_tip(&self) -> &'static str {
        "Set CYPRESS_CACHE_FOLDER to a shared path. Remove old Cypress versions with 'cypress cache clear'."
    }
}
