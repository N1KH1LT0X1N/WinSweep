//! Windows edition detection and feature availability
//! 
//! This module detects Windows edition and available features,
//! particularly for handling differences between Home and Pro editions.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// Windows edition types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowsEdition {
    Home,
    Pro,
    Enterprise,
    Education,
    ProEducation,
    ProForWorkstations,
    Unknown,
}

/// Available Windows features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowsFeatures {
    pub has_group_policy: bool,
    pub has_hyper_v: bool,
    pub has_bitlocker: bool,
    pub has_device_encryption: bool,
    pub has_remote_desktop: bool,
    pub has_wsl: bool,
    pub has_wsl2: bool,
    pub has_docker_desktop: bool,
    pub has_diskpart: bool,
    pub has_power_shell_7: bool,
    pub has_windows_sandbox: bool,
    pub has_windows_defender_exploit_guard: bool,
    pub has_windows_defender_application_control: bool,
    pub has_windows_defender_network_inspection: bool,
    pub has_windows_defender_tamper_protection: bool,
}

/// Windows edition and feature detector
pub struct WindowsEditionDetector {
    edition: WindowsEdition,
    features: WindowsFeatures,
    build_number: u32,
    version: String,
}

impl WindowsEditionDetector {
    /// Create a new detector and analyze the system
    pub fn new() -> Result<Self> {
        let edition = Self::detect_edition()?;
        let build_number = Self::get_build_number()?;
        let version = Self::get_version()?;
        
        info!("Detected Windows edition: {:?}", edition);
        info!("Windows version: {} (Build {})", version, build_number);
        
        let features = Self::detect_features(edition, build_number)?;
        
        Ok(Self {
            edition,
            features,
            build_number,
            version,
        })
    }
    
    /// Get the detected Windows edition
    pub fn edition(&self) -> WindowsEdition {
        self.edition
    }
    
    /// Get the detected features
    pub fn features(&self) -> &WindowsFeatures {
        &self.features
    }
    
    /// Get the build number
    pub fn build_number(&self) -> u32 {
        self.build_number
    }
    
    /// Get the version string
    pub fn version(&self) -> &str {
        &self.version
    }
    
    /// Check if a feature is available
    pub fn has_feature(&self, feature: impl Fn(&WindowsFeatures) -> bool) -> bool {
        feature(&self.features)
    }
    
    /// Detect Windows edition from registry
    fn detect_edition() -> Result<WindowsEdition> {
        use crate::windows_api::WindowsApi;
        
        let api = WindowsApi::new()?;
        
        // Try multiple registry paths for edition detection
        let paths = [
            r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
            r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\EditionSpecific",
            r"SYSTEM\CurrentControlSet\Control\ProductOptions",
        ];
        
        for path in &paths {
            // Try different value names
            let value_names = ["EditionID", "ProductName", "ProductType"];
            
            for value_name in &value_names {
                if let Ok(edition_string) = api.read_registry_string(path, value_name) {
                    debug!("Found edition string: {}", edition_string);
                    
                    let edition = Self::parse_edition_string(&edition_string);
                    if edition != WindowsEdition::Unknown {
                        return Ok(edition);
                    }
                }
            }
        }
        
        warn!("Could not detect Windows edition from registry");
        Ok(WindowsEdition::Unknown)
    }
    
    /// Parse edition string into enum
    fn parse_edition_string(s: &str) -> WindowsEdition {
        let s_lower = s.to_lowercase();
        
        if s_lower.contains("home") {
            WindowsEdition::Home
        } else if s_lower.contains("pro") && s_lower.contains("education") {
            WindowsEdition::ProEducation
        } else if s_lower.contains("pro") && s_lower.contains("workstation") {
            WindowsEdition::ProForWorkstations
        } else if s_lower.contains("pro") {
            WindowsEdition::Pro
        } else if s_lower.contains("enterprise") {
            WindowsEdition::Enterprise
        } else if s_lower.contains("education") {
            WindowsEdition::Education
        } else {
            WindowsEdition::Unknown
        }
    }
    
    /// Get Windows build number
    fn get_build_number() -> Result<u32> {
        use crate::windows_api::WindowsApi;
        
        let api = WindowsApi::new()?;
        
        if let Ok(build_str) = api.read_registry_string(
            r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
            "CurrentBuildNumber",
        ) {
            build_str.parse::<u32>()
                .context("Failed to parse build number")
        } else {
            Err(anyhow::anyhow!("Could not read build number from registry"))
        }
    }
    
    /// Get Windows version string
    fn get_version() -> Result<String> {
        use crate::windows_api::WindowsApi;
        
        let api = WindowsApi::new()?;
        
        // Try DisplayVersion first (Windows 10/11)
        if let Ok(version) = api.read_registry_string(
            r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
            "DisplayVersion",
        ) {
            Ok(version)
        } else {
            // Fallback to CurrentVersion
            api.read_registry_string(
                r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
                "CurrentVersion",
            )
        }
    }
    
    /// Detect available features based on edition and build
    fn detect_features(edition: WindowsEdition, build: u32) -> Result<WindowsFeatures> {
        let mut features = WindowsFeatures {
            has_group_policy: false,
            has_hyper_v: false,
            has_bitlocker: false,
            has_device_encryption: false,
            has_remote_desktop: false,
            has_wsl: false,
            has_wsl2: false,
            has_docker_desktop: false,
            has_diskpart: true, // Available on all editions
            has_power_shell_7: false,
            has_windows_sandbox: false,
            has_windows_defender_exploit_guard: false,
            has_windows_defender_application_control: false,
            has_windows_defender_network_inspection: false,
            has_windows_defender_tamper_protection: false,
        };
        
        // Feature availability by edition
        match edition {
            WindowsEdition::Home => {
                // Home edition limitations
                features.has_group_policy = false;
                features.has_hyper_v = false;
                features.has_bitlocker = false;
                features.has_device_encryption = build >= 1703; // Available on newer builds
                features.has_remote_desktop = false; // Client only
                features.has_windows_sandbox = false;
                features.has_windows_defender_exploit_guard = false;
                features.has_windows_defender_application_control = false;
            }
            WindowsEdition::Pro => {
                features.has_group_policy = true;
                features.has_hyper_v = true;
                features.has_bitlocker = true;
                features.has_device_encryption = true;
                features.has_remote_desktop = true;
                features.has_windows_sandbox = build >= 18305;
                features.has_windows_defender_exploit_guard = true;
                features.has_windows_defender_application_control = true;
            }
            WindowsEdition::Enterprise | WindowsEdition::Education => {
                // All features available
                features.has_group_policy = true;
                features.has_hyper_v = true;
                features.has_bitlocker = true;
                features.has_device_encryption = true;
                features.has_remote_desktop = true;
                features.has_windows_sandbox = build >= 18305;
                features.has_windows_defender_exploit_guard = true;
                features.has_windows_defender_application_control = true;
            }
            WindowsEdition::ProEducation | WindowsEdition::ProForWorkstations => {
                // Similar to Pro with additional features
                features.has_group_policy = true;
                features.has_hyper_v = true;
                features.has_bitlocker = true;
                features.has_device_encryption = true;
                features.has_remote_desktop = true;
                features.has_windows_sandbox = build >= 18305;
                features.has_windows_defender_exploit_guard = true;
                features.has_windows_defender_application_control = true;
            }
            WindowsEdition::Unknown => {
                // Assume minimal features
                warn!("Unknown edition, assuming minimal feature set");
            }
        }
        
        // Detect WSL availability
        features.has_wsl = Self::detect_wsl_availability()?;
        if features.has_wsl {
            features.has_wsl2 = Self::detect_wsl2_availability()?;
        }
        
        // Detect Docker Desktop
        features.has_docker_desktop = Self::detect_docker_desktop()?;
        
        // Detect PowerShell 7
        features.has_power_shell_7 = Self::detect_powershell_7()?;
        
        // Windows Defender features (available on all editions with recent builds)
        if build >= 18362 {
            features.has_windows_defender_network_inspection = true;
            features.has_windows_defender_tamper_protection = true;
        }
        
        Ok(features)
    }
    
    /// Detect WSL availability
    fn detect_wsl_availability() -> Result<bool> {
        use crate::windows_api::WindowsApi;
        
        let api = WindowsApi::new()?;
        
        // Check for WSL feature in registry
        if let Ok(_) = api.read_registry_string(
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\AppModel\StateRepository",
            "PackageFamilyList",
        ) {
            return Ok(true);
        }
        
        // Check for WSL executable
        if which::which("wsl.exe").is_ok() {
            return Ok(true);
        }
        
        Ok(false)
    }
    
    /// Detect WSL2 availability
    fn detect_wsl2_availability() -> Result<bool> {
        use crate::windows_api::WindowsApi;
        
        let api = WindowsApi::new()?;
        
        // Check for WSL2 kernel
        if let Ok(_) = api.read_registry_string(
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Lxss",
            "DefaultDistribution",
        ) {
            return Ok(true);
        }
        
        // Try to run wsl --status
        if which::which("wsl.exe").is_ok() {
            // In a real implementation, we'd run wsl --status and parse output
            // For now, assume WSL2 is available if WSL is available and build >= 18362
            if let Ok(build) = Self::get_build_number() {
                return Ok(build >= 18362);
            }
        }
        
        Ok(false)
    }
    
    /// Detect Docker Desktop installation
    fn detect_docker_desktop() -> Result<bool> {
        // Check for Docker Desktop executable
        if which::which("docker.exe").is_ok() {
            return Ok(true);
        }
        
        // Check common installation paths
        let paths = [
            r"C:\Program Files\Docker\Docker\Docker Desktop.exe",
            r"C:\Program Files\Docker\Docker\resources\docker.exe",
        ];
        
        for path in &paths {
            if std::path::Path::new(path).exists() {
                return Ok(true);
            }
        }
        
        Ok(false)
    }
    
    /// Detect PowerShell 7 installation
    fn detect_powershell_7() -> Result<bool> {
        // Check for pwsh.exe
        if which::which("pwsh.exe").is_ok() {
            return Ok(true);
        }
        
        // Check common installation paths
        let paths = [
            r"C:\Program Files\PowerShell\7\pwsh.exe",
            r"C:\Program Files\PowerShell\6\pwsh.exe",
        ];
        
        for path in &paths {
            if std::path::Path::new(path).exists() {
                return Ok(true);
            }
        }
        
        Ok(false)
    }
    
    /// Get compatibility report
    pub fn get_compatibility_report(&self) -> WindowsCompatibilityReport {
        let mut limitations = Vec::new();
        let mut recommendations = Vec::new();
        
        match self.edition {
            WindowsEdition::Home => {
                limitations.push("No Group Policy Editor".to_string());
                limitations.push("No Hyper-V virtualization".to_string());
                limitations.push("No BitLocker drive encryption".to_string());
                limitations.push("No Remote Desktop host".to_string());
                limitations.push("No Windows Sandbox".to_string());
                limitations.push("Limited Windows Defender features".to_string());
                
                recommendations.push("Use Device Encryption instead of BitLocker".to_string());
                recommendations.push("Use third-party virtualization software".to_string());
                recommendations.push("Use third-party remote desktop solutions".to_string());
            }
            WindowsEdition::Pro => {
                // No major limitations
                recommendations.push("Enable Hyper-V for WSL2 support".to_string());
                recommendations.push("Consider Windows Sandbox for testing".to_string());
            }
            _ => {}
        }
        
        // WSL-specific recommendations
        if !self.features.has_wsl {
            limitations.push("WSL not available".to_string());
            recommendations.push("Install WSL from Microsoft Store or enable Windows feature".to_string());
        } else if !self.features.has_wsl2 {
            limitations.push("WSL2 not available".to_string());
            recommendations.push("Update to Windows 10 build 18362 or later".to_string());
        }
        
        // Docker-specific recommendations
        if !self.features.has_docker_desktop {
            recommendations.push("Install Docker Desktop for container support".to_string());
        }
        
        WindowsCompatibilityReport {
            edition: self.edition,
            version: self.version.clone(),
            build_number: self.build_number,
            features: self.features.clone(),
            limitations,
            recommendations,
        }
    }
}

/// Windows compatibility report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowsCompatibilityReport {
    pub edition: WindowsEdition,
    pub version: String,
    pub build_number: u32,
    pub features: WindowsFeatures,
    pub limitations: Vec<String>,
    pub recommendations: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_edition_parsing() {
        assert_eq!(WindowsEditionDetector::parse_edition_string("Windows 10 Home"), WindowsEdition::Home);
        assert_eq!(WindowsEditionDetector::parse_edition_string("Windows 11 Pro"), WindowsEdition::Pro);
        assert_eq!(WindowsEditionDetector::parse_edition_string("Windows 10 Enterprise"), WindowsEdition::Enterprise);
        assert_eq!(WindowsEditionDetector::parse_edition_string("Unknown Edition"), WindowsEdition::Unknown);
    }
}
