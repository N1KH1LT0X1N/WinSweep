//! Android SDK & AVD cache manager
//!
//! Reports the size of the Android SDK build caches, AVD images, and Gradle
//! build artifacts that are typically safe to delete.

use super::{
    calculate_directory_size, safe_delete_directory, CacheInfo, PackageCleanResult, PackageManager,
};
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tracing::{debug, info, warn};
use which::which;

/// Android SDK / AVD cache manager
#[derive(Default)]
pub struct AndroidSdkManager;

impl AndroidSdkManager {
    pub async fn new() -> Result<Self> {
        Ok(Self)
    }

    fn sdk_root() -> Option<PathBuf> {
        if let Ok(root) = std::env::var("ANDROID_HOME") {
            let p = PathBuf::from(root);
            if p.exists() {
                return Some(p);
            }
        }
        if let Ok(root) = std::env::var("ANDROID_SDK_ROOT") {
            let p = PathBuf::from(root);
            if p.exists() {
                return Some(p);
            }
        }
        // Common Windows location
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            let p = PathBuf::from(local).join("Android").join("Sdk");
            if p.exists() {
                return Some(p);
            }
        }
        None
    }

    fn avd_home() -> PathBuf {
        if let Ok(home) = std::env::var("ANDROID_AVD_HOME") {
            return PathBuf::from(home);
        }
        dirs::home_dir()
            .unwrap_or_default()
            .join(".android")
            .join("avd")
    }
}

#[async_trait]
impl PackageManager for AndroidSdkManager {
    fn name(&self) -> &'static str {
        "android_sdk"
    }
    fn display_name(&self) -> &'static str {
        "Android SDK / AVD"
    }

    async fn is_installed(&self) -> bool {
        which("adb").is_ok() || which("adb.exe").is_ok() || Self::sdk_root().is_some()
    }

    async fn get_version(&self) -> Result<Option<String>> {
        if let Ok(output) = tokio::process::Command::new("adb")
            .arg("version")
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
        let mut paths = Vec::new();

        if let Some(sdk) = Self::sdk_root() {
            // Gradle caches inside the SDK directory
            let gradle_cache = sdk.join(".gradle");
            if gradle_cache.exists() {
                paths.push(gradle_cache);
            }

            // Build-tools and platform-tools temp dirs
            let build_tools = sdk.join("build-tools");
            if build_tools.exists() {
                paths.push(build_tools);
            }
        }

        // ~/.android/cache
        let android_cache = dirs::home_dir()
            .unwrap_or_default()
            .join(".android")
            .join("cache");
        if android_cache.exists() {
            paths.push(android_cache);
        }

        // AVD snapshots (can be huge)
        let avd_home = Self::avd_home();
        if avd_home.exists() {
            // List individual AVD snapshot dirs
            if let Ok(entries) = std::fs::read_dir(&avd_home) {
                for entry in entries.flatten() {
                    let snap = entry.path().join("snapshots");
                    if snap.exists() {
                        paths.push(snap);
                    }
                }
            }
        }

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

        info!("Cleaning Android SDK caches");
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
        let mut info = Vec::new();

        if let Some(sdk) = Self::sdk_root() {
            let build_tools = sdk.join("build-tools");
            if build_tools.exists() {
                let size = calculate_directory_size(&build_tools).await.unwrap_or(0);
                info.push(CacheInfo {
                    path: build_tools,
                    size_bytes: size,
                    description: "Android SDK build-tools".to_string(),
                    can_delete: false,
                });
            }
        }

        let android_cache = dirs::home_dir()
            .unwrap_or_default()
            .join(".android")
            .join("cache");
        if android_cache.exists() {
            let size = calculate_directory_size(&android_cache).await.unwrap_or(0);
            info.push(CacheInfo {
                path: android_cache,
                size_bytes: size,
                description: "Android SDK cache (~/.android/cache)".to_string(),
                can_delete: true,
            });
        }

        // List AVD snapshots
        let avd_home = Self::avd_home();
        if avd_home.exists() {
            if let Ok(entries) = std::fs::read_dir(&avd_home) {
                for entry in entries.flatten() {
                    let snap = entry.path().join("snapshots");
                    if snap.exists() {
                        let size = calculate_directory_size(&snap).await.unwrap_or(0);
                        info.push(CacheInfo {
                            path: snap,
                            size_bytes: size,
                            description: format!(
                                "AVD snapshots: {}",
                                entry.file_name().to_string_lossy()
                            ),
                            can_delete: true,
                        });
                    }
                }
            }
        }

        Ok(info)
    }

    fn prevention_tip(&self) -> &'static str {
        "Delete old AVD snapshots via AVD Manager. Remove obsolete SDK platforms and build-tools with sdkmanager."
    }
}
