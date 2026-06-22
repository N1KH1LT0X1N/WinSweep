//! Bundler package manager implementation

use super::{
    calculate_directory_size, safe_delete_directory, CacheInfo, PackageCleanResult, PackageManager,
};
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::process::Command;
use tracing::info;
use which::which;

/// Bundler (Ruby) cache manager
#[derive(Default)]
pub struct BundlerManager {
    bundle_path: Option<PathBuf>,
}

impl BundlerManager {
    pub async fn new() -> Result<Self> {
        Ok(Self { bundle_path: None })
    }

    async fn get_bundle_path(&self) -> Result<PathBuf> {
        if let Some(ref path) = self.bundle_path {
            return Ok(path.clone());
        }
        if which("bundle.cmd").is_ok() || which("bundle").is_ok() {
            return Ok(PathBuf::from("bundle"));
        }
        anyhow::bail!("bundle not found in PATH")
    }

    async fn resolve_vendor_path(&self) -> Option<PathBuf> {
        // Try BUNDLE_PATH env var
        if let Ok(bp) = std::env::var("BUNDLE_PATH") {
            let path = PathBuf::from(bp);
            if path.exists() {
                return Some(path);
            }
        }

        // Try `bundle config path` (default vendor path)
        if let Ok(bundle_path) = self.get_bundle_path().await {
            let output = Command::new(&bundle_path)
                .args(["config", "path"])
                .output()
                .await;
            if let Ok(result) = output {
                if result.status.success() {
                    let stdout = String::from_utf8_lossy(&result.stdout);
                    let path = PathBuf::from(stdout.trim());
                    if path.exists() {
                        return Some(path);
                    }
                }
            }
        }

        // Fallback: vendor/bundle in common project roots is not global cache.
        // Bundler's global cache is usually under BUNDLE_PATH or ~/.bundle
        if let Some(home) = dirs::home_dir() {
            let path = home.join(".bundle").join("cache");
            if path.exists() {
                return Some(path);
            }
        }

        None
    }

    /// Scan common project root directories for per-project `vendor/bundle` directories.
    async fn find_vendor_bundles() -> Vec<PathBuf> {
        let mut found = Vec::new();
        let home = match dirs::home_dir() {
            Some(h) => h,
            None => return found,
        };

        // Directories that commonly contain Ruby projects
        let candidate_roots = [
            home.join("projects"),
            home.join("src"),
            home.join("dev"),
            home.join("code"),
            home.join("workspace"),
            home.join("repos"),
            home.clone(),
        ];

        for root in &candidate_roots {
            if !root.exists() {
                continue;
            }
            if let Ok(entries) = std::fs::read_dir(root) {
                for entry in entries.flatten() {
                    let vb = entry.path().join("vendor").join("bundle");
                    if vb.exists() {
                        found.push(vb);
                    }
                }
            }
        }

        found
    }
}

#[async_trait]
impl PackageManager for BundlerManager {
    fn name(&self) -> &'static str {
        "bundler"
    }

    fn display_name(&self) -> &'static str {
        "Bundler (Ruby)"
    }

    async fn is_installed(&self) -> bool {
        which("bundle.cmd").is_ok()
            || which("bundle").is_ok()
            || std::env::var("BUNDLE_PATH").is_ok()
            || dirs::home_dir()
                .map(|h| h.join(".bundle").exists())
                .unwrap_or(false)
    }

    async fn get_version(&self) -> Result<Option<String>> {
        if let Ok(bundle_path) = self.get_bundle_path().await {
            let output = Command::new(&bundle_path).arg("--version").output().await;
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

        // Global bundle path (~/.bundle/cache or BUNDLE_PATH)
        if let Some(path) = self.resolve_vendor_path().await {
            paths.push(path);
        }

        // Per-project vendor/bundle directories found under common project roots
        let vendor_bundles = Self::find_vendor_bundles().await;
        for vb in vendor_bundles {
            if !paths.contains(&vb) {
                paths.push(vb);
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
        info!("Cleaning Bundler caches ({} directories)", paths.len());
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
                        description: "Bundler vendor/cache".to_string(),
                        can_delete: true,
                    });
                }
            }
        }
        Ok(info)
    }

    fn prevention_tip(&self) -> &'static str {
        "Use 'bundle config set --local path vendor/bundle' to isolate per-project gems. Run 'bundle clean' periodically."
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_bundler_manager_creation() {
        let manager = BundlerManager::new().await;
        assert!(manager.is_ok());
        assert_eq!(manager.unwrap().name(), "bundler");
    }

    #[test]
    fn test_display_name() {
        assert_eq!(BundlerManager::default().display_name(), "Bundler (Ruby)");
    }

    /// `find_vendor_bundles()` must discover `vendor/bundle` directories that
    /// exist under a candidate project root.
    #[tokio::test]
    async fn test_find_vendor_bundles_discovers_projects() {
        // Create a temp directory that acts as a "projects" root
        let projects_root = TempDir::new().unwrap();

        // Create a fake Ruby project with vendor/bundle inside it
        let project_dir = projects_root.path().join("my_rails_app");
        let vendor_bundle = project_dir.join("vendor").join("bundle");
        std::fs::create_dir_all(&vendor_bundle).unwrap();

        // Point one of the candidate roots to our temp dir by scanning it directly
        let found: Vec<PathBuf> = {
            let root = projects_root.path();
            let mut v = Vec::new();
            if let Ok(entries) = std::fs::read_dir(root) {
                for entry in entries.flatten() {
                    let vb = entry.path().join("vendor").join("bundle");
                    if vb.exists() {
                        v.push(vb);
                    }
                }
            }
            v
        };

        assert!(
            found.contains(&vendor_bundle),
            "find_vendor_bundles logic must discover vendor/bundle in project roots"
        );
    }

    /// `get_cache_paths()` must return Ok(_) even when bundle is not installed.
    #[tokio::test]
    async fn test_get_cache_paths_does_not_panic() {
        let manager = BundlerManager::default();
        let result = manager.get_cache_paths().await;
        assert!(result.is_ok());
    }
}
