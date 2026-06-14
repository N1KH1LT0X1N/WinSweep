//! Ruby gems package manager implementation

use super::{
    calculate_directory_size, safe_delete_directory, CacheInfo, PackageCleanResult, PackageManager,
};
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::process::Command;
use tracing::info;
use which::which;

/// Ruby gems cache manager
#[derive(Default)]
pub struct RubyGemsManager {
    gem_path: Option<PathBuf>,
    cache_path: Option<PathBuf>,
}

impl RubyGemsManager {
    pub async fn new() -> Result<Self> {
        Ok(Self {
            gem_path: None,
            cache_path: None,
        })
    }

    async fn get_gem_path(&self) -> Result<PathBuf> {
        if let Some(ref path) = self.gem_path {
            return Ok(path.clone());
        }
        if which("gem.cmd").is_ok() || which("gem").is_ok() {
            return Ok(PathBuf::from("gem"));
        }
        anyhow::bail!("gem not found in PATH")
    }

    async fn resolve_cache_path(&self) -> Result<PathBuf> {
        if let Some(ref path) = self.cache_path {
            return Ok(path.clone());
        }

        // Try `gem environment gemdir`
        if let Ok(gem_path) = self.get_gem_path().await {
            let output = Command::new(&gem_path)
                .args(["environment", "gemdir"])
                .output()
                .await;
            if let Ok(result) = output {
                if result.status.success() {
                    let stdout = String::from_utf8_lossy(&result.stdout);
                    let path = PathBuf::from(stdout.trim());
                    if path.exists() {
                        return Ok(path.join("cache"));
                    }
                }
            }
        }

        // Try GEM_HOME
        if let Ok(gem_home) = std::env::var("GEM_HOME") {
            let path = PathBuf::from(&gem_home).join("cache");
            if path.exists() {
                return Ok(path);
            }
        }

        // Fallback to ~/.gem
        if let Some(home) = dirs::home_dir() {
            let path = home.join(".gem");
            if path.exists() {
                return Ok(path);
            }
        }

        anyhow::bail!("Could not resolve Ruby gems cache path")
    }
}

#[async_trait]
impl PackageManager for RubyGemsManager {
    fn name(&self) -> &'static str {
        "ruby_gems"
    }

    fn display_name(&self) -> &'static str {
        "Ruby Gems"
    }

    async fn is_installed(&self) -> bool {
        which("gem.cmd").is_ok()
            || which("gem").is_ok()
            || std::env::var("GEM_HOME").is_ok()
            || dirs::home_dir()
                .map(|h| h.join(".gem").exists())
                .unwrap_or(false)
    }

    async fn get_version(&self) -> Result<Option<String>> {
        if let Ok(gem_path) = self.get_gem_path().await {
            let output = Command::new(&gem_path).arg("--version").output().await;
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
        if let Ok(cache) = self.resolve_cache_path().await {
            paths.push(cache);
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
        info!("Cleaning Ruby gems caches ({} directories)", paths.len());
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
                        description: "Ruby gems cache".to_string(),
                        can_delete: true,
                    });
                }
            }
        }
        Ok(info)
    }

    fn prevention_tip(&self) -> &'static str {
        "Use 'gem cleanup' regularly to remove old gem versions. Set GEM_HOME to a single shared location."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ruby_gems_manager_creation() {
        let manager = RubyGemsManager::new().await;
        assert!(manager.is_ok());
        assert_eq!(manager.unwrap().name(), "ruby_gems");
    }

    #[test]
    fn test_display_name() {
        assert_eq!(RubyGemsManager::default().display_name(), "Ruby Gems");
    }
}
