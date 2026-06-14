//! conda / mamba package manager implementation

use super::{
    calculate_directory_size, safe_delete_directory, CacheInfo, PackageCleanResult, PackageManager,
};
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::process::Command;
use tracing::info;
use which::which;

/// Conda / Mamba cache manager
#[derive(Default)]
pub struct CondaManager {
    conda_path: Option<PathBuf>,
}

impl CondaManager {
    pub async fn new() -> Result<Self> {
        Ok(Self { conda_path: None })
    }

    async fn get_conda_path(&self) -> Result<PathBuf> {
        if let Some(ref path) = self.conda_path {
            return Ok(path.clone());
        }
        for cmd in ["conda.exe", "conda", "mamba.exe", "mamba"] {
            if which(cmd).is_ok() {
                return Ok(PathBuf::from(cmd));
            }
        }
        anyhow::bail!("conda/mamba not found in PATH")
    }

    fn resolve_pkgs_dirs() -> Vec<PathBuf> {
        let mut dirs = Vec::new();

        // Common Anaconda/Miniconda install locations on Windows
        let bases = [
            r"C:\ProgramData\anaconda3\pkgs",
            r"C:\ProgramData\miniconda3\pkgs",
            r"C:\Users\%USERNAME%\.conda\pkgs",
        ];
        for b in &bases {
            let expanded = shellexpand::full(b).unwrap_or_default().into_owned();
            let p = PathBuf::from(&expanded);
            if p.exists() {
                dirs.push(p);
            }
        }

        // Try CONDA_PKGS_DIRS
        if let Ok(pkgs_dirs) = std::env::var("CONDA_PKGS_DIRS") {
            for part in pkgs_dirs.split(';') {
                let p = PathBuf::from(part.trim());
                if p.exists() {
                    dirs.push(p);
                }
            }
        }

        // Try HOME\.conda\pkgs
        if let Some(home) = dirs::home_dir() {
            let p = home.join(".conda").join("pkgs");
            if p.exists() {
                dirs.push(p);
            }
        }

        dirs
    }
}

#[async_trait]
impl PackageManager for CondaManager {
    fn name(&self) -> &'static str {
        "conda"
    }

    fn display_name(&self) -> &'static str {
        "conda / mamba"
    }

    async fn is_installed(&self) -> bool {
        which("conda.exe").is_ok()
            || which("conda").is_ok()
            || which("mamba.exe").is_ok()
            || which("mamba").is_ok()
            || !Self::resolve_pkgs_dirs().is_empty()
    }

    async fn get_version(&self) -> Result<Option<String>> {
        if let Ok(conda_path) = self.get_conda_path().await {
            let output = Command::new(&conda_path).arg("--version").output().await;
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
        Ok(Self::resolve_pkgs_dirs())
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
        info!("Cleaning conda caches ({} directories)", paths.len());

        // Attempt `conda clean --all --yes` first
        if let Ok(conda_path) = self.get_conda_path().await {
            let _ = Command::new(&conda_path)
                .args(["clean", "--all", "--yes"])
                .output()
                .await;
        }

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
                        description: "conda / mamba packages cache".to_string(),
                        can_delete: true,
                    });
                }
            }
        }
        Ok(info)
    }

    fn prevention_tip(&self) -> &'static str {
        "Run 'conda clean --all' after environment changes. Pin exact versions in environment.yml to avoid redundant downloads."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_conda_manager_creation() {
        let manager = CondaManager::new().await;
        assert!(manager.is_ok());
        assert_eq!(manager.unwrap().name(), "conda");
    }

    #[test]
    fn test_display_name() {
        assert_eq!(CondaManager::default().display_name(), "conda / mamba");
    }
}
