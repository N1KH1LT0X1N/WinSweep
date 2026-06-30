//! Cargo package manager implementation

use super::{
    calculate_directory_size, format_bytes, safe_delete_directory, CacheInfo, PackageCleanResult,
    PackageManager,
};
use anyhow::Context;
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::process::Command;
use tracing::{debug, info, warn};
use which::which;

/// Cargo package manager
#[derive(Default)]
pub struct CargoManager {
    cargo_path: Option<PathBuf>,
    cargo_home: Option<PathBuf>,
}

impl CargoManager {
    /// Create a new cargo manager
    pub async fn new() -> Result<Self> {
        // Resolve cargo executable eagerly so cargo clean and version queries work.
        let cargo_path = which("cargo.exe").or_else(|_| which("cargo")).ok();
        Ok(Self {
            cargo_path,
            cargo_home: None,
        })
    }

    /// Get Cargo home directory
    async fn get_cargo_home(&self) -> Result<PathBuf> {
        if let Some(ref home) = self.cargo_home {
            return Ok(home.clone());
        }

        // Check CARGO_HOME environment variable
        if let Ok(cargo_home) = std::env::var("CARGO_HOME") {
            return Ok(PathBuf::from(cargo_home));
        }

        // Default to .cargo in home directory
        let home_dir = dirs::home_dir().context("Could not find home directory")?;
        Ok(home_dir.join(".cargo"))
    }

    /// Get registry cache path
    async fn get_registry_cache_path(&self) -> Result<PathBuf> {
        let cargo_home = self.get_cargo_home().await?;
        Ok(cargo_home.join("registry"))
    }

    /// Get git cache path
    async fn get_git_cache_path(&self) -> Result<PathBuf> {
        let cargo_home = self.get_cargo_home().await?;
        Ok(cargo_home.join("git"))
    }

    /// Get cargo executable path
    fn get_cargo_path(&self) -> Result<PathBuf> {
        if let Some(ref path) = self.cargo_path {
            return Ok(path.clone());
        }
        which("cargo.exe")
            .or_else(|_| which("cargo"))
            .context("cargo not found in PATH")
    }

    /// Get target directories
    async fn get_target_paths(&self) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();

        // Global target directory
        let cargo_home = self.get_cargo_home().await?;
        paths.push(cargo_home.join("target"));

        // Common project target directories
        if let Ok(current_dir) = std::env::current_dir() {
            // Check for target in current and parent directories
            let mut dir = current_dir.clone();
            for _ in 0..5 {
                // Check up to 5 levels up
                let target_path = dir.join("target");
                if target_path.exists() {
                    paths.push(target_path);
                }

                if !dir.pop() {
                    break;
                }
            }
        }

        Ok(paths)
    }
}

#[async_trait]
impl PackageManager for CargoManager {
    fn name(&self) -> &'static str {
        "cargo"
    }

    fn display_name(&self) -> &'static str {
        "Rust Package Manager (Cargo)"
    }

    async fn is_installed(&self) -> bool {
        // Check if cargo is in PATH
        which("cargo.exe").is_ok()
    }

    /// Get cargo version
    async fn get_version(&self) -> Result<Option<String>> {
        let cargo_path = self.get_cargo_path()?;

        let output = Command::new(cargo_path).arg("--version").output().await;

        match output {
            Ok(result) if result.status.success() => {
                let version = String::from_utf8_lossy(&result.stdout).trim().to_string();
                Ok(Some(version))
            }
            _ => Ok(None),
        }
    }

    async fn get_cache_paths(&self) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();

        // Registry cache — the primary Cargo cache under $CARGO_HOME/registry
        paths.push(self.get_registry_cache_path().await?);

        // Git cache — $CARGO_HOME/git
        paths.push(self.get_git_cache_path().await?);

        // NOTE: We intentionally do NOT include project-local `target/` directories
        // here.  Walking up from CWD to find them is unreliable (it may return
        // WinSweep's own build output), and project targets should be cleaned
        // through the scanner's artifact-directory detection, not this cleaner.

        Ok(paths)
    }

    async fn calculate_cache_size(&self) -> Result<u64> {
        let paths = self.get_cache_paths().await?;
        let mut total_size = 0u64;

        for path in paths {
            if path.exists() {
                total_size += calculate_directory_size(&path).await?;
            }
        }

        Ok(total_size)
    }

    /// Clean all cargo caches
    async fn clean_all_caches(&self) -> Result<PackageCleanResult> {
        let start_time = std::time::Instant::now();
        let mut space_freed = 0u64;
        let mut items_deleted = 0u64;
        let mut errors = Vec::new();

        info!("Cleaning cargo caches");

        // Recommend cargo-cache if it isn't installed — it provides smarter selective cleanup
        // (e.g. keep the N most recent versions of each crate).
        let has_cargo_cache = which("cargo-cache").is_ok()
            || Command::new("cargo")
                .args(["cache", "--version"])
                .output()
                .await
                .map(|o| o.status.success())
                .unwrap_or(false);

        if !has_cargo_cache {
            warn!(
                "cargo-cache is not installed. For smarter Cargo cache management, run: \
                 cargo install cargo-cache"
            );
            errors.push(
                "cargo-cache not found — install it with `cargo install cargo-cache` \
                 for selective registry cleanup. Falling back to full directory removal."
                    .to_string(),
            );
        } else {
            // Use cargo-cache --autoclean for the registry (keeps all crates used in any local project)
            debug!("Running cargo-cache --autoclean");
            match Command::new("cargo")
                .args(["cache", "--autoclean"])
                .output()
                .await
            {
                Ok(result) if result.status.success() => {
                    debug!("cargo-cache --autoclean succeeded");
                }
                Ok(result) => {
                    warn!(
                        "cargo-cache --autoclean failed: {}",
                        String::from_utf8_lossy(&result.stderr)
                    );
                }
                Err(e) => warn!("Failed to run cargo-cache: {}", e),
            }
        }

        // NOTE: We deliberately do NOT run `cargo clean` here. `cargo clean` only
        // acts on the target directory of whatever crate it is invoked from, which
        // would be WinSweep's *own* current working directory — an unrelated project.
        // Deleting that target/ is a foot-gun, so we rely solely on removing the
        // global registry/git caches under $CARGO_HOME below.

        // Clean cache directories manually
        let paths = self.get_cache_paths().await?;

        for path in paths {
            if path.exists() {
                // Skip target directories that might be in use
                if path.ends_with("target") {
                    debug!("Skipping target directory: {}", path.display());
                    continue;
                }

                debug!("Cleaning Cargo cache directory: {}", path.display());

                match safe_delete_directory(&path).await {
                    Ok(size) => {
                        space_freed += size;
                        items_deleted += 1;
                        debug!(
                            "Deleted Cargo cache: {} (freed {})",
                            path.display(),
                            format_bytes(size)
                        );
                    }
                    Err(e) => {
                        let error = format!("Failed to delete {}: {}", path.display(), e);
                        warn!("{}", error);
                        errors.push(error);
                    }
                }
            }
        }

        Ok(PackageCleanResult {
            package_manager: self.name().to_string(),
            space_freed,
            items_deleted,
            errors,
            duration_ms: start_time.elapsed().as_millis() as u64,
        })
    }

    async fn clean_paths(&self, paths: &[PathBuf]) -> Result<PackageCleanResult> {
        let start_time = std::time::Instant::now();
        let mut space_freed = 0u64;
        let mut items_deleted = 0u64;
        let mut errors = Vec::new();

        for path in paths {
            if path.exists() {
                // Skip target directories
                if path.ends_with("target") {
                    debug!("Skipping target directory: {}", path.display());
                    continue;
                }

                match safe_delete_directory(path).await {
                    Ok(size) => {
                        space_freed += size;
                        items_deleted += 1;
                    }
                    Err(e) => {
                        errors.push(format!("Failed to delete {}: {}", path.display(), e));
                    }
                }
            }
        }

        Ok(PackageCleanResult {
            package_manager: self.name().to_string(),
            space_freed,
            items_deleted,
            errors,
            duration_ms: start_time.elapsed().as_millis() as u64,
        })
    }

    async fn get_cache_info(&self) -> Result<Vec<CacheInfo>> {
        let mut cache_info = Vec::new();

        // Registry cache
        let registry_cache = self.get_registry_cache_path().await?;
        if registry_cache.exists() {
            let size = calculate_directory_size(&registry_cache).await?;
            cache_info.push(CacheInfo {
                path: registry_cache.clone(),
                size_bytes: size,
                description: "Cargo registry cache".to_string(),
                can_delete: true,
            });
        }

        // Git cache
        let git_cache = self.get_git_cache_path().await?;
        if git_cache.exists() {
            let size = calculate_directory_size(&git_cache).await?;
            cache_info.push(CacheInfo {
                path: git_cache.clone(),
                size_bytes: size,
                description: "Cargo git cache".to_string(),
                can_delete: true,
            });
        }

        // Target directories
        for target_path in self.get_target_paths().await? {
            if target_path.exists() {
                let size = calculate_directory_size(&target_path).await?;
                cache_info.push(CacheInfo {
                    path: target_path,
                    size_bytes: size,
                    description: "Cargo build artifacts".to_string(),
                    can_delete: false, // Don't auto-delete target dirs
                });
            }
        }

        Ok(cache_info)
    }

    fn prevention_tip(&self) -> &'static str {
        "Use 'cargo sweep' or set CARGO_TARGET_DIR to a shared directory. Clean old registry versions with 'cargo cache --autoclean'."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cargo_manager_creation() {
        let manager = CargoManager::new().await;
        assert!(manager.is_ok(), "CargoManager::new() must succeed");
        assert_eq!(manager.unwrap().name(), "cargo");
    }

    #[test]
    fn test_display_name() {
        assert_eq!(
            CargoManager::default().display_name(),
            "Rust Package Manager (Cargo)"
        );
    }

    /// cargo is always available inside a `cargo test` run, so `cargo_path`
    /// must be populated after construction.
    #[tokio::test]
    async fn test_cargo_path_initialized() {
        let manager = CargoManager::new().await.unwrap();
        assert!(
            manager.cargo_path.is_some(),
            "cargo_path must be populated when cargo is in PATH (always true inside `cargo test`)"
        );
    }

    /// `get_cargo_path()` must succeed when cargo is in PATH (always true here).
    #[test]
    fn test_get_cargo_path_succeeds() {
        // Use the cached path if available, otherwise fall back to which()
        let manager = CargoManager {
            cargo_path: None,
            cargo_home: None,
        };
        let result = manager.get_cargo_path();
        assert!(result.is_ok(), "get_cargo_path() must find cargo in PATH");
    }

    /// `get_cargo_home()` must return ~/.cargo by default.
    #[tokio::test]
    async fn test_get_cargo_home_default() {
        let manager = CargoManager::default();
        let home = manager.get_cargo_home().await;
        assert!(home.is_ok());
        assert!(
            home.unwrap().ends_with(".cargo"),
            "default cargo home must end with .cargo"
        );
    }

    /// CARGO_HOME env var must be honoured.
    #[tokio::test]
    async fn test_cargo_home_env_var() {
        std::env::set_var("CARGO_HOME", "/tmp/test_cargo_home");
        let manager = CargoManager::default();
        let home = manager.get_cargo_home().await.unwrap();
        std::env::remove_var("CARGO_HOME");
        assert_eq!(home, std::path::PathBuf::from("/tmp/test_cargo_home"));
    }
}
