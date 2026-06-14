//! Playwright browser cache manager
//!
//! Playwright downloads Chromium, Firefox, and WebKit binaries to a shared
//! cache directory.  This module reports its size and offers to remove
//! browser binaries that are no longer referenced by any local project.

use super::{
    calculate_directory_size, safe_delete_directory, CacheInfo, PackageCleanResult, PackageManager,
};
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tracing::{debug, info, warn};
use which::which;

/// Playwright browser cache manager
#[derive(Default)]
pub struct PlaywrightManager;

impl PlaywrightManager {
    pub async fn new() -> Result<Self> {
        Ok(Self)
    }

    /// Resolve the Playwright browsers cache directory.
    ///
    /// `PLAYWRIGHT_BROWSERS_PATH` overrides the default.  Default on Windows is
    /// `%USERPROFILE%\AppData\Local\ms-playwright`.
    fn browsers_cache() -> PathBuf {
        if let Ok(path) = std::env::var("PLAYWRIGHT_BROWSERS_PATH") {
            return PathBuf::from(path);
        }
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            return PathBuf::from(local).join("ms-playwright");
        }
        dirs::home_dir()
            .unwrap_or_default()
            .join("AppData")
            .join("Local")
            .join("ms-playwright")
    }
}

#[async_trait]
impl PackageManager for PlaywrightManager {
    fn name(&self) -> &'static str {
        "playwright"
    }
    fn display_name(&self) -> &'static str {
        "Playwright (browser binaries)"
    }

    async fn is_installed(&self) -> bool {
        // Playwright is a Node package — check both `playwright` CLI and the cache directory
        which("playwright").is_ok() || Self::browsers_cache().exists()
    }

    async fn get_version(&self) -> Result<Option<String>> {
        if let Ok(output) = tokio::process::Command::new("npx")
            .args(["playwright", "--version"])
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
        let cache = Self::browsers_cache();
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

        info!("Cleaning Playwright browser cache");

        // Prefer `playwright uninstall --all` so Playwright can track what it removes
        let _playwright_cmd = if which("playwright").is_ok() {
            "playwright"
        } else {
            "npx playwright" // fallback – may not parse as a command but we try
        };

        let clean_result = tokio::process::Command::new("npx")
            .args(["playwright", "uninstall", "--all"])
            .output()
            .await;

        match clean_result {
            Ok(out) if out.status.success() => {
                debug!("playwright uninstall --all succeeded");
                items_deleted += 1;
            }
            Ok(out) => {
                let msg = format!(
                    "playwright uninstall --all failed: {}",
                    String::from_utf8_lossy(&out.stderr)
                );
                warn!("{} — falling back to directory deletion", msg);
                errors.push(msg);

                // Fallback: delete the whole cache directory
                for path in self.get_cache_paths().await? {
                    match safe_delete_directory(&path).await {
                        Ok(size) => {
                            space_freed += size;
                            items_deleted += 1;
                        }
                        Err(e) => {
                            errors.push(format!("Failed to delete {}: {}", path.display(), e))
                        }
                    }
                }
            }
            Err(_) => {
                // npx not available — direct deletion
                for path in self.get_cache_paths().await? {
                    let _size_before = calculate_directory_size(&path).await.unwrap_or(0);
                    match safe_delete_directory(&path).await {
                        Ok(size) => {
                            space_freed += size;
                            items_deleted += 1;
                        }
                        Err(e) => {
                            errors.push(format!("Failed to delete {}: {}", path.display(), e))
                        }
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

        let cache = Self::browsers_cache();
        if cache.exists() {
            // Report each browser sub-directory separately
            if let Ok(entries) = std::fs::read_dir(&cache) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        let size = calculate_directory_size(&p).await.unwrap_or(0);
                        info.push(CacheInfo {
                            path: p.clone(),
                            size_bytes: size,
                            description: format!(
                                "Playwright browser: {}",
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
        "Use 'npx playwright uninstall --all' for old browsers. Pin browser versions in CI to avoid redundant installs."
    }
}
