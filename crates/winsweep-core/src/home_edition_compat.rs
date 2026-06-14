//! Windows Home Edition compatibility layer
//!
//! This module provides fallback implementations for features not available
//! on Windows Home editions, ensuring WinSweep functionality across all editions.

use crate::windows_api::WindowsApi;
use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio::process::Command;
use tracing::{debug, info, warn};

/// Elevation requirement for operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElevationRequirement {
    /// No elevation needed
    None,
    /// Elevation needed on all editions
    Always,
    /// Elevation needed only on Home edition
    HomeOnly,
    /// Elevation not available on Home edition
    HomeNotSupported,
}

/// Feature compatibility information
#[derive(Debug, Clone)]
pub struct FeatureInfo {
    /// Feature name
    pub name: String,
    /// Whether the feature is available
    pub available: bool,
    /// Elevation requirement
    pub elevation: ElevationRequirement,
    /// Reason if not available
    pub unavailable_reason: Option<String>,
    /// Recommended workaround
    pub workaround: Option<String>,
}

/// Home Edition compatibility manager
pub struct HomeEditionCompat {
    windows_api: WindowsApi,
    is_home_edition: bool,
    build_number: u32,
}

impl HomeEditionCompat {
    /// Create a new compatibility manager
    pub fn new() -> Result<Self> {
        let windows_api = WindowsApi::new()?;

        // Detect Windows edition
        let is_home_edition = Self::detect_home_edition(&windows_api)?;

        // Get build number
        let build_number = Self::get_build_number(&windows_api)?;

        info!("Windows Home Edition: {}", is_home_edition);
        info!("Build Number: {}", build_number);

        Ok(Self {
            windows_api,
            is_home_edition,
            build_number,
        })
    }

    /// Check if running on Home edition
    pub fn is_home_edition(&self) -> bool {
        self.is_home_edition
    }

    /// Get build number
    pub fn build_number(&self) -> u32 {
        self.build_number
    }

    /// Get elevation requirement for an operation
    pub fn get_elevation_requirement(&self, operation: &str) -> ElevationRequirement {
        match operation {
            // Operations that never need elevation
            "user_temp_cleanup" | "browser_cache_cleanup" | "recycle_bin_empty" => {
                ElevationRequirement::None
            }

            // Operations that always need elevation
            "windows_update_cleanup" | "system_file_cleanup" | "registry_cleanup" => {
                ElevationRequirement::Always
            }

            // Operations that need elevation only on Home edition
            "system_temp_cleanup" | "prefetch_cleanup" | "service_management"
                if self.is_home_edition =>
            {
                ElevationRequirement::HomeOnly
            }

            // Operations that are not supported on Home edition
            "group_policy_cleanup" if self.is_home_edition => {
                ElevationRequirement::HomeNotSupported
            }

            // Default: no elevation needed
            _ => ElevationRequirement::None,
        }
    }

    /// Check if an operation is supported on the current edition
    pub fn is_operation_supported(&self, operation: &str) -> bool {
        !matches!(
            self.get_elevation_requirement(operation),
            ElevationRequirement::HomeNotSupported
        )
    }

    /// Get feature information for an operation
    pub fn get_feature_info(&self, operation: &str) -> FeatureInfo {
        let elevation = self.get_elevation_requirement(operation);
        let (available, unavailable_reason, workaround) = match operation {
            "group_policy_cleanup" if self.is_home_edition => (
                false,
                Some("Group Policy Editor is not available on Windows Home edition".to_string()),
                Some("Use registry edits or third-party tools like Policy Plus".to_string()),
            ),
            "windows_update_cleanup" if self.is_home_edition => (
                true,
                None,
                Some("Requires administrator privileges".to_string()),
            ),
            "service_management" if self.is_home_edition => (
                true,
                None,
                Some("Limited service management on Home edition".to_string()),
            ),
            _ => (true, None, None),
        };

        FeatureInfo {
            name: operation.to_string(),
            available,
            elevation,
            unavailable_reason,
            workaround,
        }
    }

    /// Compact WSL2 VHDX using appropriate method
    pub async fn compact_wsl_vhdx(&self, distribution: &str) -> Result<WslCompactResult> {
        info!("Compacting WSL2 VHDX for distribution: {}", distribution);

        // Method 1: Try wsl --manage if available
        if self.try_wsl_manage_compact(distribution).await? {
            return Ok(WslCompactResult {
                method: WslCompactMethod::WslManage,
                success: true,
                message: "Successfully compacted using wsl --manage".to_string(),
            });
        }

        // Method 2: Try wslconfig.exe
        if self.try_wslconfig_compact(distribution).await? {
            return Ok(WslCompactResult {
                method: WslCompactMethod::Wslconfig,
                success: true,
                message: "Successfully compacted using wslconfig.exe".to_string(),
            });
        }

        // Method 3: Use diskpart directly (Home edition fallback)
        if self.try_diskpart_compact(distribution).await? {
            return Ok(WslCompactResult {
                method: WslCompactMethod::Diskpart,
                success: true,
                message: "Successfully compacted using diskpart".to_string(),
            });
        }

        // Method 4: Manual instructions for user
        Ok(WslCompactResult {
            method: WslCompactMethod::Manual,
            success: false,
            message: "Automatic compaction failed. See manual instructions.".to_string(),
        })
    }

    /// Try to compact using wsl --manage
    async fn try_wsl_manage_compact(&self, distribution: &str) -> Result<bool> {
        // Check if wsl --manage is available
        let output = Command::new("wsl")
            .arg("--manage")
            .arg("--help")
            .output()
            .await;

        match output {
            Ok(result) if result.status.success() => {
                debug!("wsl --manage is available, attempting compaction");

                // Try to shutdown first
                let _ = Command::new("wsl")
                    .arg("--manage")
                    .arg(distribution)
                    .arg("--shutdown")
                    .output()
                    .await;

                // Try compact (if supported)
                let compact_output = Command::new("wsl")
                    .arg("--manage")
                    .arg(distribution)
                    .arg("--optimize")
                    .output()
                    .await;

                match compact_output {
                    Ok(result) if result.status.success() => {
                        info!("Successfully compacted using wsl --manage");
                        Ok(true)
                    }
                    _ => {
                        debug!("wsl --manage --optimize not available");
                        Ok(false)
                    }
                }
            }
            _ => {
                debug!("wsl --manage not available");
                Ok(false)
            }
        }
    }

    /// Try to compact using wslconfig.exe
    async fn try_wslconfig_compact(&self, distribution: &str) -> Result<bool> {
        // wslconfig.exe doesn't directly support compaction
        // But we can use it to shutdown the distribution
        debug!("Trying wslconfig.exe approach");

        // Shutdown the distribution
        let output = Command::new("wslconfig.exe")
            .arg("/shutdown")
            .output()
            .await;

        match output {
            Ok(result) if result.status.success() => {
                // After shutdown, try diskpart method
                self.try_diskpart_compact(distribution).await
            }
            _ => {
                debug!("wslconfig.exe /shutdown failed");
                Ok(false)
            }
        }
    }

    /// Try to compact using diskpart (Home edition fallback)
    async fn try_diskpart_compact(&self, distribution: &str) -> Result<bool> {
        debug!("Using diskpart fallback for VHDX compaction");

        // Find the VHDX file
        let vhdx_path = self.find_wsl_vhdx_path(distribution)?;

        if !vhdx_path.exists() {
            warn!("VHDX file not found: {}", vhdx_path.display());
            return Ok(false);
        }

        // Create diskpart script
        let script = format!(
            "select vdisk file=\"{}\"\nattach vdisk readonly\ncompact vdisk\ndetach vdisk\nexit",
            vhdx_path.display()
        );

        // Write script to temp file
        let temp_script = std::env::temp_dir().join("winsweep-compact.txt");
        std::fs::write(&temp_script, script)?;

        // Run diskpart
        let output = Command::new("diskpart.exe")
            .arg("/s")
            .arg(&temp_script)
            .output()
            .await;

        // Clean up
        let _ = std::fs::remove_file(&temp_script);

        match output {
            Ok(result) => {
                if result.status.success() {
                    info!("Successfully compacted VHDX using diskpart");
                    Ok(true)
                } else {
                    warn!(
                        "diskpart compaction failed: {}",
                        String::from_utf8_lossy(&result.stderr)
                    );
                    Ok(false)
                }
            }
            Err(e) => {
                warn!("Failed to run diskpart: {}", e);
                Ok(false)
            }
        }
    }

    /// Find WSL VHDX path for a distribution
    fn find_wsl_vhdx_path(&self, distribution: &str) -> Result<PathBuf> {
        // Method 1: Check registry
        if let Ok(path) = self.windows_api.read_registry_string(
            &format!(
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\Lxss\{}",
                distribution
            ),
            "VhdxFilePath",
        ) {
            return Ok(PathBuf::from(path));
        }

        // Method 2: Check default WSL locations
        let user_profile = std::env::var("USERPROFILE").unwrap_or_default();
        let possible_paths = [
            format!(
                r"{}\AppData\Local\Packages\{}\LocalState\ext4.vhdx",
                user_profile,
                self.get_wsl_package_name(distribution)
            ),
            format!(
                r"{}\AppData\Local\WSL\{}\ext4.vhdx",
                user_profile, distribution
            ),
        ];

        for path in possible_paths {
            if PathBuf::from(&path).exists() {
                return Ok(PathBuf::from(path));
            }
        }

        Err(anyhow::anyhow!(
            "Could not find VHDX path for distribution {}",
            distribution
        ))
    }

    /// Get WSL package name from distribution name
    fn get_wsl_package_name(&self, distribution: &str) -> String {
        // Common distribution package names
        match distribution.to_lowercase().as_str() {
            "ubuntu" => "CanonicalGroupLimited.UbuntuonWindows".to_string(),
            "ubuntu-18.04" => "CanonicalGroupLimited.Ubuntu18.04onWindows".to_string(),
            "ubuntu-20.04" => "CanonicalGroupLimited.Ubuntu20.04onWindows".to_string(),
            "ubuntu-22.04" => "CanonicalGroupLimited.Ubuntu22.04onWindows".to_string(),
            "debian" => "TheDebianProject.DebianGNULinux".to_string(),
            "kali-linux" => "KaliLinux.KaliLinux".to_string(),
            "opensuse" => "SUSE.openSUSELeap".to_string(),
            _ => format!("Unknown.{}.onWindows", distribution),
        }
    }

    /// Get manual compaction instructions
    pub fn get_manual_compaction_instructions(&self, distribution: &str) -> String {
        format!(
            r#"Manual WSL2 VHDX Compaction Instructions for {}

Since automatic compaction failed, please follow these steps:

1. Open Command Prompt as Administrator
2. Run: wsl --shutdown
3. Run: diskpart
4. In diskpart, run:
   select vdisk file="<VHDX_PATH>"
   attach vdisk readonly
   compact vdisk
   detach vdisk
   exit

5. Replace <VHDX_PATH> with your distribution's VHDX file, typically:
   %USERPROFILE%\AppData\Local\Packages\<PACKAGE_NAME>\LocalState\ext4.vhdx

6. Restart WSL with: wsl

Note: On Windows Home edition, diskpart is the recommended method for VHDX compaction."#,
            distribution
        )
    }

    /// Check if Device Encryption is available (BitLocker alternative)
    pub fn has_device_encryption(&self) -> bool {
        if !self.is_home_edition {
            return false; // Pro/Enterprise have BitLocker
        }

        // Check for Device Encryption
        if let Ok(status) = self.windows_api.read_registry_string(
            r"SYSTEM\CurrentControlSet\Control\BitLocker",
            "DeviceEncryption",
        ) {
            return status == "1";
        }

        // Check for TPM 2.0 (required for Device Encryption)
        if let Ok(tpm_info) = self.windows_api.read_registry_string(
            r"SYSTEM\CurrentControlSet\Services\TPM\WMI\AdminInfo",
            "SpecVersion",
        ) {
            return tpm_info.starts_with("2.");
        }

        false
    }

    /// Get Home edition limitations
    pub fn get_limitations(&self) -> Vec<String> {
        let mut limitations = Vec::new();

        if self.is_home_edition {
            limitations.push("No Group Policy Editor".to_string());
            limitations.push("No Hyper-V virtualization".to_string());
            limitations.push("No BitLocker drive encryption".to_string());
            limitations.push("No Remote Desktop host".to_string());
            limitations.push("No Windows Sandbox".to_string());

            if self.build_number < 21364 {
                limitations.push("No wsl --manage command".to_string());
            }
        }

        limitations
    }

    /// Get available workarounds for limitations
    pub fn get_workarounds(&self) -> Vec<String> {
        let mut workarounds = Vec::new();

        if self.is_home_edition {
            workarounds.push("Use diskpart for VHD management instead of Hyper-V".to_string());
            workarounds.push("Use Device Encryption instead of BitLocker".to_string());
            workarounds.push("Use wslconfig.exe for basic WSL management".to_string());
            workarounds.push("Edit registry directly instead of Group Policy".to_string());
            workarounds.push("Use third-party remote desktop solutions".to_string());

            if self.build_number < 21364 {
                workarounds.push("Use manual diskpart scripts for WSL2 compaction".to_string());
            }
        }

        workarounds
    }

    /// Detect if running on Home edition
    fn detect_home_edition(windows_api: &WindowsApi) -> Result<bool> {
        if let Ok(edition) = windows_api
            .read_registry_string(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion", "EditionID")
        {
            Ok(edition.to_lowercase().contains("home"))
        } else {
            // Fallback: check product name
            if let Ok(product_name) = windows_api.read_registry_string(
                r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
                "ProductName",
            ) {
                Ok(product_name.to_lowercase().contains("home"))
            } else {
                Ok(false)
            }
        }
    }

    /// Get Windows build number
    fn get_build_number(windows_api: &WindowsApi) -> Result<u32> {
        if let Ok(build_str) = windows_api.read_registry_string(
            r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
            "CurrentBuildNumber",
        ) {
            build_str
                .parse::<u32>()
                .context("Failed to parse build number")
        } else {
            Err(anyhow::anyhow!("Could not read build number"))
        }
    }
}

/// Result of WSL VHDX compaction
#[derive(Debug, Clone)]
pub struct WslCompactResult {
    pub method: WslCompactMethod,
    pub success: bool,
    pub message: String,
}

/// Method used for WSL VHDX compaction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WslCompactMethod {
    WslManage,
    Wslconfig,
    Diskpart,
    Manual,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_home_edition_detection() {
        // This test would need to run on actual Windows
        let compat = HomeEditionCompat::new();
        assert!(compat.is_ok());
    }
}
