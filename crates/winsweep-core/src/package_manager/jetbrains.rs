//! JetBrains IDE cache / log manager implementation

use super::{
    calculate_directory_size, safe_delete_directory, CacheInfo, PackageCleanResult, PackageManager,
};
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tracing::info;

/// JetBrains IDE cache manager
#[derive(Default)]
pub struct JetBrainsManager;

impl JetBrainsManager {
    pub async fn new() -> Result<Self> {
        Ok(Self)
    }

    fn base_dir() -> Option<PathBuf> {
        // %LOCALAPPDATA%\JetBrains
        dirs::cache_dir().map(|d| d.join("JetBrains"))
    }

    fn cache_dirs() -> Vec<PathBuf> {
        let mut paths = Vec::new();
        if let Some(base) = Self::base_dir() {
            if !base.exists() {
                return paths;
            }
            // Each IDE has its own subfolder (e.g., IntelliJIdea2023.1, WebStorm2023.2, etc.)
            if let Ok(entries) = std::fs::read_dir(&base) {
                for entry in entries.flatten() {
                    let product = entry.path();
                    if !product.is_dir() {
                        continue;
                    }
                    for sub in &["log", "cache", "caches", "tmp"] {
                        let p = product.join(sub);
                        if p.exists() {
                            paths.push(p);
                        }
                    }
                }
            }
        }
        paths
    }
}

#[async_trait]
impl PackageManager for JetBrainsManager {
    fn name(&self) -> &'static str {
        "jetbrains"
    }

    fn display_name(&self) -> &'static str {
        "JetBrains IDEs"
    }

    async fn is_installed(&self) -> bool {
        Self::base_dir().map(|p| p.exists()).unwrap_or(false)
    }

    async fn get_version(&self) -> Result<Option<String>> {
        Ok(None)
    }

    async fn get_cache_paths(&self) -> Result<Vec<PathBuf>> {
        Ok(Self::cache_dirs())
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
        info!("Cleaning JetBrains caches ({} directories)", paths.len());
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
                    let desc = if p.file_name().map(|n| n == "log").unwrap_or(false) {
                        "JetBrains logs"
                    } else {
                        "JetBrains cache"
                    };
                    info.push(CacheInfo {
                        path: p.clone(),
                        size_bytes: size,
                        description: desc.to_string(),
                        can_delete: true,
                    });
                }
            }
        }
        Ok(info)
    }

    fn prevention_tip(&self) -> &'static str {
        "Enable 'Help | Delete IDE Logs and Caches' after major updates. Reduce local history retention in Settings | Appearance & Behavior | System Settings."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_jetbrains_manager_creation() {
        let manager = JetBrainsManager::new().await;
        assert!(manager.is_ok());
        assert_eq!(manager.unwrap().name(), "jetbrains");
    }

    #[test]
    fn test_display_name() {
        assert_eq!(JetBrainsManager.display_name(), "JetBrains IDEs");
    }
}
