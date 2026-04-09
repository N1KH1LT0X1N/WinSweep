//! pnpm package manager cache cleanup

use anyhow::{Result, Context};
use std::path::PathBuf;
use tokio::process::Command;
use tracing::{debug, info, warn};
use crate::package_manager::{PackageManager, PackageCleanResult};

/// pnpm package manager
pub struct PnpmManager {
    executable_path: PathBuf,
}

impl PnpmManager {
    /// Create a new pnpm manager
    pub async fn new() -> Result<Self> {
        let executable_path = Self::find_pnpm_executable()
            .context("pnpm executable not found")?;
        
        Ok(Self { executable_path })
    }

    /// Find pnpm executable
    fn find_pnpm_executable() -> Option<PathBuf> {
        // Check PATH first
        if let Ok(path) = which::which("pnpm") {
            return Some(path);
        }

        // Check common installation locations
        let common_paths = vec![
            r"%LOCALAPPDATA%\pnpm\pnpm.exe",
            r"%APPDATA%\npm\pnpm.cmd",
            r"%ProgramFiles%\pnpm\pnpm.exe",
            r"%ProgramFiles(x86)%\pnpm\pnpm.exe",
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
        path.replace("%LOCALAPPDATA%", &std::env::var("LOCALAPPDATA").unwrap_or_default())
            .replace("%APPDATA%", &std::env::var("APPDATA").unwrap_or_default())
            .replace("%ProgramFiles%", &std::env::var("ProgramFiles").unwrap_or_default())
            .replace("%ProgramFiles(x86)%", &std::env::var("ProgramFiles(x86)").unwrap_or_default())
            .into()
    }
}

impl PackageManager for PnpmManager {
    fn name(&self) -> &str {
        "pnpm"
    }

    fn display_name(&self) -> &str {
        "pnpm"
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

        // Get pnpm store path
        if let Ok(output) = Command::new(&self.executable_path)
            .arg("store")
            .arg("path")
            .output()
            .await
        {
            if output.status.success() {
                if let Ok(store_path) = String::from_utf8(output.stdout) {
                    let store_path = store_path.trim();
                    if !store_path.is_empty() {
                        paths.push(PathBuf::from(store_path));
                    }
                }
            }
        }

        // Add default cache locations
        let home_dir = dirs::home_dir().unwrap_or_default();
        
        // pnpm global cache
        paths.push(home_dir.join(".pnpm-store"));
        paths.push(home_dir.join(".pnpm-cache"));
        
        // Local cache
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            paths.push(PathBuf::from(local_app_data).join("pnpm"));
        }

        // Temp directories
        if let Ok(temp) = std::env::var("TEMP") {
            paths.push(PathBuf::from(temp).join("pnpm"));
        }

        // Deduplicate
        paths.sort();
        paths.dedup();

        Ok(paths)
    }

    async fn clean_all_caches(&self) -> Result<PackageCleanResult> {
        self.clean_cache(false).await
    }

    async fn clean_paths(&self, paths: &[PathBuf]) -> Result<PackageCleanResult> {
        info!("Cleaning specific pnpm cache paths: {:?}", paths);
        
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
            package_manager: "pnpm".to_string(),
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
                    description: format!("pnpm cache: {}", path.file_name().unwrap_or_default().to_string_lossy()),
                    can_delete: true,
                });
            }
        }

        Ok(cache_info)
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

    async fn clean_cache(&self, dry_run: bool) -> Result<PackageCleanResult> {
        info!("Starting pnpm cache cleanup (dry_run: {})", dry_run);

        let mut space_freed = 0;
        let mut items_deleted = 0;
        let mut errors = Vec::new();
        let start_time = std::time::Instant::now();

        // Use pnpm store prune if available
        if self.is_installed().await {
            debug!("Running 'pnpm store prune'");
            
            if dry_run {
                // For dry run, just calculate what would be deleted
                let cache_paths = self.get_cache_paths().await?;
                for path in cache_paths {
                    if let Ok(size) = Self::calculate_directory_size(&path) {
                        space_freed += size;
                        items_deleted += Self::count_files(&path);
                    }
                }
            } else {
                // Actually run pnpm store prune
                match Command::new(&self.executable_path)
                    .arg("store")
                    .arg("prune")
                    .output()
                {
                    Ok(output) => {
                        if !output.status.success() {
                            let error = format!("pnpm store prune failed: {}", 
                                String::from_utf8_lossy(&output.stderr));
                            warn!("{}", error);
                            errors.push(error);
                        } else {
                            debug!("pnpm store prune completed successfully");
                        }
                    }
                    Err(e) => {
                        let error = format!("Failed to run pnpm store prune: {}", e);
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
            package_manager: "pnpm".to_string(),
            space_freed,
            items_deleted,
            errors,
            duration_ms,
        })
    }

    async fn clean_global_packages(&self, dry_run: bool) -> Result<PackageCleanResult> {
        info!("Starting pnpm global packages cleanup (dry_run: {})", dry_run);

        let mut space_freed = 0;
        let mut items_deleted = 0;
        let mut errors = Vec::new();
        let start_time = std::time::Instant::now();

        // Get global packages directory
        if let Ok(output) = Command::new(&self.executable_path)
            .arg("root")
            .arg("-g")
            .output()
        {
            if output.status.success() {
                if let Ok(root_path) = String::from_utf8(output.stdout) {
                    let global_modules = PathBuf::from(root_path.trim()).join("node_modules");
                    
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
                }
            }
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;

        Ok(PackageCleanResult {
            package_manager: "pnpm".to_string(),
            space_freed,
            items_deleted,
            errors,
            duration_ms,
        })
    }
}

impl PnpmManager {
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
                let (deleted, freed) = Self::delete_directory_contents(&path).await?;
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
