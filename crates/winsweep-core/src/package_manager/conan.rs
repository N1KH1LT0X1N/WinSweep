//! Conan C/C++ package manager cache cleanup

use super::{
    calculate_directory_size, safe_delete_directory, CacheInfo, PackageCleanResult, PackageManager,
};
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tracing::{debug, info, warn};
use which::which;

/// Conan C/C++ package manager
#[derive(Default)]
pub struct ConanManager;

impl ConanManager {
    pub async fn new() -> Result<Self> {
        Ok(Self)
    }

    fn conan_home() -> PathBuf {
        if let Ok(home) = std::env::var("CONAN_HOME") {
            return PathBuf::from(home);
        }
        // Conan 2.x uses ~/.conan2, Conan 1.x uses ~/.conan
        let home = dirs::home_dir().unwrap_or_default();
        let v2 = home.join(".conan2");
        if v2.exists() {
            v2
        } else {
            home.join(".conan")
        }
    }
}

#[async_trait]
impl PackageManager for ConanManager {
    fn name(&self) -> &'static str {
        "conan"
    }
    fn display_name(&self) -> &'static str {
        "Conan (C/C++)"
    }

    async fn is_installed(&self) -> bool {
        which("conan").is_ok() || which("conan.exe").is_ok()
    }

    async fn get_version(&self) -> Result<Option<String>> {
        if let Ok(output) = tokio::process::Command::new("conan")
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
        let conan_home = Self::conan_home();
        let mut paths = vec![
            conan_home.join("p"),    // Conan 2.x package store
            conan_home.join("data"), // Conan 1.x package data
            conan_home.join("b"),    // Conan 2.x build folder
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

        info!("Cleaning Conan package cache");

        // Try `conan remove -c *` for a clean removal (Conan 2.x)
        let _ = tokio::process::Command::new("conan")
            .args(["remove", "--confirm", "*"])
            .output()
            .await;

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
        let conan_home = Self::conan_home();
        let mut info = Vec::new();

        for (rel, desc) in [
            ("p", "Conan 2.x package store"),
            ("data", "Conan 1.x package data"),
            ("b", "Conan 2.x build artifacts"),
        ] {
            let p = conan_home.join(rel);
            if p.exists() {
                let size = calculate_directory_size(&p).await.unwrap_or(0);
                info.push(CacheInfo {
                    path: p,
                    size_bytes: size,
                    description: desc.to_string(),
                    can_delete: true,
                });
            }
        }

        Ok(info)
    }

    fn prevention_tip(&self) -> &'static str {
        "Use 'conan remove -c *' to clear old packages. Enable revision_mode=scm to avoid duplicate recipe builds."
    }
}
