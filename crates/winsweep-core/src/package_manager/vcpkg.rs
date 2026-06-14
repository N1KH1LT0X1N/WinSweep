//! vcpkg C/C++ package manager cache cleanup

use super::{
    calculate_directory_size, safe_delete_directory, CacheInfo, PackageCleanResult, PackageManager,
};
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tracing::{debug, info, warn};
use which::which;

/// vcpkg C/C++ package manager
#[derive(Default)]
pub struct VcpkgManager;

impl VcpkgManager {
    pub async fn new() -> Result<Self> {
        Ok(Self)
    }

    fn vcpkg_root() -> Option<PathBuf> {
        if let Ok(root) = std::env::var("VCPKG_ROOT") {
            let p = PathBuf::from(root);
            if p.exists() {
                return Some(p);
            }
        }
        // Common default install locations
        for candidate in [r"C:\vcpkg", r"C:\src\vcpkg", r"C:\tools\vcpkg"] {
            let p = PathBuf::from(candidate);
            if p.exists() {
                return Some(p);
            }
        }
        // Check home dir
        let home = dirs::home_dir().unwrap_or_default();
        let p = home.join("vcpkg");
        if p.exists() {
            return Some(p);
        }
        None
    }

    fn default_binary_cache() -> PathBuf {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            PathBuf::from(local).join("vcpkg").join("archives")
        } else {
            dirs::home_dir()
                .unwrap_or_default()
                .join(".vcpkg")
                .join("archives")
        }
    }
}

#[async_trait]
impl PackageManager for VcpkgManager {
    fn name(&self) -> &'static str {
        "vcpkg"
    }
    fn display_name(&self) -> &'static str {
        "vcpkg (C/C++)"
    }

    async fn is_installed(&self) -> bool {
        which("vcpkg").is_ok() || which("vcpkg.exe").is_ok() || Self::vcpkg_root().is_some()
    }

    async fn get_version(&self) -> Result<Option<String>> {
        if let Ok(output) = tokio::process::Command::new("vcpkg")
            .arg("version")
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
        let mut paths = vec![Self::default_binary_cache()];

        if let Some(root) = Self::vcpkg_root() {
            // buildtrees and packages directories can be large
            let buildtrees = root.join("buildtrees");
            let packages = root.join("packages");
            if buildtrees.exists() {
                paths.push(buildtrees);
            }
            if packages.exists() {
                paths.push(packages);
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

        info!("Cleaning vcpkg caches");
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

        let binary_cache = Self::default_binary_cache();
        if binary_cache.exists() {
            let size = calculate_directory_size(&binary_cache).await.unwrap_or(0);
            info.push(CacheInfo {
                path: binary_cache,
                size_bytes: size,
                description: "vcpkg binary cache (archives)".to_string(),
                can_delete: true,
            });
        }

        if let Some(root) = Self::vcpkg_root() {
            for (rel, desc, can_delete) in [
                ("buildtrees", "vcpkg build trees (safe to delete)", true),
                ("packages", "vcpkg installed packages", false),
            ] {
                let p = root.join(rel);
                if p.exists() {
                    let size = calculate_directory_size(&p).await.unwrap_or(0);
                    info.push(CacheInfo {
                        path: p,
                        size_bytes: size,
                        description: desc.to_string(),
                        can_delete,
                    });
                }
            }
        }

        Ok(info)
    }

    fn prevention_tip(&self) -> &'static str {
        "Set VCPKG_BINARY_SOURCES to a shared NuGet or HTTP cache. Clean buildtrees/ directory regularly."
    }
}
