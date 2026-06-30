//! Tool availability detection module
//!
//! This module detects the availability of various tools and services
//! that WinSweep interacts with.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::info;

/// Tool detector for checking availability of external tools
pub struct ToolDetector {
    tools: HashMap<String, ToolInfo>,
}

/// Information about a detected tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub version: Option<String>,
    pub path: Option<PathBuf>,
    pub available: bool,
    pub required: bool,
    pub description: String,
}

impl ToolDetector {
    /// Create a new tool detector
    pub fn new() -> Result<Self> {
        let mut detector = Self {
            tools: HashMap::new(),
        };

        detector.detect_all_tools()?;
        Ok(detector)
    }

    /// Detect all relevant tools
    fn detect_all_tools(&mut self) -> Result<()> {
        info!("Detecting tool availability...");

        // System tools
        self.detect_diskpart()?;
        self.detect_wsl()?;
        self.detect_docker()?;
        self.detect_powershell();

        // Package managers
        self.detect_npm()?;
        self.detect_pip()?;
        self.detect_cargo()?;
        self.detect_nuget()?;

        // Development tools
        self.detect_git()?;
        self.detect_vscode()?;

        // Windows-specific tools
        self.detect_schtasks()?;
        self.detect_sc()?;
        self.detect_wevtutil()?;

        info!("Tool detection complete");
        Ok(())
    }

    /// Detect diskpart availability
    fn detect_diskpart(&mut self) -> Result<()> {
        let tool_info = if let Ok(path) = which::which("diskpart.exe") {
            let version = self.get_file_version(&path)?;
            ToolInfo {
                name: "diskpart".to_string(),
                version,
                path: Some(path),
                available: true,
                required: true,
                description: "Disk partition management tool".to_string(),
            }
        } else {
            ToolInfo {
                name: "diskpart".to_string(),
                version: None,
                path: None,
                available: false,
                required: true,
                description: "Disk partition management tool".to_string(),
            }
        };

        self.tools.insert("diskpart".to_string(), tool_info);
        Ok(())
    }

    /// Detect WSL availability
    fn detect_wsl(&mut self) -> Result<()> {
        let tool_info = if let Ok(path) = which::which("wsl.exe") {
            let version = self.get_wsl_version()?;
            ToolInfo {
                name: "wsl".to_string(),
                version,
                path: Some(path),
                available: true,
                required: false,
                description: "Windows Subsystem for Linux".to_string(),
            }
        } else {
            ToolInfo {
                name: "wsl".to_string(),
                version: None,
                path: None,
                available: false,
                required: false,
                description: "Windows Subsystem for Linux".to_string(),
            }
        };

        self.tools.insert("wsl".to_string(), tool_info);
        Ok(())
    }

    /// Detect Docker availability
    fn detect_docker(&mut self) -> Result<()> {
        let tool_info = if let Ok(path) = which::which("docker.exe") {
            let version = self.get_docker_version()?;
            ToolInfo {
                name: "docker".to_string(),
                version,
                path: Some(path),
                available: true,
                required: false,
                description: "Docker container platform".to_string(),
            }
        } else {
            ToolInfo {
                name: "docker".to_string(),
                version: None,
                path: None,
                available: false,
                required: false,
                description: "Docker container platform".to_string(),
            }
        };

        self.tools.insert("docker".to_string(), tool_info);
        Ok(())
    }

    /// Detect PowerShell availability
    fn detect_powershell(&mut self) {
        let mut pwsh_info = ToolInfo {
            name: "powershell".to_string(),
            version: None,
            path: None,
            available: false,
            required: true,
            description: "Windows PowerShell".to_string(),
        };

        // Check for PowerShell 5 (built-in)
        if let Ok(path) = which::which("powershell.exe") {
            pwsh_info.available = true;
            pwsh_info.path = Some(path);
            pwsh_info.version = Some("5.1".to_string());
        }

        self.tools.insert("powershell".to_string(), pwsh_info);

        // Check for PowerShell 7 (optional)
        let mut pwsh7_info = ToolInfo {
            name: "pwsh".to_string(),
            version: None,
            path: None,
            available: false,
            required: false,
            description: "PowerShell 7+".to_string(),
        };

        if let Ok(path) = which::which("pwsh.exe") {
            pwsh7_info.available = true;
            pwsh7_info.version = self.get_powershell7_version(&path).ok().flatten();
            pwsh7_info.path = Some(path);
        }

        self.tools.insert("pwsh".to_string(), pwsh7_info);
    }

    /// Detect npm availability
    fn detect_npm(&mut self) -> Result<()> {
        let tool_info = if let Ok(path) = which::which("npm.cmd") {
            let version = self.get_npm_version()?;
            ToolInfo {
                name: "npm".to_string(),
                version,
                path: Some(path),
                available: true,
                required: false,
                description: "Node.js package manager".to_string(),
            }
        } else {
            ToolInfo {
                name: "npm".to_string(),
                version: None,
                path: None,
                available: false,
                required: false,
                description: "Node.js package manager".to_string(),
            }
        };

        self.tools.insert("npm".to_string(), tool_info);
        Ok(())
    }

    /// Detect pip availability
    fn detect_pip(&mut self) -> Result<()> {
        let tool_info = if let Ok(path) = which::which("pip.exe") {
            let version = self.get_pip_version()?;
            ToolInfo {
                name: "pip".to_string(),
                version,
                path: Some(path),
                available: true,
                required: false,
                description: "Python package manager".to_string(),
            }
        } else {
            ToolInfo {
                name: "pip".to_string(),
                version: None,
                path: None,
                available: false,
                required: false,
                description: "Python package manager".to_string(),
            }
        };

        self.tools.insert("pip".to_string(), tool_info);
        Ok(())
    }

    /// Detect cargo availability
    fn detect_cargo(&mut self) -> Result<()> {
        let tool_info = if let Ok(path) = which::which("cargo.exe") {
            let version = self.get_cargo_version()?;
            ToolInfo {
                name: "cargo".to_string(),
                version,
                path: Some(path),
                available: true,
                required: false,
                description: "Rust package manager".to_string(),
            }
        } else {
            ToolInfo {
                name: "cargo".to_string(),
                version: None,
                path: None,
                available: false,
                required: false,
                description: "Rust package manager".to_string(),
            }
        };

        self.tools.insert("cargo".to_string(), tool_info);
        Ok(())
    }

    /// Detect NuGet availability
    fn detect_nuget(&mut self) -> Result<()> {
        let tool_info = if let Ok(path) = which::which("nuget.exe") {
            let version = self.get_nuget_version()?;
            ToolInfo {
                name: "nuget".to_string(),
                version,
                path: Some(path),
                available: true,
                required: false,
                description: ".NET package manager".to_string(),
            }
        } else {
            ToolInfo {
                name: "nuget".to_string(),
                version: None,
                path: None,
                available: false,
                required: false,
                description: ".NET package manager".to_string(),
            }
        };

        self.tools.insert("nuget".to_string(), tool_info);
        Ok(())
    }

    /// Detect Git availability
    fn detect_git(&mut self) -> Result<()> {
        let tool_info = if let Ok(path) = which::which("git.exe") {
            let version = self.get_git_version()?;
            ToolInfo {
                name: "git".to_string(),
                version,
                path: Some(path),
                available: true,
                required: false,
                description: "Git version control".to_string(),
            }
        } else {
            ToolInfo {
                name: "git".to_string(),
                version: None,
                path: None,
                available: false,
                required: false,
                description: "Git version control".to_string(),
            }
        };

        self.tools.insert("git".to_string(), tool_info);
        Ok(())
    }

    /// Detect VS Code availability
    fn detect_vscode(&mut self) -> Result<()> {
        let tool_info = if let Ok(path) = which::which("code.exe") {
            let version = self.get_vscode_version()?;
            ToolInfo {
                name: "vscode".to_string(),
                version,
                path: Some(path),
                available: true,
                required: false,
                description: "Visual Studio Code".to_string(),
            }
        } else {
            ToolInfo {
                name: "vscode".to_string(),
                version: None,
                path: None,
                available: false,
                required: false,
                description: "Visual Studio Code".to_string(),
            }
        };

        self.tools.insert("vscode".to_string(), tool_info);
        Ok(())
    }

    /// Detect schtasks availability
    fn detect_schtasks(&mut self) -> Result<()> {
        let tool_info = if let Ok(path) = which::which("schtasks.exe") {
            ToolInfo {
                name: "schtasks".to_string(),
                version: None,
                path: Some(path),
                available: true,
                required: false,
                description: "Task scheduler utility".to_string(),
            }
        } else {
            ToolInfo {
                name: "schtasks".to_string(),
                version: None,
                path: None,
                available: false,
                required: false,
                description: "Task scheduler utility".to_string(),
            }
        };

        self.tools.insert("schtasks".to_string(), tool_info);
        Ok(())
    }

    /// Detect sc.exe availability
    fn detect_sc(&mut self) -> Result<()> {
        let tool_info = if let Ok(path) = which::which("sc.exe") {
            ToolInfo {
                name: "sc".to_string(),
                version: None,
                path: Some(path),
                available: true,
                required: false,
                description: "Service control utility".to_string(),
            }
        } else {
            ToolInfo {
                name: "sc".to_string(),
                version: None,
                path: None,
                available: false,
                required: false,
                description: "Service control utility".to_string(),
            }
        };

        self.tools.insert("sc".to_string(), tool_info);
        Ok(())
    }

    /// Detect wevtutil availability
    fn detect_wevtutil(&mut self) -> Result<()> {
        let tool_info = if let Ok(path) = which::which("wevtutil.exe") {
            ToolInfo {
                name: "wevtutil".to_string(),
                version: None,
                path: Some(path),
                available: true,
                required: false,
                description: "Windows Event Log utility".to_string(),
            }
        } else {
            ToolInfo {
                name: "wevtutil".to_string(),
                version: None,
                path: None,
                available: false,
                required: false,
                description: "Windows Event Log utility".to_string(),
            }
        };

        self.tools.insert("wevtutil".to_string(), tool_info);
        Ok(())
    }

    /// Get file version from an executable using the Win32 version-info API.
    ///
    /// Returns the four-part file version (`major.minor.build.revision`) read from
    /// the binary's `VS_FIXEDFILEINFO` resource, or `None` if the file carries no
    /// version resource.
    fn get_file_version(&self, path: &Path) -> Result<Option<String>> {
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            use windows::core::PCWSTR;
            use windows::Win32::Storage::FileSystem::{
                GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW, VS_FIXEDFILEINFO,
            };

            let wide: Vec<u16> = path
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            unsafe {
                let size = GetFileVersionInfoSizeW(PCWSTR(wide.as_ptr()), None);
                if size == 0 {
                    return Ok(None);
                }

                let mut buffer = vec![0u8; size as usize];
                GetFileVersionInfoW(
                    PCWSTR(wide.as_ptr()),
                    0,
                    size,
                    buffer.as_mut_ptr() as *mut std::ffi::c_void,
                )?;

                let sub_block: Vec<u16> = "\\".encode_utf16().chain(std::iter::once(0)).collect();
                let mut value_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
                let mut value_len: u32 = 0;

                let ok = VerQueryValueW(
                    buffer.as_ptr() as *const std::ffi::c_void,
                    PCWSTR(sub_block.as_ptr()),
                    &mut value_ptr,
                    &mut value_len,
                );

                if !ok.as_bool()
                    || value_ptr.is_null()
                    || (value_len as usize) < std::mem::size_of::<VS_FIXEDFILEINFO>()
                {
                    return Ok(None);
                }

                let info = &*(value_ptr as *const VS_FIXEDFILEINFO);
                // 0xFEEF04BD is the documented VS_FIXEDFILEINFO signature.
                if info.dwSignature != 0xFEEF_04BD {
                    return Ok(None);
                }

                let ms = info.dwFileVersionMS;
                let ls = info.dwFileVersionLS;
                let version = format!(
                    "{}.{}.{}.{}",
                    (ms >> 16) & 0xffff,
                    ms & 0xffff,
                    (ls >> 16) & 0xffff,
                    ls & 0xffff
                );
                Ok(Some(version))
            }
        }

        #[cfg(not(windows))]
        {
            let _ = path;
            Ok(None)
        }
    }

    /// Get WSL version
    fn get_wsl_version(&self) -> Result<Option<String>> {
        use std::process::Command;

        let output = Command::new("wsl").arg("--version").output();
        match output {
            Ok(result) if result.status.success() => {
                let version_str = String::from_utf8_lossy(&result.stdout);
                Ok(Some(version_str.trim().to_string()))
            }
            _ => Ok(None),
        }
    }

    /// Get Docker version
    fn get_docker_version(&self) -> Result<Option<String>> {
        use std::process::Command;

        let output = Command::new("docker").arg("--version").output();
        match output {
            Ok(result) if result.status.success() => {
                let version_str = String::from_utf8_lossy(&result.stdout);
                Ok(Some(version_str.trim().to_string()))
            }
            _ => Ok(None),
        }
    }

    /// Get PowerShell 7 version
    fn get_powershell7_version(&self, path: &PathBuf) -> Result<Option<String>> {
        use std::process::Command;

        let output = Command::new(path).arg("--version").output();
        match output {
            Ok(result) if result.status.success() => {
                let version_str = String::from_utf8_lossy(&result.stdout);
                Ok(Some(version_str.trim().to_string()))
            }
            _ => Ok(None),
        }
    }

    /// Get npm version
    fn get_npm_version(&self) -> Result<Option<String>> {
        use std::process::Command;

        let output = Command::new("npm").arg("--version").output();
        match output {
            Ok(result) if result.status.success() => {
                let version_str = String::from_utf8_lossy(&result.stdout);
                Ok(Some(version_str.trim().to_string()))
            }
            _ => Ok(None),
        }
    }

    /// Get pip version
    fn get_pip_version(&self) -> Result<Option<String>> {
        use std::process::Command;

        let output = Command::new("pip").arg("--version").output();
        match output {
            Ok(result) if result.status.success() => {
                let version_str = String::from_utf8_lossy(&result.stdout);
                Ok(Some(version_str.trim().to_string()))
            }
            _ => Ok(None),
        }
    }

    /// Get cargo version
    fn get_cargo_version(&self) -> Result<Option<String>> {
        use std::process::Command;

        let output = Command::new("cargo").arg("--version").output();
        match output {
            Ok(result) if result.status.success() => {
                let version_str = String::from_utf8_lossy(&result.stdout);
                Ok(Some(version_str.trim().to_string()))
            }
            _ => Ok(None),
        }
    }

    /// Get NuGet version by parsing the banner printed by `nuget help`.
    ///
    /// The NuGet CLI prints a first line of the form `NuGet Version: 6.11.0.123`;
    /// we extract the version token from it.
    fn get_nuget_version(&self) -> Result<Option<String>> {
        use std::process::Command;

        let output = Command::new("nuget").arg("help").output();
        match output {
            Ok(result) if result.status.success() => {
                let banner = String::from_utf8_lossy(&result.stdout);
                Ok(Self::parse_nuget_version(&banner))
            }
            _ => {
                // Fall back to the bare invocation, which also prints the banner.
                let output = Command::new("nuget").output();
                match output {
                    Ok(result) => {
                        let banner = String::from_utf8_lossy(&result.stdout);
                        Ok(Self::parse_nuget_version(&banner))
                    }
                    _ => Ok(None),
                }
            }
        }
    }

    /// Extract the version token from a NuGet CLI banner.
    fn parse_nuget_version(banner: &str) -> Option<String> {
        for line in banner.lines() {
            let line = line.trim();
            // Match "NuGet Version: X.Y.Z..." case-insensitively.
            if let Some(idx) = line.to_lowercase().find("nuget version:") {
                let rest = line[idx + "nuget version:".len()..].trim();
                let token = rest.split_whitespace().next()?;
                if !token.is_empty() {
                    return Some(token.to_string());
                }
            }
        }
        None
    }

    /// Get Git version
    fn get_git_version(&self) -> Result<Option<String>> {
        use std::process::Command;

        let output = Command::new("git").arg("--version").output();
        match output {
            Ok(result) if result.status.success() => {
                let version_str = String::from_utf8_lossy(&result.stdout);
                Ok(Some(version_str.trim().to_string()))
            }
            _ => Ok(None),
        }
    }

    /// Get VS Code version
    fn get_vscode_version(&self) -> Result<Option<String>> {
        use std::process::Command;

        let output = Command::new("code").arg("--version").output();
        match output {
            Ok(result) if result.status.success() => {
                let version_str = String::from_utf8_lossy(&result.stdout);
                // First line is the version
                let version = version_str.lines().next();
                Ok(version.map(|v| v.trim().to_string()))
            }
            _ => Ok(None),
        }
    }

    /// Get all detected tools
    pub fn get_tools(&self) -> &HashMap<String, ToolInfo> {
        &self.tools
    }

    /// Get a specific tool
    pub fn get_tool(&self, name: &str) -> Option<&ToolInfo> {
        self.tools.get(name)
    }

    /// Check if a tool is available
    pub fn is_available(&self, name: &str) -> bool {
        self.tools.get(name).is_some_and(|t| t.available)
    }

    /// Get all missing required tools
    pub fn get_missing_required(&self) -> Vec<&ToolInfo> {
        self.tools
            .values()
            .filter(|t| t.required && !t.available)
            .collect()
    }

    /// Get all available optional tools
    pub fn get_available_optional(&self) -> Vec<&ToolInfo> {
        self.tools
            .values()
            .filter(|t| !t.required && t.available)
            .collect()
    }
}

impl Default for ToolDetector {
    fn default() -> Self {
        Self::new().expect("Failed to create ToolDetector")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_nuget_version() {
        let banner = "NuGet Version: 6.11.0.123\nusage: NuGet <command> [args]";
        assert_eq!(
            ToolDetector::parse_nuget_version(banner),
            Some("6.11.0.123".to_string())
        );
        assert_eq!(ToolDetector::parse_nuget_version("no version here"), None);
    }

    #[cfg(windows)]
    #[test]
    fn test_get_file_version_for_system_dll() {
        // kernel32.dll always carries a version resource on Windows.
        let detector = ToolDetector::new().unwrap();
        let path = PathBuf::from(r"C:\Windows\System32\kernel32.dll");
        if path.exists() {
            let version = detector.get_file_version(&path).unwrap();
            assert!(
                version.is_some(),
                "kernel32.dll should expose a file version"
            );
            // Sanity: dotted numeric form.
            let v = version.unwrap();
            assert!(v.split('.').count() == 4, "unexpected version form: {v}");
        }
    }
}
