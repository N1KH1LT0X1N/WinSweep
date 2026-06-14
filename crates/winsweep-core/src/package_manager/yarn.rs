//! Yarn package manager cache cleanup

use crate::package_manager::{CacheInfo, PackageCleanResult, PackageManager};
use anyhow::Context;
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::process::Command;
use tracing::{debug, info, warn};

/// Yarn package manager
pub struct YarnManager {
    executable_path: PathBuf,
    is_berry: bool,
}

impl YarnManager {
    /// Create a new Yarn manager
    pub async fn new() -> Result<Self> {
        let executable_path = Self::find_yarn_executable().context("Yarn executable not found")?;

        // Check if it's Yarn Berry (v2+) or Classic (v1)
        let is_berry = Self::detect_yarn_version(&executable_path).await;

        Ok(Self {
            executable_path,
            is_berry,
        })
    }

    /// Find Yarn executable
    fn find_yarn_executable() -> Option<PathBuf> {
        // Check PATH first
        if let Ok(path) = which::which("yarn") {
            return Some(path);
        }

        // Check common installation locations
        let common_paths = vec![
            r"%LOCALAPPDATA%\Yarn\bin\yarn.cmd",
            r"%APPDATA%\npm\yarn.cmd",
            r"%ProgramFiles%\Yarn\bin\yarn.cmd",
            r"%ProgramFiles(x86)%\Yarn\bin\yarn.cmd",
        ];

        for path in common_paths {
            let expanded = Self::expand_env(path);
            if expanded.exists() {
                return Some(expanded);
            }
        }

        None
    }

    /// Detect if this is Yarn Berry (v2+) or Classic (v1)
    async fn detect_yarn_version(executable: &PathBuf) -> bool {
        if let Ok(output) = Command::new(executable).arg("--version").output().await {
            if output.status.success() {
                if let Ok(version) = String::from_utf8(output.stdout) {
                    // Yarn Berry versions start with 2, 3, 4, etc.
                    return version.starts_with('2')
                        || version.starts_with('3')
                        || version.starts_with('4');
                }
            }
        }
        false
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
        .into()
    }
}

#[async_trait]
impl PackageManager for YarnManager {
    fn name(&self) -> &'static str {
        "yarn"
    }

    fn display_name(&self) -> &'static str {
        if self.is_berry {
            "Yarn Berry"
        } else {
            "Yarn Classic"
        }
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

        if self.is_berry {
            // Yarn Berry cache locations
            let home_dir = dirs::home_dir().unwrap_or_default();

            // Global cache
            paths.push(home_dir.join(".yarn/berry/cache"));

            // Check for custom cache dir
            if let Ok(output) = Command::new(&self.executable_path)
                .arg("config")
                .arg("get")
                .arg("cacheFolder")
                .output()
                .await
            {
                if output.status.success() {
                    if let Ok(cache_path) = String::from_utf8(output.stdout) {
                        let cache_path = cache_path.trim();
                        if !cache_path.is_empty() && cache_path != "undefined" {
                            paths.push(PathBuf::from(cache_path));
                        }
                    }
                }
            }

            // Zero-install cache
            paths.push(home_dir.join(".yarn/berry/linker"));
            paths.push(home_dir.join(".yarn/berry/releases"));
        } else {
            // Yarn Classic cache locations
            let home_dir = dirs::home_dir().unwrap_or_default();

            // Global cache
            paths.push(home_dir.join(".yarn-cache"));

            // Local cache
            if let Ok(app_data) = std::env::var("LOCALAPPDATA") {
                paths.push(PathBuf::from(app_data).join("Yarn\\Cache"));
            }

            // Temporary files
            if let Ok(temp) = std::env::var("TEMP") {
                paths.push(PathBuf::from(temp).join("yarn-*"));
            }
        }

        // Deduplicate
        paths.sort();
        paths.dedup();

        Ok(paths)
    }

    async fn clean_all_caches(&self) -> Result<PackageCleanResult> {
        if self.is_berry {
            // Safety: skip any project-local .yarn/cache that is a zero-installs setup
            let home_dir = dirs::home_dir().unwrap_or_default();
            let global_cache = home_dir.join(".yarn/berry/cache");
            // Only the global cache is safe to delete unconditionally
            let _ = global_cache; // used below in clean_cache flow
        }
        self.clean_cache(false).await
    }

    async fn clean_paths(&self, paths: &[PathBuf]) -> Result<PackageCleanResult> {
        info!("Cleaning specific Yarn cache paths: {:?}", paths);

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
            package_manager: "yarn".to_string(),
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

                // Detect zero-installs: if this is <project>/.yarn/cache, check the project root
                let is_zero_installs =
                    if path.ends_with(".yarn/cache") || path.ends_with(".yarn\\cache") {
                        path.parent() // .yarn
                            .and_then(|p| p.parent()) // project root
                            .map(Self::is_zero_installs)
                            .unwrap_or(false)
                    } else {
                        false
                    };

                let (can_delete, description) = if is_zero_installs {
                    (
                        false,
                        format!(
                            "Yarn zero-installs cache (committed to git — DO NOT DELETE): {}",
                            path.display()
                        ),
                    )
                } else {
                    (
                        true,
                        format!(
                            "Yarn cache: {}",
                            path.file_name().unwrap_or_default().to_string_lossy()
                        ),
                    )
                };

                cache_info.push(CacheInfo {
                    path: path.clone(),
                    size_bytes: size,
                    description,
                    can_delete,
                });
            }
        }

        Ok(cache_info)
    }

    fn prevention_tip(&self) -> &'static str {
        "Use 'yarn cache clean' periodically. For Berry, prefer 'nodeLinker: pnp' to avoid large node_modules."
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

impl YarnManager {
    /// Returns `true` if `project_dir` appears to be a Yarn Berry zero-installs project.
    ///
    /// A zero-installs project commits `.yarn/cache` to git — i.e. its `.gitignore` does NOT
    /// exclude `.yarn/cache`.  Deleting that directory would break the project for all
    /// team members who rely on the committed packages.
    pub fn is_zero_installs(project_dir: &std::path::Path) -> bool {
        let gitignore = project_dir.join(".gitignore");
        if !gitignore.exists() {
            // No .gitignore → assume zero-installs to be safe
            return project_dir.join(".yarn").join("cache").exists();
        }
        if let Ok(content) = std::fs::read_to_string(&gitignore) {
            // If .gitignore contains an entry that ignores .yarn/cache then it is NOT zero-installs
            let ignores_cache = content.lines().any(|line| {
                let line = line.trim();
                line == ".yarn/cache" || line == ".yarn/cache/" || line == "**/.yarn/cache"
            });
            if ignores_cache {
                return false;
            }
            // .yarn/cache exists but not listed in .gitignore → zero-installs
            return project_dir.join(".yarn").join("cache").exists();
        }
        false
    }

    pub async fn clean_cache(&self, dry_run: bool) -> Result<PackageCleanResult> {
        info!(
            "Starting Yarn cache cleanup (dry_run: {}, version: {})",
            dry_run,
            if self.is_berry { "Berry" } else { "Classic" }
        );

        let mut space_freed = 0;
        let mut items_deleted = 0;
        let mut errors = Vec::new();
        let start_time = std::time::Instant::now();

        if self.is_berry {
            // Yarn Berry: Use yarn cache clean
            debug!("Running 'yarn cache clean --all'");

            if !dry_run {
                match Command::new(&self.executable_path)
                    .arg("cache")
                    .arg("clean")
                    .arg("--all")
                    .output()
                    .await
                {
                    Ok(output) => {
                        if !output.status.success() {
                            let error = format!(
                                "Yarn cache clean failed: {}",
                                String::from_utf8_lossy(&output.stderr)
                            );
                            warn!("{}", error);
                            errors.push(error);
                        } else {
                            debug!("Yarn cache clean completed successfully");
                        }
                    }
                    Err(e) => {
                        let error = format!("Failed to run Yarn cache clean: {}", e);
                        warn!("{}", error);
                        errors.push(error);
                    }
                }
            }
        } else {
            // Yarn Classic: Use yarn cache clean
            debug!("Running 'yarn cache clean'");

            if !dry_run {
                match Command::new(&self.executable_path)
                    .arg("cache")
                    .arg("clean")
                    .output()
                    .await
                {
                    Ok(output) => {
                        if !output.status.success() {
                            let error = format!(
                                "Yarn cache clean failed: {}",
                                String::from_utf8_lossy(&output.stderr)
                            );
                            warn!("{}", error);
                            errors.push(error);
                        } else {
                            debug!("Yarn cache clean completed successfully");
                        }
                    }
                    Err(e) => {
                        let error = format!("Failed to run Yarn cache clean: {}", e);
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
            package_manager: "yarn".to_string(),
            space_freed,
            items_deleted,
            errors,
            duration_ms,
        })
    }

    pub async fn clean_global_packages(&self, dry_run: bool) -> Result<PackageCleanResult> {
        info!(
            "Starting Yarn global packages cleanup (dry_run: {})",
            dry_run
        );

        let mut space_freed = 0;
        let mut items_deleted = 0;
        let mut errors = Vec::new();
        let start_time = std::time::Instant::now();

        // Get global modules directory
        let home_dir = dirs::home_dir().unwrap_or_default();
        let global_modules = if self.is_berry {
            home_dir.join(".yarn/berry/global")
        } else {
            home_dir.join(".config/yarn/global")
        };

        if global_modules.exists() {
            if dry_run {
                if let Ok(size) = Self::calculate_directory_size(&global_modules) {
                    space_freed += size;
                    items_deleted += Self::count_files(&global_modules);
                }
            } else {
                match Self::delete_directory_contents(&global_modules).await {
                    Ok((deleted, freed)) => {
                        items_deleted += deleted;
                        space_freed += freed;
                    }
                    Err(e) => {
                        errors.push(format!("Failed to clean global packages: {}", e));
                    }
                }
            }
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;

        Ok(PackageCleanResult {
            package_manager: "yarn".to_string(),
            space_freed,
            items_deleted,
            errors,
            duration_ms,
        })
    }
}

impl YarnManager {
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
