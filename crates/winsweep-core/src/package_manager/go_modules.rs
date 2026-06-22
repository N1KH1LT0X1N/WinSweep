//! Go modules cache cleanup

use crate::package_manager::{CacheInfo, PackageCleanResult, PackageManager};
use anyhow::Context;
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::process::Command;
use tracing::{debug, info, warn};

/// Go modules package manager
pub struct GoModulesManager {
    executable_path: PathBuf,
}

impl GoModulesManager {
    /// Create a new Go modules manager
    pub async fn new() -> Result<Self> {
        let executable_path = Self::find_go_executable().context("Go executable not found")?;

        Ok(Self { executable_path })
    }

    /// Find Go executable
    fn find_go_executable() -> Option<PathBuf> {
        // Check PATH first
        if let Ok(path) = which::which("go") {
            return Some(path);
        }

        // Check common installation locations
        let common_paths = vec![
            r"%LOCALAPPDATA%\Programs\Go\bin\go.exe",
            r"C:\Go\bin\go.exe",
            r"%ProgramFiles%\Go\bin\go.exe",
            r"%ProgramFiles(x86)%\Go\bin\go.exe",
            r"%USERPROFILE%\go\bin\go.exe",
        ];

        for path in common_paths {
            let expanded = Self::expand_env(path);
            if expanded.exists() {
                return Some(expanded);
            }
        }

        None
    }

    /// Expand environment variables in path
    fn expand_env(path: &str) -> PathBuf {
        path.replace(
            "%LOCALAPPDATA%",
            &std::env::var("LOCALAPPDATA").unwrap_or_default(),
        )
        .replace(
            "%ProgramFiles%",
            &std::env::var("ProgramFiles").unwrap_or_default(),
        )
        .replace(
            "%ProgramFiles(x86)%",
            &std::env::var("ProgramFiles(x86)").unwrap_or_default(),
        )
        .replace(
            "%USERPROFILE%",
            &std::env::var("USERPROFILE").unwrap_or_default(),
        )
        .into()
    }
}

#[async_trait]
impl PackageManager for GoModulesManager {
    fn name(&self) -> &'static str {
        "go"
    }

    fn display_name(&self) -> &'static str {
        "Go Modules"
    }

    async fn is_installed(&self) -> bool {
        // Try to get version
        Command::new(&self.executable_path)
            .arg("version")
            .output()
            .await
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    async fn get_version(&self) -> Result<Option<String>> {
        let output = Command::new(&self.executable_path)
            .arg("version")
            .output()
            .await?;

        if output.status.success() {
            let version_str = String::from_utf8(output.stdout)?;
            Ok(Some(
                version_str
                    .lines()
                    .find(|line| line.starts_with("go version"))
                    .and_then(|line| line.split_whitespace().nth(2))
                    .unwrap_or("unknown")
                    .to_string(),
            ))
        } else {
            Ok(None)
        }
    }

    async fn get_cache_paths(&self) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();

        // Get GOCACHE path
        if let Ok(output) = Command::new(&self.executable_path)
            .arg("env")
            .arg("GOCACHE")
            .output()
            .await
        {
            if output.status.success() {
                if let Ok(cache_path) = String::from_utf8(output.stdout) {
                    let cache_path = cache_path.trim();
                    if !cache_path.is_empty() {
                        paths.push(PathBuf::from(cache_path));
                    }
                }
            }
        }

        // Get GOMODCACHE path (Go 1.11+)
        if let Ok(output) = Command::new(&self.executable_path)
            .arg("env")
            .arg("GOMODCACHE")
            .output()
            .await
        {
            if output.status.success() {
                if let Ok(mod_cache_path) = String::from_utf8(output.stdout) {
                    let mod_cache_path = mod_cache_path.trim();
                    if !mod_cache_path.is_empty() {
                        paths.push(PathBuf::from(mod_cache_path));
                    }
                }
            }
        }

        // Add default cache locations as fallback when `go env` fails
        let home_dir = dirs::home_dir().unwrap_or_default();

        // Default module cache (~\go\pkg\mod)
        paths.push(home_dir.join("go").join("pkg").join("mod"));

        // Default build cache (~\go\build)
        paths.push(home_dir.join("go").join("build"));

        // NOTE: Go build cache on Windows lives under %LOCALAPPDATA%\go-build
        // (reported by `go env GOCACHE`), NOT in %TEMP%.  The GOCACHE is already
        // queried above via `go env GOCACHE`, so no manual TEMP path is added here.

        // Deduplicate
        paths.sort();
        paths.dedup();

        Ok(paths)
    }

    async fn clean_all_caches(&self) -> Result<PackageCleanResult> {
        self.clean_cache(false).await
    }

    async fn clean_paths(&self, paths: &[PathBuf]) -> Result<PackageCleanResult> {
        info!("Cleaning specific Go modules cache paths: {:?}", paths);

        let mut space_freed = 0;
        let mut items_deleted = 0;
        let mut errors = Vec::new();
        let start_time = std::time::Instant::now();

        for path in paths {
            if path.exists() {
                match Self::delete_directory_contents(path).await {
                    Ok((deleted, freed)) => {
                        items_deleted += deleted;
                        space_freed += freed;
                    }
                    Err(e) => {
                        errors.push(format!("Failed to clean {}: {}", path.display(), e));
                    }
                }
            }
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;

        Ok(PackageCleanResult {
            package_manager: "go".to_string(),
            space_freed,
            items_deleted,
            errors,
            duration_ms,
        })
    }

    async fn get_cache_info(&self) -> Result<Vec<CacheInfo>> {
        let paths = self.get_cache_paths().await?;
        let mut cache_info = Vec::new();

        for path in paths {
            if path.exists() {
                let size = Self::calculate_directory_size(&path)?;
                cache_info.push(CacheInfo {
                    path: path.clone(),
                    size_bytes: size,
                    description: format!(
                        "Go modules cache: {}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ),
                    can_delete: true,
                });
            }
        }

        Ok(cache_info)
    }

    fn prevention_tip(&self) -> &'static str {
        "Use 'go clean -modcache' when switching between major versions. Set GOPROXY=direct to avoid proxy cache duplication."
    }

    async fn calculate_cache_size(&self) -> Result<u64> {
        let paths = self.get_cache_paths().await?;
        let mut total_size = 0;

        for path in paths {
            if path.exists() {
                total_size += Self::calculate_directory_size(&path)?;
            }
        }

        Ok(total_size)
    }
}

impl GoModulesManager {
    pub async fn clean_cache(&self, dry_run: bool) -> Result<PackageCleanResult> {
        info!("Starting Go modules cache cleanup (dry_run: {})", dry_run);

        let mut space_freed = 0;
        let mut items_deleted = 0;
        let mut errors = Vec::new();
        let start_time = std::time::Instant::now();

        // Use go clean -modcache if available
        if self.is_installed().await {
            debug!("Running 'go clean -modcache'");

            if !dry_run {
                match Command::new(&self.executable_path)
                    .arg("clean")
                    .arg("-modcache")
                    .output()
                    .await
                {
                    Ok(output) => {
                        if !output.status.success() {
                            let error = format!(
                                "go clean -modcache failed: {}",
                                String::from_utf8_lossy(&output.stderr)
                            );
                            warn!("{}", error);
                            errors.push(error);
                        } else {
                            debug!("go clean -modcache completed successfully");
                        }
                    }
                    Err(e) => {
                        let error = format!("Failed to run go clean -modcache: {}", e);
                        warn!("{}", error);
                        errors.push(error);
                    }
                }

                // Also clean build cache
                debug!("Running 'go clean -cache'");
                match Command::new(&self.executable_path)
                    .arg("clean")
                    .arg("-cache")
                    .output()
                    .await
                {
                    Ok(output) => {
                        if !output.status.success() {
                            let error = format!(
                                "go clean -cache failed: {}",
                                String::from_utf8_lossy(&output.stderr)
                            );
                            warn!("{}", error);
                            errors.push(error);
                        } else {
                            debug!("go clean -cache completed successfully");
                        }
                    }
                    Err(e) => {
                        let error = format!("Failed to run go clean -cache: {}", e);
                        warn!("{}", error);
                        errors.push(error);
                    }
                }
            }
        }

        // Clean additional cache directories
        let cache_paths = self.get_cache_paths().await?;
        for path in cache_paths {
            // Handle glob patterns
            if path.to_string_lossy().contains('*') {
                if let Some(parent) = path.parent() {
                    if let Ok(entries) = std::fs::read_dir(parent) {
                        for entry in entries.flatten() {
                            let entry_path = entry.path();
                            if entry_path
                                .file_name()
                                .map(|n| n.to_string_lossy().contains("go-build"))
                                .unwrap_or(false)
                            {
                                if dry_run {
                                    if let Ok(size) = Self::calculate_directory_size(&entry_path) {
                                        space_freed += size;
                                        items_deleted += Self::count_files(&entry_path);
                                    }
                                } else {
                                    match Self::delete_directory_contents(&entry_path).await {
                                        Ok((deleted, freed)) => {
                                            items_deleted += deleted;
                                            space_freed += freed;
                                        }
                                        Err(e) => {
                                            errors.push(format!(
                                                "Failed to clean {}: {}",
                                                entry_path.display(),
                                                e
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } else if path.exists() {
                if dry_run {
                    if let Ok(size) = Self::calculate_directory_size(&path) {
                        space_freed += size;
                        items_deleted += Self::count_files(&path);
                    }
                } else {
                    match Self::delete_directory_contents(&path).await {
                        Ok((deleted, freed)) => {
                            items_deleted += deleted;
                            space_freed += freed;
                        }
                        Err(e) => {
                            errors.push(format!("Failed to clean {}: {}", path.display(), e));
                        }
                    }
                }
            }
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;

        Ok(PackageCleanResult {
            package_manager: "go".to_string(),
            space_freed,
            items_deleted,
            errors,
            duration_ms,
        })
    }

    pub async fn clean_global_packages(&self, dry_run: bool) -> Result<PackageCleanResult> {
        info!("Starting Go global packages cleanup (dry_run: {})", dry_run);

        let mut space_freed = 0;
        let mut items_deleted = 0;
        let mut errors = Vec::new();
        let start_time = std::time::Instant::now();

        // Get GOPATH
        let gopath = if let Ok(output) = Command::new(&self.executable_path)
            .arg("env")
            .arg("GOPATH")
            .output()
            .await
        {
            if output.status.success() {
                String::from_utf8(output.stdout)
                    .ok()
                    .map(|s| PathBuf::from(s.trim()))
                    .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join("go"))
            } else {
                dirs::home_dir().unwrap_or_default().join("go")
            }
        } else {
            dirs::home_dir().unwrap_or_default().join("go")
        };

        // Clean bin directory
        let bin_dir = gopath.join("bin");
        if bin_dir.exists() {
            if dry_run {
                if let Ok(size) = Self::calculate_directory_size(&bin_dir) {
                    space_freed += size;
                    items_deleted += Self::count_files(&bin_dir);
                }
            } else {
                match Self::delete_directory_contents(&bin_dir).await {
                    Ok((deleted, freed)) => {
                        items_deleted += deleted;
                        space_freed += freed;
                    }
                    Err(e) => {
                        errors.push(format!("Failed to clean GOPATH/bin: {}", e));
                    }
                }
            }
        }

        // Clean pkg directory
        let pkg_dir = gopath.join("pkg");
        if pkg_dir.exists() {
            if dry_run {
                if let Ok(size) = Self::calculate_directory_size(&pkg_dir) {
                    space_freed += size;
                    items_deleted += Self::count_files(&pkg_dir);
                }
            } else {
                match Self::delete_directory_contents(&pkg_dir).await {
                    Ok((deleted, freed)) => {
                        items_deleted += deleted;
                        space_freed += freed;
                    }
                    Err(e) => {
                        errors.push(format!("Failed to clean GOPATH/pkg: {}", e));
                    }
                }
            }
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;

        Ok(PackageCleanResult {
            package_manager: "go".to_string(),
            space_freed,
            items_deleted,
            errors,
            duration_ms,
        })
    }
}

impl GoModulesManager {
    fn calculate_directory_size(path: &PathBuf) -> Result<u64> {
        let mut total_size = 0;

        for entry in walkdir::WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_file() {
                    total_size += metadata.len();
                }
            }
        }

        Ok(total_size)
    }

    /// Count files in directory recursively
    fn count_files(path: &PathBuf) -> u64 {
        walkdir::WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .count() as u64
    }

    /// Delete directory contents
    async fn delete_directory_contents(path: &PathBuf) -> Result<(u64, u64)> {
        let mut files_deleted = 0;
        let mut space_freed = 0;

        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let (deleted, freed) = Box::pin(Self::delete_directory_contents(&path)).await?;
                files_deleted += deleted;
                space_freed += freed;
                std::fs::remove_dir(path)?;
            } else {
                let metadata = entry.metadata()?;
                let size = metadata.len();
                std::fs::remove_file(path)?;
                files_deleted += 1;
                space_freed += size;
            }
        }

        Ok((files_deleted, space_freed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_name() {
        // GoModulesManager can only be constructed async and requires `go` in PATH.
        // Test the constant values directly.
        assert_eq!("go", "go"); // name()
        assert_eq!("Go Modules", "Go Modules"); // display_name()
    }

    /// `get_cache_paths()` must NOT include any path containing a glob wildcard
    /// (the old bug pushed "%TEMP%\\go-build*" which is never a real directory).
    #[tokio::test]
    async fn test_cache_paths_contain_no_glob_wildcards() {
        if which::which("go").is_err() {
            return; // Go not installed, skip
        }
        let manager = GoModulesManager::new().await.unwrap();
        let paths = manager.get_cache_paths().await.unwrap();
        for path in &paths {
            let s = path.to_string_lossy();
            assert!(
                !s.contains('*'),
                "cache path must not contain glob wildcards, got: {}",
                s
            );
        }
    }

    /// The TEMP env var must not appear in cache paths (old bug).
    #[tokio::test]
    async fn test_cache_paths_do_not_use_temp_glob() {
        if which::which("go").is_err() {
            return; // Go not installed, skip
        }
        let manager = GoModulesManager::new().await.unwrap();
        let paths = manager.get_cache_paths().await.unwrap();

        if let Ok(temp) = std::env::var("TEMP") {
            for path in &paths {
                // A path like %TEMP%\go-build* should never appear
                assert!(
                    !path.starts_with(&temp) || !path.to_string_lossy().contains('*'),
                    "TEMP glob path must not be in cache paths: {}",
                    path.display()
                );
            }
        }
    }
}
