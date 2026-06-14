//! Maven build tool cache cleanup

use super::{
    calculate_directory_size, safe_delete_directory, CacheInfo, PackageCleanResult, PackageManager,
};
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tracing::{debug, info, warn};
use which::which;

/// Maven build tool manager
#[derive(Default)]
pub struct MavenManager;

impl MavenManager {
    pub async fn new() -> Result<Self> {
        Ok(Self)
    }

    fn local_repo() -> PathBuf {
        if let Ok(m2_home) = std::env::var("MAVEN_OPTS") {
            // Very rough heuristic; most users rely on default
            let _ = m2_home;
        }
        dirs::home_dir()
            .unwrap_or_default()
            .join(".m2")
            .join("repository")
    }
}

#[async_trait]
impl PackageManager for MavenManager {
    fn name(&self) -> &'static str {
        "maven"
    }
    fn display_name(&self) -> &'static str {
        "Apache Maven"
    }

    async fn is_installed(&self) -> bool {
        which("mvn").is_ok() || which("mvn.cmd").is_ok() || Self::local_repo().exists()
    }

    async fn get_version(&self) -> Result<Option<String>> {
        if let Ok(output) = tokio::process::Command::new("mvn")
            .arg("--version")
            .output()
            .await
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                return Ok(text.lines().next().map(|l| l.trim().to_string()));
            }
        }
        Ok(None)
    }

    async fn get_cache_paths(&self) -> Result<Vec<PathBuf>> {
        let m2 = dirs::home_dir().unwrap_or_default().join(".m2");
        let mut paths = vec![m2.join("repository"), m2.join("wrapper")];
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

        info!("Cleaning Maven local repository");
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
        let m2 = dirs::home_dir().unwrap_or_default().join(".m2");
        let mut info = Vec::new();

        let repo = m2.join("repository");
        if repo.exists() {
            let size = calculate_directory_size(&repo).await.unwrap_or(0);
            info.push(CacheInfo {
                path: repo,
                size_bytes: size,
                description: "Maven local repository (~/.m2/repository)".to_string(),
                can_delete: true,
            });
        }

        let wrapper = m2.join("wrapper");
        if wrapper.exists() {
            let size = calculate_directory_size(&wrapper).await.unwrap_or(0);
            info.push(CacheInfo {
                path: wrapper,
                size_bytes: size,
                description: "Maven wrapper distributions".to_string(),
                can_delete: true,
            });
        }

        Ok(info)
    }

    fn prevention_tip(&self) -> &'static str {
        "Use 'mvn dependency:purge-local-repository' to remove unused artifacts. Set localRepository to a shared path in settings.xml."
    }
}
