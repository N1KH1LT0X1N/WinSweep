//! Vagrant box cache manager implementation

use super::{
    calculate_directory_size, safe_delete_directory, CacheInfo, PackageCleanResult, PackageManager,
};
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tracing::info;
use which::which;

/// Vagrant box cache manager
#[derive(Default)]
pub struct VagrantManager;

impl VagrantManager {
    pub async fn new() -> Result<Self> {
        Ok(Self)
    }

    fn boxes_dir() -> Option<PathBuf> {
        // VAGRANT_HOME\boxes or ~/.vagrant.d/boxes
        if let Ok(home) = std::env::var("VAGRANT_HOME") {
            let p = PathBuf::from(&home).join("boxes");
            if p.exists() {
                return Some(p);
            }
        }
        dirs::home_dir().map(|h| h.join(".vagrant.d").join("boxes"))
    }
}

#[async_trait]
impl PackageManager for VagrantManager {
    fn name(&self) -> &'static str {
        "vagrant"
    }

    fn display_name(&self) -> &'static str {
        "Vagrant"
    }

    async fn is_installed(&self) -> bool {
        which("vagrant.exe").is_ok()
            || which("vagrant").is_ok()
            || Self::boxes_dir().map(|p| p.exists()).unwrap_or(false)
    }

    async fn get_version(&self) -> Result<Option<String>> {
        if which("vagrant.exe").is_ok() || which("vagrant").is_ok() {
            let output = tokio::process::Command::new("vagrant")
                .arg("--version")
                .output()
                .await;
            if let Ok(result) = output {
                if result.status.success() {
                    return Ok(Some(
                        String::from_utf8_lossy(&result.stdout).trim().to_string(),
                    ));
                }
            }
        }
        Ok(None)
    }

    async fn get_cache_paths(&self) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        if let Some(dir) = Self::boxes_dir() {
            if dir.exists() {
                paths.push(dir);
            }
        }
        Ok(paths)
    }

    async fn calculate_cache_size(&self) -> Result<u64> {
        let paths = self.get_cache_paths().await?;
        let mut total = 0u64;
        for p in paths {
            if p.exists() {
                total += calculate_directory_size(&p).await?;
            }
        }
        Ok(total)
    }

    async fn clean_all_caches(&self) -> Result<PackageCleanResult> {
        let start = std::time::Instant::now();
        let paths = self.get_cache_paths().await?;
        let mut freed = 0u64;
        let mut items = 0u64;
        let mut errors = Vec::new();
        info!("Cleaning Vagrant boxes ({} directories)", paths.len());
        for p in paths {
            if p.exists() {
                match safe_delete_directory(&p).await {
                    Ok(n) => {
                        freed += n;
                        items += 1;
                    }
                    Err(e) => errors.push(format!("{}: {}", p.display(), e)),
                }
            }
        }
        Ok(PackageCleanResult {
            package_manager: self.name().to_string(),
            space_freed: freed,
            items_deleted: items,
            errors,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn clean_paths(&self, paths: &[PathBuf]) -> Result<PackageCleanResult> {
        let start = std::time::Instant::now();
        let mut freed = 0u64;
        let mut items = 0u64;
        let mut errors = Vec::new();
        for p in paths {
            if p.exists() {
                match safe_delete_directory(p).await {
                    Ok(n) => {
                        freed += n;
                        items += 1;
                    }
                    Err(e) => errors.push(format!("{}: {}", p.display(), e)),
                }
            }
        }
        Ok(PackageCleanResult {
            package_manager: self.name().to_string(),
            space_freed: freed,
            items_deleted: items,
            errors,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn get_cache_info(&self) -> Result<Vec<CacheInfo>> {
        let mut info = Vec::new();
        if let Ok(paths) = self.get_cache_paths().await {
            for p in paths {
                if p.exists() {
                    let size = calculate_directory_size(&p).await.unwrap_or(0);
                    info.push(CacheInfo {
                        path: p.clone(),
                        size_bytes: size,
                        description: "Vagrant boxes".to_string(),
                        can_delete: true,
                    });
                }
            }
        }
        Ok(info)
    }

    fn prevention_tip(&self) -> &'static str {
        "Remove unused boxes with 'vagrant box remove <name>'. Use lightweight base images (e.g., generic/alpine) when possible."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_vagrant_manager_creation() {
        let manager = VagrantManager::new().await;
        assert!(manager.is_ok());
        assert_eq!(manager.unwrap().name(), "vagrant");
    }

    #[test]
    fn test_display_name() {
        assert_eq!(VagrantManager::default().display_name(), "Vagrant");
    }
}
