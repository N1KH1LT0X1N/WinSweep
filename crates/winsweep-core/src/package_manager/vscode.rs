//! VS Code cache manager implementation

use super::{
    calculate_directory_size, safe_delete_directory, CacheInfo, PackageCleanResult, PackageManager,
};
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tracing::info;

/// Visual Studio Code cache manager
#[derive(Default)]
pub struct VsCodeManager;

impl VsCodeManager {
    pub async fn new() -> Result<Self> {
        Ok(Self)
    }

    fn code_dir() -> Option<PathBuf> {
        // Portable VS Code sets VSCODE_PORTABLE to the data directory root.
        // In portable mode the user-data lives at $VSCODE_PORTABLE/user-data.
        if let Ok(portable) = std::env::var("VSCODE_PORTABLE") {
            let p = PathBuf::from(portable).join("user-data");
            if p.exists() {
                return Some(p);
            }
        }
        // Standard install: %APPDATA%\Code on Windows
        dirs::config_dir().map(|d| d.join("Code"))
    }

    fn cache_dirs() -> Vec<PathBuf> {
        let mut paths = Vec::new();
        let cache_subdirs = ["Cache", "CachedData", "CachedExtensions", "logs"];

        // Standard / portable VS Code
        if let Some(base) = Self::code_dir() {
            for sub in &cache_subdirs {
                let p = base.join(sub);
                if p.exists() {
                    paths.push(p);
                }
            }
        }

        // VS Code Insiders (uses a separate config directory)
        if let Some(config) = dirs::config_dir() {
            let insiders = config.join("Code - Insiders");
            if insiders.exists() {
                for sub in &cache_subdirs {
                    let p = insiders.join(sub);
                    if p.exists() {
                        paths.push(p);
                    }
                }
            }
        }

        paths
    }
}

#[async_trait]
impl PackageManager for VsCodeManager {
    fn name(&self) -> &'static str {
        "vscode"
    }

    fn display_name(&self) -> &'static str {
        "Visual Studio Code"
    }

    async fn is_installed(&self) -> bool {
        Self::code_dir().map(|p| p.exists()).unwrap_or(false)
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
        info!("Cleaning VS Code caches ({} directories)", paths.len());
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
                    let desc = if p.file_name().map(|n| n == "logs").unwrap_or(false) {
                        "VS Code logs"
                    } else {
                        "VS Code cache"
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
        "Disable unused extensions and set 'workbench.enableExperiments' to false to reduce CachedExtensions growth."
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_vscode_manager_creation() {
        let manager = VsCodeManager::new().await;
        assert!(manager.is_ok());
        assert_eq!(manager.unwrap().name(), "vscode");
    }

    #[test]
    fn test_display_name() {
        assert_eq!(VsCodeManager.display_name(), "Visual Studio Code");
    }

    /// `code_dir()` must honour VSCODE_PORTABLE env var and return the
    /// user-data sub-directory when it exists.
    #[test]
    fn test_code_dir_uses_vscode_portable_env() {
        let tmp = TempDir::new().unwrap();
        let user_data = tmp.path().join("user-data");
        std::fs::create_dir_all(&user_data).unwrap();

        std::env::set_var("VSCODE_PORTABLE", tmp.path());
        let dir = VsCodeManager::code_dir();
        std::env::remove_var("VSCODE_PORTABLE");

        assert_eq!(
            dir,
            Some(user_data),
            "code_dir() must use VSCODE_PORTABLE/user-data when it exists"
        );
    }

    /// When VSCODE_PORTABLE points to a directory without user-data, code_dir()
    /// must fall back to the standard config path.
    #[test]
    fn test_code_dir_falls_back_when_portable_user_data_absent() {
        let tmp = TempDir::new().unwrap();
        // Do NOT create user-data inside tmp
        std::env::set_var("VSCODE_PORTABLE", tmp.path());
        let dir = VsCodeManager::code_dir();
        std::env::remove_var("VSCODE_PORTABLE");

        // The fallback is dirs::config_dir().map(|d| d.join("Code"))
        let expected = dirs::config_dir().map(|d| d.join("Code"));
        assert_eq!(dir, expected, "must fall back to standard Code config dir");
    }

    /// `cache_dirs()` must not panic even if VS Code is not installed.
    #[test]
    fn test_cache_dirs_no_panic() {
        let dirs = VsCodeManager::cache_dirs();
        // All returned paths must actually exist on disk
        for p in &dirs {
            assert!(
                p.exists(),
                "cache_dirs() must only return paths that exist: {}",
                p.display()
            );
        }
    }
}
