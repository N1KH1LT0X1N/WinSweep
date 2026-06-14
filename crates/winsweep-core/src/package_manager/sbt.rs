//! sbt (Scala Build Tool) cache cleanup

use super::{
    calculate_directory_size, safe_delete_directory, CacheInfo, PackageCleanResult, PackageManager,
};
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tracing::{debug, info, warn};
use which::which;

/// sbt Scala build tool manager
#[derive(Default)]
pub struct SbtManager;

impl SbtManager {
    pub async fn new() -> Result<Self> {
        Ok(Self)
    }

    fn sbt_home() -> PathBuf {
        if let Ok(home) = std::env::var("SBT_HOME") {
            return PathBuf::from(home);
        }
        dirs::home_dir().unwrap_or_default().join(".sbt")
    }

    fn ivy_cache() -> PathBuf {
        if let Ok(home) = std::env::var("IVY_HOME") {
            return PathBuf::from(home).join("cache");
        }
        dirs::home_dir()
            .unwrap_or_default()
            .join(".ivy2")
            .join("cache")
    }

    fn coursier_cache() -> PathBuf {
        // Coursier is the default dependency fetcher for sbt 1.3+
        if let Ok(cache) = std::env::var("COURSIER_CACHE") {
            return PathBuf::from(cache);
        }
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            let p = PathBuf::from(local)
                .join("Coursier")
                .join("cache")
                .join("v1");
            if p.exists() {
                return p;
            }
        }
        dirs::home_dir()
            .unwrap_or_default()
            .join(".cache")
            .join("coursier")
            .join("v1")
    }
}

#[async_trait]
impl PackageManager for SbtManager {
    fn name(&self) -> &'static str {
        "sbt"
    }
    fn display_name(&self) -> &'static str {
        "sbt (Scala Build Tool)"
    }

    async fn is_installed(&self) -> bool {
        which("sbt").is_ok() || which("sbt.bat").is_ok() || Self::ivy_cache().exists()
    }

    async fn get_version(&self) -> Result<Option<String>> {
        if let Ok(output) = tokio::process::Command::new("sbt")
            .arg("sbtVersion")
            .output()
            .await
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                if let Some(line) = text.lines().find(|l| l.contains("[info]")) {
                    return Ok(Some(line.trim().to_string()));
                }
            }
        }
        Ok(None)
    }

    async fn get_cache_paths(&self) -> Result<Vec<PathBuf>> {
        let mut paths = vec![
            Self::ivy_cache(),
            Self::coursier_cache(),
            Self::sbt_home().join("boot"),
        ];
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

        info!("Cleaning sbt / ivy2 / coursier caches");
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

        let ivy = Self::ivy_cache();
        if ivy.exists() {
            let size = calculate_directory_size(&ivy).await.unwrap_or(0);
            info.push(CacheInfo {
                path: ivy,
                size_bytes: size,
                description: "Ivy2 dependency cache (~/.ivy2/cache)".to_string(),
                can_delete: true,
            });
        }

        let coursier = Self::coursier_cache();
        if coursier.exists() {
            let size = calculate_directory_size(&coursier).await.unwrap_or(0);
            info.push(CacheInfo {
                path: coursier,
                size_bytes: size,
                description: "Coursier artifact cache".to_string(),
                can_delete: true,
            });
        }

        let boot = Self::sbt_home().join("boot");
        if boot.exists() {
            let size = calculate_directory_size(&boot).await.unwrap_or(0);
            info.push(CacheInfo {
                path: boot,
                size_bytes: size,
                description: "sbt boot directory (~/.sbt/boot)".to_string(),
                can_delete: true,
            });
        }

        Ok(info)
    }

    fn prevention_tip(&self) -> &'static str {
        "Run 'sbt clean' after builds. Set COURSIER_CACHE to a shared location and use 'coursier gc' periodically."
    }
}
