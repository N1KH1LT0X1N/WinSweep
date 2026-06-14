//! Poetry package manager cache cleanup

use crate::package_manager::{CacheInfo, PackageCleanResult, PackageManager};
use anyhow::Context;
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::process::Command;
use tracing::{debug, info, warn};

/// Poetry package manager
pub struct PoetryManager {
    executable_path: PathBuf,
}

impl PoetryManager {
    /// Create a new Poetry manager
    pub async fn new() -> Result<Self> {
        let executable_path =
            Self::find_poetry_executable().context("Poetry executable not found")?;

        Ok(Self { executable_path })
    }

    /// Find Poetry executable
    fn find_poetry_executable() -> Option<PathBuf> {
        // Check PATH first
        if let Ok(path) = which::which("poetry") {
            return Some(path);
        }

        // Check common installation locations
        let common_paths = vec![
            r"%LOCALAPPDATA%\Programs\Python\Scripts\poetry.exe",
            r"%APPDATA%\Python\Scripts\poetry.exe",
            r"%ProgramFiles%\Python\Scripts\poetry.exe",
            r"%ProgramFiles(x86)%\Python\Scripts\poetry.exe",
            r"%USERPROFILE%\.poetry\bin\poetry.exe",
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
        .replace("%APPDATA%", &std::env::var("APPDATA").unwrap_or_default())
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
impl PackageManager for PoetryManager {
    fn name(&self) -> &'static str {
        "poetry"
    }

    fn display_name(&self) -> &'static str {
        "Poetry"
    }

    async fn is_installed(&self) -> bool {
        // Try to get version
        Command::new(&self.executable_path)
            .arg("--version")
            .output()
            .await
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    async fn get_version(&self) -> Result<Option<String>> {
        let output = Command::new(&self.executable_path)
            .arg("--version")
            .output()
            .await?;

        if output.status.success() {
            Ok(String::from_utf8(output.stdout)
                .ok()
                .map(|s| s.trim().to_string()))
        } else {
            Ok(None)
        }
    }

    async fn get_cache_paths(&self) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();

        // Get Poetry cache directory
        if let Ok(output) = Command::new(&self.executable_path)
            .arg("config")
            .arg("cache-dir")
            .output()
            .await
        {
            if output.status.success() {
                if let Ok(cache_dir) = String::from_utf8(output.stdout) {
                    let cache_dir = cache_dir.trim();
                    if !cache_dir.is_empty() {
                        paths.push(PathBuf::from(cache_dir));
                    }
                }
            }
        }

        // Add default cache locations
        let home_dir = dirs::home_dir().unwrap_or_default();

        // Poetry v1.2+ cache location
        paths.push(home_dir.join(".cache").join("pypoetry"));

        // Older versions
        paths.push(home_dir.join(".poetry").join("cache"));

        // Virtual environments
        paths.push(home_dir.join(".poetry").join("venv"));

        // Build cache
        paths.push(home_dir.join(".poetry").join("artifacts"));

        // Deduplicate
        paths.sort();
        paths.dedup();

        Ok(paths)
    }

    async fn clean_all_caches(&self) -> Result<PackageCleanResult> {
        self.clean_cache(false).await
    }

    async fn clean_paths(&self, paths: &[PathBuf]) -> Result<PackageCleanResult> {
        info!("Cleaning specific Poetry cache paths: {:?}", paths);

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
            package_manager: "poetry".to_string(),
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
                        "Poetry cache: {}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ),
                    can_delete: true,
                });
            }
        }

        Ok(cache_info)
    }

    fn prevention_tip(&self) -> &'static str {
        "Use 'poetry cache clear pypi --all' after environment rebuilds. Pin versions in pyproject.toml to avoid redundant downloads."
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

impl PoetryManager {
    pub async fn clean_cache(&self, dry_run: bool) -> Result<PackageCleanResult> {
        info!("Starting Poetry cache cleanup (dry_run: {})", dry_run);

        let mut space_freed = 0;
        let mut items_deleted = 0;
        let mut errors = Vec::new();
        let start_time = std::time::Instant::now();

        // Use Poetry cache clear if available
        if self.is_installed().await {
            debug!("Running 'poetry cache clear --all pypi'");

            if !dry_run {
                match Command::new(&self.executable_path)
                    .arg("cache")
                    .arg("clear")
                    .arg("--all")
                    .arg("pypi")
                    .arg("-q")
                    .output()
                    .await
                {
                    Ok(output) => {
                        if !output.status.success() {
                            let error = format!(
                                "Poetry cache clear failed: {}",
                                String::from_utf8_lossy(&output.stderr)
                            );
                            warn!("{}", error);
                            errors.push(error);
                        } else {
                            debug!("Poetry cache clear completed successfully");
                        }
                    }
                    Err(e) => {
                        let error = format!("Failed to run Poetry cache clear: {}", e);
                        warn!("{}", error);
                        errors.push(error);
                    }
                }
            }
        }

        // Clean additional cache directories
        let cache_paths = self.get_cache_paths().await?;
        for path in cache_paths {
            if path.exists() {
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
            package_manager: "poetry".to_string(),
            space_freed,
            items_deleted,
            errors,
            duration_ms,
        })
    }

    pub async fn clean_global_packages(&self, dry_run: bool) -> Result<PackageCleanResult> {
        info!(
            "Starting Poetry global packages cleanup (dry_run: {})",
            dry_run
        );

        let mut space_freed = 0;
        let mut items_deleted = 0;
        let mut errors = Vec::new();
        let start_time = std::time::Instant::now();

        // Poetry doesn't have global packages in the same way as npm
        // But we can clean the global project cache
        let home_dir = dirs::home_dir().unwrap_or_default();
        let global_cache = home_dir.join(".cache").join("pypoetry").join("cache");

        if global_cache.exists() {
            if dry_run {
                if let Ok(size) = Self::calculate_directory_size(&global_cache) {
                    space_freed += size;
                    items_deleted += Self::count_files(&global_cache);
                }
            } else {
                match Self::delete_directory_contents(&global_cache).await {
                    Ok((deleted, freed)) => {
                        items_deleted += deleted;
                        space_freed += freed;
                    }
                    Err(e) => {
                        errors.push(format!("Failed to clean global cache: {}", e));
                    }
                }
            }
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;

        Ok(PackageCleanResult {
            package_manager: "poetry".to_string(),
            space_freed,
            items_deleted,
            errors,
            duration_ms,
        })
    }
}

impl PoetryManager {
    /// Calculate directory size recursively
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
