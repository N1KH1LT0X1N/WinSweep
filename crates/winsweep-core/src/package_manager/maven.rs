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
        Self::resolve_local_repo(dirs::home_dir())
    }

    /// Inner logic extracted so tests can supply a custom home directory.
    fn resolve_local_repo(home: Option<PathBuf>) -> PathBuf {
        // 1. Some CI systems expose MAVEN_LOCAL_REPO directly.
        if let Ok(repo) = std::env::var("MAVEN_LOCAL_REPO") {
            let p = PathBuf::from(&repo);
            if p.is_absolute() {
                return p;
            }
        }

        // 2. Parse ~/.m2/settings.xml for a custom <localRepository> element.
        //    We do a lightweight text scan to avoid pulling in an XML crate.
        if let Some(ref home) = home {
            let settings = home.join(".m2").join("settings.xml");
            if let Ok(content) = std::fs::read_to_string(&settings) {
                for line in content.lines() {
                    let line = line.trim();
                    if let Some(inner) = line
                        .strip_prefix("<localRepository>")
                        .and_then(|s| s.strip_suffix("</localRepository>"))
                    {
                        let trimmed = inner.trim();
                        if !trimmed.is_empty() {
                            let p = PathBuf::from(trimmed);
                            if p.is_absolute() {
                                return p;
                            }
                            // Resolve relative paths against the home directory
                            return home.join(trimmed);
                        }
                    }
                }
            }
            // 3. Default: ~/.m2/repository
            home.join(".m2").join("repository")
        } else {
            PathBuf::from(".m2").join("repository")
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_maven_manager_creation() {
        let manager = MavenManager::new().await;
        assert!(manager.is_ok());
        assert_eq!(manager.unwrap().name(), "maven");
    }

    #[test]
    fn test_display_name() {
        assert_eq!(MavenManager.display_name(), "Apache Maven");
    }

    /// MAVEN_LOCAL_REPO env var must override the default ~/.m2/repository path.
    #[test]
    fn test_local_repo_from_maven_local_repo_env() {
        let dir = TempDir::new().unwrap();
        let expected = dir.path().to_path_buf();
        std::env::set_var("MAVEN_LOCAL_REPO", expected.to_str().unwrap());
        let result = MavenManager::local_repo();
        std::env::remove_var("MAVEN_LOCAL_REPO");
        assert_eq!(result, expected);
    }

    /// When settings.xml contains a <localRepository> element the path must be used.
    #[test]
    fn test_local_repo_from_settings_xml() {
        let home_dir = TempDir::new().unwrap();
        let m2_dir = home_dir.path().join(".m2");
        std::fs::create_dir_all(&m2_dir).unwrap();

        let custom_repo = home_dir.path().join("my-repo");
        std::fs::create_dir_all(&custom_repo).unwrap();

        let settings_path = m2_dir.join("settings.xml");
        let mut f = std::fs::File::create(&settings_path).unwrap();
        writeln!(
            f,
            "<settings>\n  <localRepository>{}</localRepository>\n</settings>",
            custom_repo.display()
        )
        .unwrap();

        // Pass the temp home directly — no env var manipulation needed
        std::env::remove_var("MAVEN_LOCAL_REPO");
        let result = MavenManager::resolve_local_repo(Some(home_dir.path().to_path_buf()));
        assert_eq!(result, custom_repo);
    }

    /// When neither env var nor settings.xml is present the default path is returned.
    #[test]
    fn test_local_repo_default() {
        std::env::remove_var("MAVEN_LOCAL_REPO");
        let result = MavenManager::local_repo();
        assert!(
            result.ends_with(std::path::Path::new(".m2/repository")),
            "default must end with .m2/repository, got: {}",
            result.display()
        );
    }
}
