//! Browser cache managers for Chrome, Edge, and Firefox
//!
//! These are surfaced in the Package Managers view so users can review and
//! clear browser caches the same way as any other cache.

use super::{calculate_directory_size, safe_delete_directory, CacheInfo, PackageCleanResult};
use crate::package_manager::PackageManager;
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tracing::{debug, info};

// ── helpers ──────────────────────────────────────────────────────────────────

/// Collect every `Cache`, `Cache2`, `Code Cache` and `GPUCache` sub-directory
/// found inside all Chromium "User Data" profiles.
async fn chromium_cache_paths(user_data: &PathBuf) -> Vec<PathBuf> {
    let cache_dirs = [
        "Cache",
        "Cache2",
        "Code Cache",
        "GPUCache",
        "ShaderCache",
        "JumpListIcons",
        "JumpListIconsOld",
    ];
    let mut paths = Vec::new();
    if !user_data.exists() {
        return paths;
    }
    // Enumerate profile directories (Default, Profile 1, Profile 2 …)
    let Ok(mut entries) = tokio::fs::read_dir(user_data).await else {
        return paths;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let profile = entry.path();
        if !profile.is_dir() {
            continue;
        }
        for dir in &cache_dirs {
            let candidate = profile.join(dir);
            if candidate.is_dir() {
                debug!("Browser cache found: {}", candidate.display());
                paths.push(candidate);
            }
        }
    }
    paths
}

/// Sum the size of every path in the list, ignoring missing ones.
async fn total_size(paths: &[PathBuf]) -> u64 {
    let mut total = 0u64;
    for p in paths {
        total += calculate_directory_size(p).await.unwrap_or(0);
    }
    total
}

/// Delete every path in the list; return (total_freed, error_list).
async fn delete_paths(paths: &[PathBuf]) -> (u64, Vec<String>) {
    let mut freed = 0u64;
    let mut errors = Vec::new();
    for p in paths {
        match safe_delete_directory(p).await {
            Ok(n) => freed += n,
            Err(e) => errors.push(format!("{}: {}", p.display(), e)),
        }
    }
    (freed, errors)
}

// ── Chrome ────────────────────────────────────────────────────────────────────

/// Google Chrome browser cache manager
pub struct ChromeManager;

impl ChromeManager {
    pub async fn new() -> Result<Self> {
        Ok(Self)
    }

    fn user_data_path() -> Option<PathBuf> {
        dirs::cache_dir().map(|base| {
            // dirs::cache_dir() → %LOCALAPPDATA%
            base.parent()
                .unwrap_or(&base)
                .join("Local")
                .join("Google")
                .join("Chrome")
                .join("User Data")
        })
    }
}

#[async_trait]
impl PackageManager for ChromeManager {
    fn name(&self) -> &'static str {
        "chrome"
    }

    fn display_name(&self) -> &'static str {
        "Google Chrome"
    }

    async fn is_installed(&self) -> bool {
        Self::user_data_path().map(|p| p.exists()).unwrap_or(false)
    }

    async fn get_version(&self) -> Result<Option<String>> {
        Ok(None)
    }

    async fn get_cache_paths(&self) -> Result<Vec<PathBuf>> {
        let Some(user_data) = Self::user_data_path() else {
            return Ok(vec![]);
        };
        Ok(chromium_cache_paths(&user_data).await)
    }

    async fn calculate_cache_size(&self) -> Result<u64> {
        let paths = self.get_cache_paths().await?;
        Ok(total_size(&paths).await)
    }

    async fn clean_all_caches(&self) -> Result<PackageCleanResult> {
        let start = std::time::Instant::now();
        let paths = self.get_cache_paths().await?;
        info!("Cleaning Chrome caches ({} directories)", paths.len());
        let (freed, errors) = delete_paths(&paths).await;
        Ok(PackageCleanResult {
            package_manager: "Google Chrome".to_string(),
            space_freed: freed,
            items_deleted: paths.len() as u64,
            errors,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn clean_paths(&self, paths: &[PathBuf]) -> Result<PackageCleanResult> {
        let start = std::time::Instant::now();
        let (freed, errors) = delete_paths(paths).await;
        Ok(PackageCleanResult {
            package_manager: "Google Chrome".to_string(),
            space_freed: freed,
            items_deleted: paths.len() as u64,
            errors,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn get_cache_info(&self) -> Result<Vec<CacheInfo>> {
        let paths = self.get_cache_paths().await?;
        let mut info = Vec::new();
        for p in paths {
            let size = calculate_directory_size(&p).await.unwrap_or(0);
            info.push(CacheInfo {
                path: p.clone(),
                size_bytes: size,
                description: "Chrome cache directory".to_string(),
                can_delete: true,
            });
        }
        Ok(info)
    }

    fn prevention_tip(&self) -> &'static str {
        "Enable 'Clear cookies and site data when you close all windows' in Chrome settings."
    }
}

// ── Edge ──────────────────────────────────────────────────────────────────────

/// Microsoft Edge browser cache manager
pub struct EdgeManager;

impl EdgeManager {
    pub async fn new() -> Result<Self> {
        Ok(Self)
    }

    fn user_data_path() -> Option<PathBuf> {
        dirs::cache_dir().map(|base| {
            base.parent()
                .unwrap_or(&base)
                .join("Local")
                .join("Microsoft")
                .join("Edge")
                .join("User Data")
        })
    }
}

#[async_trait]
impl PackageManager for EdgeManager {
    fn name(&self) -> &'static str {
        "edge"
    }

    fn display_name(&self) -> &'static str {
        "Microsoft Edge"
    }

    async fn is_installed(&self) -> bool {
        Self::user_data_path().map(|p| p.exists()).unwrap_or(false)
    }

    async fn get_version(&self) -> Result<Option<String>> {
        Ok(None)
    }

    async fn get_cache_paths(&self) -> Result<Vec<PathBuf>> {
        let Some(user_data) = Self::user_data_path() else {
            return Ok(vec![]);
        };
        Ok(chromium_cache_paths(&user_data).await)
    }

    async fn calculate_cache_size(&self) -> Result<u64> {
        let paths = self.get_cache_paths().await?;
        Ok(total_size(&paths).await)
    }

    async fn clean_all_caches(&self) -> Result<PackageCleanResult> {
        let start = std::time::Instant::now();
        let paths = self.get_cache_paths().await?;
        info!("Cleaning Edge caches ({} directories)", paths.len());
        let (freed, errors) = delete_paths(&paths).await;
        Ok(PackageCleanResult {
            package_manager: "Microsoft Edge".to_string(),
            space_freed: freed,
            items_deleted: paths.len() as u64,
            errors,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn clean_paths(&self, paths: &[PathBuf]) -> Result<PackageCleanResult> {
        let start = std::time::Instant::now();
        let (freed, errors) = delete_paths(paths).await;
        Ok(PackageCleanResult {
            package_manager: "Microsoft Edge".to_string(),
            space_freed: freed,
            items_deleted: paths.len() as u64,
            errors,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn get_cache_info(&self) -> Result<Vec<CacheInfo>> {
        let paths = self.get_cache_paths().await?;
        let mut info = Vec::new();
        for p in paths {
            let size = calculate_directory_size(&p).await.unwrap_or(0);
            info.push(CacheInfo {
                path: p.clone(),
                size_bytes: size,
                description: "Edge cache directory".to_string(),
                can_delete: true,
            });
        }
        Ok(info)
    }

    fn prevention_tip(&self) -> &'static str {
        "Enable 'Clear browsing data on close' in Edge Privacy settings."
    }
}

// ── Firefox ───────────────────────────────────────────────────────────────────

/// Mozilla Firefox browser cache manager
pub struct FirefoxManager;

impl FirefoxManager {
    pub async fn new() -> Result<Self> {
        Ok(Self)
    }

    /// Returns every `cache2` directory found in Firefox profiles.
    async fn cache_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        // Primary cache location: %LOCALAPPDATA%\Mozilla\Firefox\Profiles\*\cache2
        let local_cache_root = dirs::cache_dir().map(|b| {
            b.parent()
                .unwrap_or(&b)
                .join("Local")
                .join("Mozilla")
                .join("Firefox")
                .join("Profiles")
        });

        if let Some(root) = local_cache_root {
            if let Ok(mut profiles) = tokio::fs::read_dir(&root).await {
                while let Ok(Some(entry)) = profiles.next_entry().await {
                    let candidate = entry.path().join("cache2");
                    if candidate.is_dir() {
                        debug!("Firefox cache2 found: {}", candidate.display());
                        paths.push(candidate);
                    }
                }
            }
        }

        // Roaming profiles (older Firefox): %APPDATA%\Mozilla\Firefox\Profiles\*\cache2
        let roaming_root =
            dirs::config_dir().map(|b| b.join("Mozilla").join("Firefox").join("Profiles"));
        if let Some(root) = roaming_root {
            if let Ok(mut profiles) = tokio::fs::read_dir(&root).await {
                while let Ok(Some(entry)) = profiles.next_entry().await {
                    let candidate = entry.path().join("cache2");
                    if candidate.is_dir() && !paths.contains(&candidate) {
                        debug!("Firefox roaming cache2 found: {}", candidate.display());
                        paths.push(candidate);
                    }
                }
            }
        }

        paths
    }

    fn profiles_root_exists() -> bool {
        let local = dirs::cache_dir().map(|b| {
            b.parent()
                .unwrap_or(&b)
                .join("Local")
                .join("Mozilla")
                .join("Firefox")
                .join("Profiles")
        });
        let roaming =
            dirs::config_dir().map(|b| b.join("Mozilla").join("Firefox").join("Profiles"));
        local.map(|p| p.exists()).unwrap_or(false) || roaming.map(|p| p.exists()).unwrap_or(false)
    }
}

#[async_trait]
impl PackageManager for FirefoxManager {
    fn name(&self) -> &'static str {
        "firefox"
    }

    fn display_name(&self) -> &'static str {
        "Mozilla Firefox"
    }

    async fn is_installed(&self) -> bool {
        Self::profiles_root_exists()
    }

    async fn get_version(&self) -> Result<Option<String>> {
        Ok(None)
    }

    async fn get_cache_paths(&self) -> Result<Vec<PathBuf>> {
        Ok(Self::cache_paths().await)
    }

    async fn calculate_cache_size(&self) -> Result<u64> {
        let paths = self.get_cache_paths().await?;
        Ok(total_size(&paths).await)
    }

    async fn clean_all_caches(&self) -> Result<PackageCleanResult> {
        let start = std::time::Instant::now();
        let paths = self.get_cache_paths().await?;
        info!("Cleaning Firefox caches ({} directories)", paths.len());
        let (freed, errors) = delete_paths(&paths).await;
        Ok(PackageCleanResult {
            package_manager: "Mozilla Firefox".to_string(),
            space_freed: freed,
            items_deleted: paths.len() as u64,
            errors,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn clean_paths(&self, paths: &[PathBuf]) -> Result<PackageCleanResult> {
        let start = std::time::Instant::now();
        let (freed, errors) = delete_paths(paths).await;
        Ok(PackageCleanResult {
            package_manager: "Mozilla Firefox".to_string(),
            space_freed: freed,
            items_deleted: paths.len() as u64,
            errors,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn get_cache_info(&self) -> Result<Vec<CacheInfo>> {
        let paths = self.get_cache_paths().await?;
        let mut info = Vec::new();
        for p in paths {
            let size = calculate_directory_size(&p).await.unwrap_or(0);
            info.push(CacheInfo {
                path: p.clone(),
                size_bytes: size,
                description: "Firefox cache2 directory".to_string(),
                can_delete: true,
            });
        }
        Ok(info)
    }

    fn prevention_tip(&self) -> &'static str {
        "Use Firefox's 'Delete cookies and site data when Firefox is closed' option in Privacy & Security."
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_chrome_manager_creation() {
        let manager = ChromeManager::new().await;
        assert!(
            manager.is_ok(),
            "ChromeManager::new() should always succeed"
        );
        assert_eq!(manager.unwrap().name(), "chrome");
    }

    #[tokio::test]
    async fn test_edge_manager_creation() {
        let manager = EdgeManager::new().await;
        assert!(manager.is_ok(), "EdgeManager::new() should always succeed");
        assert_eq!(manager.unwrap().name(), "edge");
    }

    #[tokio::test]
    async fn test_firefox_manager_creation() {
        let manager = FirefoxManager::new().await;
        assert!(
            manager.is_ok(),
            "FirefoxManager::new() should always succeed"
        );
        assert_eq!(manager.unwrap().name(), "firefox");
    }

    #[tokio::test]
    async fn test_display_names() {
        assert_eq!(ChromeManager.display_name(), "Google Chrome");
        assert_eq!(EdgeManager.display_name(), "Microsoft Edge");
        assert_eq!(FirefoxManager.display_name(), "Mozilla Firefox");
    }

    #[tokio::test]
    async fn test_chrome_empty_user_data() {
        let temp = TempDir::new().unwrap();
        // An empty directory → no cache subdirs found
        let paths = chromium_cache_paths(&temp.path().to_path_buf()).await;
        assert!(paths.is_empty(), "no cache paths in empty dir");
    }

    #[tokio::test]
    async fn test_chromium_cache_paths_detection() {
        let temp = TempDir::new().unwrap();
        // Create a fake "Default" profile with a Cache directory
        let profile = temp.path().join("Default");
        tokio::fs::create_dir_all(profile.join("Cache"))
            .await
            .unwrap();
        tokio::fs::create_dir_all(profile.join("Code Cache"))
            .await
            .unwrap();

        let paths = chromium_cache_paths(&temp.path().to_path_buf()).await;
        assert_eq!(paths.len(), 2);
        assert!(paths.iter().any(|p| p.ends_with("Cache")));
        assert!(paths.iter().any(|p| p.ends_with("Code Cache")));
    }

    #[tokio::test]
    async fn test_clean_paths_returns_freed() {
        let temp = TempDir::new().unwrap();
        let cache_dir = temp.path().join("Cache");
        tokio::fs::create_dir_all(&cache_dir).await.unwrap();
        tokio::fs::write(cache_dir.join("file.bin"), b"hello world")
            .await
            .unwrap();

        let manager = ChromeManager;
        let result = manager.clean_paths(&[cache_dir.clone()]).await.unwrap();
        assert_eq!(result.space_freed, 11, "should report freed bytes");
        assert!(!cache_dir.exists(), "cache dir should be deleted");
    }

    #[tokio::test]
    async fn test_get_cache_info_empty() {
        let manager = ChromeManager;
        // Even if Chrome is not installed, get_cache_info returns an empty vec (not an error)
        let result = manager.get_cache_info().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_total_size_missing_paths() {
        // Paths that don't exist should contribute 0 to total
        let paths = vec![PathBuf::from(r"C:\does\not\exist\cache")];
        let size = total_size(&paths).await;
        assert_eq!(size, 0);
    }
}
