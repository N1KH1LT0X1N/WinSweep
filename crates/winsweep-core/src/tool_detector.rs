//! Tool availability detection module
//!
//! This module detects the availability of various tools and services
//! that WinSweep interacts with.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
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

    /// Get file version from executable
    fn get_file_version(&self, _path: &PathBuf) -> Result<Option<String>> {
        // In a real implementation, this would use Windows APIs to get version info
        // For now, return None
        Ok(None)
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

    /// Get NuGet version
    fn get_nuget_version(&self) -> Result<Option<String>> {
        use std::process::Command;

        let output = Command::new("nuget").output();
        match output {
            Ok(result) if result.status.success() => {
                let _version_str = String::from_utf8_lossy(&result.stdout);
                // Parse version from output
                Ok(None) // Simplified for now
            }
            _ => Ok(None),
        }
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
