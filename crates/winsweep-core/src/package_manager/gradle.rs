//! Gradle build tool cache cleanup

use super::{
    calculate_directory_size, safe_delete_directory, CacheInfo, PackageCleanResult, PackageManager,
};
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tracing::{debug, info, warn};
use which::which;

/// Gradle build tool manager
#[derive(Default)]
pub struct GradleManager;

impl GradleManager {
    pub async fn new() -> Result<Self> {
        Ok(Self)
    }

    fn gradle_home() -> PathBuf {
        if let Ok(gradle_home) = std::env::var("GRADLE_HOME") {
            return PathBuf::from(gradle_home);
        }
        dirs::home_dir().unwrap_or_default().join(".gradle")
    }
}

#[async_trait]
impl PackageManager for GradleManager {
    fn name(&self) -> &'static str {
        "gradle"
    }
    fn display_name(&self) -> &'static str {
        "Gradle"
    }

    async fn is_installed(&self) -> bool {
        which("gradle").is_ok()
            || which("gradlew").is_ok()
            || Self::gradle_home().join("wrapper").exists()
    }

    async fn get_version(&self) -> Result<Option<String>> {
        if let Ok(output) = tokio::process::Command::new("gradle")
            .arg("--version")
            .output()
            .await
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                if let Some(line) = text.lines().find(|l| l.starts_with("Gradle")) {
                    return Ok(Some(line.trim().to_string()));
                }
            }
        }
        Ok(None)
    }

    async fn get_cache_paths(&self) -> Result<Vec<PathBuf>> {
        let home = Self::gradle_home();
        let mut paths = vec![
            home.join("caches"),
            home.join("daemon"),
            home.join("wrapper").join("dists"),
            home.join("build-scan-data"),
        ];
        paths.retain(|p| p.exists());
        Ok(paths)
    }

    async fn calculate_cache_size(&self) -> Result<u64> {
        let mut total = 0u64;
        for p in self.get_cache_paths().await? {
            total += calculate_directory_size(&p).await.unwrap_or(0);
        }
        Ok(total)
    }

    async fn clean_all_caches(&self) -> Result<PackageCleanResult> {
        let start = std::time::Instant::now();
        let mut space_freed = 0u64;
        let mut items_deleted = 0u64;
        let mut errors = Vec::new();

        info!("Cleaning Gradle caches");
        for path in self.get_cache_paths().await? {
            debug!("Deleting {}", path.display());
            match safe_delete_directory(&path).await {
                Ok(size) => {
                    space_freed += size;
                    items_deleted += 1;
                }
                Err(e) => {
                    let msg = format!("Failed to delete {}: {}", path.display(), e);
                    warn!("{}", msg);
                    errors.push(msg);
                }
            }
        }

        Ok(PackageCleanResult {
            package_manager: self.name().to_string(),
            space_freed,
            items_deleted,
            errors,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn clean_paths(&self, paths: &[PathBuf]) -> Result<PackageCleanResult> {
        let start = std::time::Instant::now();
        let mut space_freed = 0u64;
        let mut items_deleted = 0u64;
        let mut errors = Vec::new();

        for path in paths {
            if path.exists() {
                match safe_delete_directory(path).await {
                    Ok(size) => {
                        space_freed += size;
                        items_deleted += 1;
                    }
                    Err(e) => errors.push(format!("Failed to delete {}: {}", path.display(), e)),
                }
            }
        }

        Ok(PackageCleanResult {
            package_manager: self.name().to_string(),
            space_freed,
            items_deleted,
            errors,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn get_cache_info(&self) -> Result<Vec<CacheInfo>> {
        let descriptions = [
            ("caches", "Gradle dependency & build caches"),
            ("daemon", "Gradle daemon log files"),
            ("wrapper/dists", "Gradle wrapper distributions"),
            ("build-scan-data", "Gradle build scan data"),
        ];
        let home = Self::gradle_home();
        let mut info = Vec::new();
        for (rel, desc) in &descriptions {
            let p = home.join(rel);
            if p.exists() {
                let size = calculate_directory_size(&p).await.unwrap_or(0);
                info.push(CacheInfo {
                    path: p,
                    size_bytes: size,
                    description: desc.to_string(),
                    can_delete: true,
                });
            }
        }
        Ok(info)
    }

    fn prevention_tip(&self) -> &'static str {
        "Run 'gradle clean' after builds. Disable build scan uploads in CI to avoid build-scan-data growth."
    }
}
