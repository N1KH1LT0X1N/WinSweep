//! Git LFS cache reporter and cleanup manager
//!
//! Reports the size of the Git LFS object cache and offers `git lfs prune`
//! to remove objects not referenced by any local commit.

use super::{calculate_directory_size, CacheInfo, PackageCleanResult, PackageManager};
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tracing::{debug, info, warn};
use which::which;

/// Git LFS cache manager
#[derive(Default)]
pub struct GitLfsManager;

impl GitLfsManager {
    pub async fn new() -> Result<Self> {
        Ok(Self)
    }

    fn lfs_cache_dir() -> PathBuf {
        // git lfs stores objects at $(git lfs env | grep LocalWorkingDir)/lfs/objects
        // For the global cache it's inside the git object store.
        // The easiest portable path is: `git lfs env` → LFS_OBJECTS_DIR
        // Fallback: %APPDATA%\Git\lfs\objects
        if let Ok(appdata) = std::env::var("APPDATA") {
            let p = PathBuf::from(appdata)
                .join("Git")
                .join("lfs")
                .join("objects");
            if p.exists() {
                return p;
            }
        }
        dirs::home_dir()
            .unwrap_or_default()
            .join(".git")
            .join("lfs")
            .join("objects")
    }
}

#[async_trait]
impl PackageManager for GitLfsManager {
    fn name(&self) -> &'static str {
        "git_lfs"
    }
    fn display_name(&self) -> &'static str {
        "Git LFS"
    }

    async fn is_installed(&self) -> bool {
        which("git-lfs").is_ok()
            || which("git-lfs.exe").is_ok()
            || (which("git").is_ok() && {
                tokio::process::Command::new("git")
                    .args(["lfs", "version"])
                    .output()
                    .await
                    .map(|o| o.status.success())
                    .unwrap_or(false)
            })
    }

    async fn get_version(&self) -> Result<Option<String>> {
        if let Ok(output) = tokio::process::Command::new("git")
            .args(["lfs", "version"])
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
        // Try to resolve via `git lfs env`
        if let Ok(output) = tokio::process::Command::new("git")
            .args(["lfs", "env"])
            .output()
            .await
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                for line in text.lines() {
                    if line.starts_with("LocalWorkingDir") || line.starts_with("LFS_OBJECT_STORE") {
                        if let Some((_, path_str)) = line.split_once('=') {
                            let path = PathBuf::from(path_str.trim()).join("objects");
                            if path.exists() {
                                return Ok(vec![path]);
                            }
                        }
                    }
                }
            }
        }

        let fallback = Self::lfs_cache_dir();
        if fallback.exists() {
            Ok(vec![fallback])
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

        info!("Pruning Git LFS objects via `git lfs prune`");

        // Measure size before prune
        let size_before = self.calculate_cache_size().await.unwrap_or(0);

        match tokio::process::Command::new("git")
            .args(["lfs", "prune"])
            .output()
            .await
        {
            Ok(result) if result.status.success() => {
                debug!("git lfs prune succeeded");
                let size_after = self.calculate_cache_size().await.unwrap_or(0);
                space_freed = size_before.saturating_sub(size_after);
                items_deleted = 1;
            }
            Ok(result) => {
                let msg = format!(
                    "git lfs prune failed: {}",
                    String::from_utf8_lossy(&result.stderr)
                );
                warn!("{}", msg);
                errors.push(msg);
            }
            Err(e) => {
                let msg = format!("Failed to run git lfs prune: {}", e);
                warn!("{}", msg);
                errors.push(msg);
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

    async fn clean_paths(&self, _paths: &[PathBuf]) -> Result<PackageCleanResult> {
        // For Git LFS, always defer to `git lfs prune` — direct deletion is unsafe
        self.clean_all_caches().await
    }

    async fn get_cache_info(&self) -> Result<Vec<CacheInfo>> {
        let mut info = Vec::new();

        for path in self.get_cache_paths().await? {
            let size = calculate_directory_size(&path).await.unwrap_or(0);
            info.push(CacheInfo {
                path,
                size_bytes: size,
                description: "Git LFS object store (use `git lfs prune` to clean)".to_string(),
                can_delete: false, // must go through `git lfs prune`
            });
        }

        Ok(info)
    }

    fn prevention_tip(&self) -> &'static str {
        "Run 'git lfs prune' to remove unreferenced objects. Set lfs.fetchrecentrefsdays to a lower value."
    }
}
