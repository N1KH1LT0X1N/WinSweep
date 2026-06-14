//! WSL (Windows Subsystem for Linux) detection and management
//!
//! This module provides comprehensive WSL detection with registry fallbacks
//! for Windows Home edition compatibility.

use crate::windows_api::WindowsApi;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use tracing::{debug, warn};

/// WSL version types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WslVersion {
    Wsl1,
    Wsl2,
    Unknown,
}

/// WSL distribution information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WslDistribution {
    pub name: String,
    pub version: WslVersion,
    pub state: WslState,
    pub default: bool,
    pub wsl_path: Option<PathBuf>,
    pub vhdx_path: Option<PathBuf>,
    pub filesystem_path: Option<PathBuf>,
}

/// WSL distribution state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WslState {
    Running,
    Stopped,
    Installing,
    Uninstalling,
    Unknown,
}

/// WSL detection and management
pub struct WslDetector {
    windows_api: WindowsApi,
    distributions: HashMap<String, WslDistribution>,
    has_wsl: bool,
    has_wsl2: bool,
    has_manage_command: bool,
}

impl WslDetector {
    /// Create a new WSL detector
    pub fn new() -> Result<Self> {
        let windows_api = WindowsApi::new()?;

        let mut detector = Self {
            windows_api,
            distributions: HashMap::new(),
            has_wsl: false,
            has_wsl2: false,
            has_manage_command: false,
        };

        detector.detect_wsl_availability()?;
        if detector.has_wsl {
            detector.detect_distributions()?;
        }

        Ok(detector)
    }

    /// Check if WSL is available
    pub fn has_wsl(&self) -> bool {
        self.has_wsl
    }

    /// Check if WSL2 is available
    pub fn has_wsl2(&self) -> bool {
        self.has_wsl2
    }

    /// Check if wsl --manage command is available
    pub fn has_manage_command(&self) -> bool {
        self.has_manage_command
    }

    /// Get all detected distributions
    pub fn distributions(&self) -> &HashMap<String, WslDistribution> {
        &self.distributions
    }

    /// Get a specific distribution
    pub fn get_distribution(&self, name: &str) -> Option<&WslDistribution> {
        self.distributions.get(name)
    }

    /// Detect WSL availability using multiple methods
    fn detect_wsl_availability(&mut self) -> Result<()> {
        // Method 1: Check for wsl.exe in PATH
        if which::which("wsl.exe").is_ok() {
            self.has_wsl = true;
            debug!("WSL detected via wsl.exe in PATH");
        }

        // Method 2: Check registry for WSL feature
        if !self.has_wsl && self.check_registry_wsl_feature()? {
            self.has_wsl = true;
            debug!("WSL detected via registry feature");
        }

        // Method 3: Check for Lxss registry key
        if !self.has_wsl && self.check_registry_lxss()? {
            self.has_wsl = true;
            debug!("WSL detected via Lxss registry key");
        }

        // Method 4: Check for WSL-related files
        if !self.has_wsl && self.check_wsl_files()? {
            self.has_wsl = true;
            debug!("WSL detected via file system");
        }

        if self.has_wsl {
            // Detect WSL2 availability
            self.detect_wsl2_availability()?;

            // Detect wsl --manage command availability
            self.detect_manage_command()?;
        }

        Ok(())
    }

    /// Check registry for WSL feature
    fn check_registry_wsl_feature(&self) -> Result<bool> {
        let paths = [
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\AppModel\StateRepository",
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Appx\AppxAllUserStore\EndPoints",
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Container\Feature",
        ];

        for path in &paths {
            if self
                .windows_api
                .read_registry_string(path, "PackageFamilyList")
                .is_ok()
            {
                return Ok(true);
            }
        }

        // Check for WSL optional feature
        if let Ok(feature_state) = self.windows_api.read_registry_string(
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing\PackageIndex\Microsoft-Windows-Subsystem-Linux-Package~31bf3856ad364e35~amd64~~10.0.19041.1",
            "InstallState",
        ) {
            return Ok(feature_state.contains("Installed"));
        }

        Ok(false)
    }

    /// Check for Lxss registry key
    fn check_registry_lxss(&self) -> Result<bool> {
        // Check if Lxss key exists
        if self
            .windows_api
            .read_registry_string(
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\Lxss",
                "DefaultDistribution",
            )
            .is_ok()
        {
            return Ok(true);
        }

        // Check for distribution-specific keys
        let paths = [
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Lxss\{GUID}",
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Lxss\ distributions",
        ];

        for path in &paths {
            if self
                .windows_api
                .read_registry_string(path, "DistributionName")
                .is_ok()
            {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Check for WSL-related files
    fn check_wsl_files(&self) -> Result<bool> {
        let paths = [
            r"C:\Windows\System32\lxss\lxssmanager.dll",
            r"C:\Windows\System32\wsl.exe",
            r"C:\Windows\System32\wslapi.dll",
            r"C:\Windows\System32\lxrun.exe",
        ];

        for path in &paths {
            if std::path::Path::new(path).exists() {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Detect WSL2 availability
    fn detect_wsl2_availability(&mut self) -> Result<()> {
        // Method 1: Check registry for WSL2 kernel
        if let Ok(kernel_path) = self.windows_api.read_registry_string(
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Lxss",
            "KernelFilePath",
        ) {
            if kernel_path.contains("kernel") || kernel_path.contains("wsl2") {
                self.has_wsl2 = true;
                debug!("WSL2 detected via registry kernel path");
                return Ok(());
            }
        }

        // Method 2: Check for WSL2 kernel file
        let kernel_paths = [
            r"C:\Windows\System32\lxss\tools\kernel",
            r"C:\Windows\System32\drivers\wsl2.sys",
            r"C:\Windows\System32\lxss\kernel",
        ];

        for path in &kernel_paths {
            if std::path::Path::new(path).exists() {
                self.has_wsl2 = true;
                debug!("WSL2 detected via kernel file");
                return Ok(());
            }
        }

        // Method 3: Try to run wsl --status (if available)
        if self.has_wsl {
            if let Ok(output) = Command::new("wsl").arg("--status").output() {
                if String::from_utf8_lossy(&output.stdout).contains("WSL 2") {
                    self.has_wsl2 = true;
                    debug!("WSL2 detected via wsl --status");
                    return Ok(());
                }
            }
        }

        // Method 4: Check Windows build number (WSL2 requires build 18362+)
        if let Ok(build_str) = self.windows_api.read_registry_string(
            r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
            "CurrentBuildNumber",
        ) {
            if let Ok(build) = build_str.parse::<u32>() {
                if build >= 18362 {
                    self.has_wsl2 = true;
                    debug!("WSL2 available based on build number {}", build);
                    return Ok(());
                }
            }
        }

        Ok(())
    }

    /// Detect wsl --manage command availability
    fn detect_manage_command(&mut self) -> Result<()> {
        // Try to run wsl --manage --help
        if let Ok(output) = Command::new("wsl").arg("--manage").arg("--help").output() {
            if output.status.success() {
                self.has_manage_command = true;
                debug!("wsl --manage command available");
                return Ok(());
            }
        }

        // Check Windows build (wsl --manage added in build 21364+)
        if let Ok(build_str) = self.windows_api.read_registry_string(
            r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
            "CurrentBuildNumber",
        ) {
            if let Ok(build) = build_str.parse::<u32>() {
                if build >= 21364 {
                    self.has_manage_command = true;
                    debug!("wsl --manage should be available for build {}", build);
                }
            }
        }

        Ok(())
    }

    /// Detect all WSL distributions
    fn detect_distributions(&mut self) -> Result<()> {
        // Method 1: Parse registry
        self.detect_distributions_from_registry()?;

        // Method 2: Use wsl -l -v if available
        if self.has_wsl {
            self.detect_distributions_from_wsl_command()?;
        }

        Ok(())
    }

    /// Detect distributions from registry
    fn detect_distributions_from_registry(&mut self) -> Result<()> {
        // Get default distribution
        let _default_distro = self
            .windows_api
            .read_registry_string(
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\Lxss",
                "DefaultDistribution",
            )
            .ok();

        // Enumerate distribution GUIDs
        // In a real implementation, we'd enumerate subkeys of Lxss
        // For now, we'll use common distribution names

        let common_distros = [
            "Ubuntu",
            "Ubuntu-18.04",
            "Ubuntu-20.04",
            "Ubuntu-22.04",
            "Debian",
            "kali-linux",
            "openSUSE-Leap",
            "SLES",
            "SLES-12",
            "SLES-15",
            "Ubuntu-16.04",
            "Ubuntu-18.04",
            "Ubuntu-20.04",
            "Arch",
            "CentOS",
            "CentOS-7",
            "CentOS-8",
            "Debian",
            "Fedora",
            "Fedora-33",
            "Fedora-34",
            "Pengwin",
            "Pengwin-Enterprise",
            "RancherOS",
            "RancherDesktop",
            "Alpine",
            "Alpine-3.14",
        ];

        for distro_name in &common_distros {
            if let Some(distro) = self.detect_single_distribution_from_registry(distro_name)? {
                self.distributions.insert(distro_name.to_string(), distro);
            }
        }

        Ok(())
    }

    /// Detect a single distribution from registry
    fn detect_single_distribution_from_registry(
        &self,
        name: &str,
    ) -> Result<Option<WslDistribution>> {
        let base_path = format!(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Lxss\{}", name);

        // Check if distribution exists in registry
        if self
            .windows_api
            .read_registry_string(&base_path, "DistributionName")
            .is_err()
        {
            return Ok(None);
        }

        // Get distribution state
        let state =
            if let Ok(state_str) = self.windows_api.read_registry_string(&base_path, "State") {
                match state_str.as_str() {
                    "1" => WslState::Running,
                    "2" => WslState::Stopped,
                    "3" => WslState::Installing,
                    "4" => WslState::Uninstalling,
                    _ => WslState::Unknown,
                }
            } else {
                WslState::Unknown
            };

        // Get version
        let version =
            if let Ok(version_num) = self.windows_api.read_registry_string(&base_path, "Version") {
                match version_num.as_str() {
                    "1" => WslVersion::Wsl1,
                    "2" => WslVersion::Wsl2,
                    _ => WslVersion::Unknown,
                }
            } else {
                WslVersion::Unknown
            };

        // Get paths
        let wsl_path = self
            .windows_api
            .read_registry_string(&base_path, "BasePath")
            .ok()
            .map(PathBuf::from);

        let vhdx_path = if version == WslVersion::Wsl2 {
            self.windows_api
                .read_registry_string(&base_path, "VhdxFilePath")
                .ok()
                .map(PathBuf::from)
        } else {
            None
        };

        let filesystem_path = wsl_path.as_ref().map(|p| p.join("rootfs"));

        Ok(Some(WslDistribution {
            name: name.to_string(),
            version,
            state,
            default: false, // Will be set by caller
            wsl_path,
            vhdx_path,
            filesystem_path,
        }))
    }

    /// Detect distributions using wsl command
    fn detect_distributions_from_wsl_command(&mut self) -> Result<()> {
        let output = Command::new("wsl").arg("-l").arg("-v").output();

        match output {
            Ok(result) if result.status.success() => {
                let stdout = String::from_utf8_lossy(&result.stdout);

                for line in stdout.lines().skip(1) {
                    // Skip header
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 3 {
                        let name = parts[0].to_string();
                        let state = match parts[1] {
                            "Running" => WslState::Running,
                            "Stopped" => WslState::Stopped,
                            _ => WslState::Unknown,
                        };
                        let version = match parts[2] {
                            "1" => WslVersion::Wsl1,
                            "2" => WslVersion::Wsl2,
                            _ => WslVersion::Unknown,
                        };

                        let default = parts.get(3).map(|&s| s == "*").unwrap_or(false);

                        // Update or add distribution
                        let distro = WslDistribution {
                            name: name.clone(),
                            version,
                            state,
                            default,
                            wsl_path: None, // Will be filled by registry detection
                            vhdx_path: None,
                            filesystem_path: None,
                        };

                        self.distributions.insert(name, distro);
                    }
                }
            }
            Ok(_) => {
                warn!("wsl -l -v command failed");
            }
            Err(e) => {
                warn!("Failed to run wsl command: {}", e);
            }
        }

        Ok(())
    }

    /// Get VHDX size for a WSL2 distribution
    pub fn get_vhdx_size(&self, distribution: &str) -> Result<u64> {
        if let Some(distro) = self.distributions.get(distribution) {
            if let Some(vhdx_path) = &distro.vhdx_path {
                if vhdx_path.exists() {
                    let metadata = std::fs::metadata(vhdx_path)?;
                    return Ok(metadata.len());
                }
            }
        }

        Err(anyhow::anyhow!(
            "Could not find VHDX for distribution {}",
            distribution
        ))
    }

    /// Check if a distribution has a VHDX file
    pub fn has_vhdx(&self, distribution: &str) -> bool {
        if let Some(distro) = self.distributions.get(distribution) {
            distro.version == WslVersion::Wsl2
                && distro.vhdx_path.as_ref().is_some_and(|p| p.exists())
        } else {
            false
        }
    }

    /// Get all WSL2 distributions
    pub fn get_wsl2_distributions(&self) -> Vec<&WslDistribution> {
        self.distributions
            .values()
            .filter(|d| d.version == WslVersion::Wsl2)
            .collect()
    }

    /// Get total size of all WSL2 VHDX files
    pub fn get_total_wsl2_size(&self) -> u64 {
        self.get_wsl2_distributions()
            .iter()
            .filter_map(|d| d.vhdx_path.as_ref())
            .filter_map(|p| p.metadata().ok())
            .map(|m| m.len())
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wsl_detector_creation() {
        // This test would require WSL to be installed
        // In CI, we might skip this test
        let detector = WslDetector::new();
        assert!(detector.is_ok());
    }
}
