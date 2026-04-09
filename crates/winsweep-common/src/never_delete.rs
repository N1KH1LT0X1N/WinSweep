//! NEVER_DELETE list - paths that should never be deleted
//! 
//! This module contains the list of critical system paths and files
//! that must never be deleted by WinSweep.

use std::path::PathBuf;

/// Critical system paths that should never be deleted
pub const NEVER_DELETE_PATHS: &[&str] = &[
    // Windows system directories
    r"C:\Windows",
    r"C:\Windows\System32",
    r"C:\Windows\SysWOW64",
    r"C:\Windows\System32\drivers",
    r"C:\Windows\System32\config",
    r"C:\Windows\System32\winevt",
    r"C:\Windows\System32\LogFiles",
    r"C:\Windows\System32\catroot",
    r"C:\Windows\System32\catroot2",
    r"C:\Windows\Microsoft.NET",
    r"C:\Windows\assembly",
    r"C:\Windows\servicing",
    r"C:\Windows\WinSxS",
    r"C:\Windows\SoftwareDistribution",
    r"C:\Windows\inf",
    r"C:\Windows\Installer",
    r"C:\Windows\Fonts",
    r"C:\Windows\resources",
    r"C:\Windows\addins",
    r"C:\Windows\PolicyDefinitions",
    r"C:\Windows\debug",
    r"C:\Windows\LiveKernelReports",
    r"C:\Windows\Logs",
    r"C:\Windows\Prefetch",
    r"C:\Windows\Repair",
    r"C:\Windows\Security",
    r"C:\Windows\SystemResources",
    
    // Program Files
    r"C:\Program Files",
    r"C:\Program Files (x86)",
    r"C:\Program Files\Common Files",
    r"C:\Program Files (x86)\Common Files",
    
    // ProgramData
    r"C:\ProgramData",
    r"C:\ProgramData\Microsoft",
    r"C:\ProgramData\Package Cache",
    
    // User profiles (template)
    r"C:\Users\Default",
    r"C:\Users\Default User",
    r"C:\Users\Public",
    r"C:\Users\Public\Desktop",
    r"C:\Users\Public\Documents",
    
    // Boot critical files
    r"C:\bootmgr",
    r"C:\BOOTNXT",
    r"C:\bootsect.bak",
    r"C:\EFI",
    r"C:\$Recycle.Bin",
    r"C:\System Volume Information",
    r"C:\Recovery",
    
    // Page file and hibernation
    r"C:\pagefile.sys",
    r"C:\hiberfil.sys",
    r"C:\swapfile.sys",
    
    // WSL critical
    r"C:\Windows\System32\lxss",
    
    // Docker critical
    r"C:\ProgramData\Docker",
    r"C:\ProgramData\DockerDesktop",
    
    // Hardware-specific
    r"C:\Intel",
    r"C:\AMD",
    r"C:\NVIDIA",
    r"C:\ATI",
    
    // OEM partitions (usually not on C: but listed for safety)
    r"C:\OEM",
    r"C:\OEMPartition",
    
    // Microsoft Office critical
    r"C:\Program Files\Microsoft Office",
    r"C:\Program Files (x86)\Microsoft Office",
    
    // Visual Studio critical
    r"C:\Program Files\Microsoft Visual Studio",
    r"C:\Program Files (x86)\Microsoft Visual Studio",
    r"C:\Program Files (x86)\Common Files\Microsoft Shared\MSEnv",
    
    // SQL Server
    r"C:\Program Files\Microsoft SQL Server",
    r"C:\Program Files (x86)\Microsoft SQL Server",
    
    // Critical registry hives (for reference)
    // r"C:\Windows\System32\config\SOFTWARE",
    // r"C:\Windows\System32\config\SYSTEM",
    // r"C:\Windows\System32\config\SAM",
    // r"C:\Windows\System32\config\SECURITY",
    // r"C:\Windows\System32\config\DEFAULT",
];

/// Additional patterns that should never be deleted
pub const NEVER_DELETE_PATTERNS: &[&str] = &[
    "*.sys",
    "*.dll",
    "*.exe",
    "*.com",
    "*.bat",
    "*.cmd",
    "*.ps1",
    "*.scr",
    "*.cpl",
    "*.msc",
    "*.msp",
    "*.msi",
    "*.msu",
    "*.cab",
    "*.psm1",
    "*.psd1",
    "*.ps1xml",
    "*.cdxml",
    "*.pssc",
    "*.diagcab",
    "*.efi",
    "*.bin",  // Careful with this one
    "*.vhd",
    "*.vhdx",
    "*.iso",
    "*.wim",
    "*.esd",
    "*.dmp",
    "*.hdmp",
    "*.mdmp",
];

/// Check if a path should never be deleted
pub fn should_never_delete(path: &PathBuf) -> bool {
    let path_str = path.to_string_lossy().to_lowercase();
    
    // Check exact paths
    for never_path in NEVER_DELETE_PATHS {
        if path_str.starts_with(&never_path.to_lowercase()) {
            return true;
        }
    }
    
    // Check if it's a parent of a never-delete path
    for never_path in NEVER_DELETE_PATHS {
        let never_path_buf = PathBuf::from(never_path);
        if never_path_buf.starts_with(&path) {
            return true;
        }
    }
    
    // Check file patterns
    if let Some(file_name) = path.file_name() {
        if let Some(file_str) = file_name.to_str() {
            for pattern in NEVER_DELETE_PATTERNS {
                if matches_pattern(file_str, pattern) {
                    // Special case: allow deletion of these extensions in certain contexts
                    if is_safe_context(&path, file_str) {
                        continue;
                    }
                    return true;
                }
            }
        }
    }
    
    false
}

/// Check if a file pattern matches
fn matches_pattern(name: &str, pattern: &str) -> bool {
    if pattern.starts_with("*.") {
        let extension = pattern.strip_prefix("*.").unwrap();
        name.to_lowercase().ends_with(&extension.to_lowercase())
    } else {
        name.to_lowercase() == pattern.to_lowercase()
    }
}

/// Check if a potentially dangerous file is safe to delete in its current context
fn is_safe_context(path: &PathBuf, file_name: &str) -> bool {
    let path_str = path.to_string_lossy().to_lowercase();
    
    // Safe contexts for .exe files
    if file_name.to_lowercase().ends_with(".exe") {
        // Allow deletion from temp folders
        if path_str.contains("\\temp\\") || path_str.contains("\\tmp\\") {
            return true;
        }
        
        // Allow deletion from cache folders
        if path_str.contains("\\cache\\") || path_str.contains("\\caches\\") {
            return true;
        }
        
        // Allow deletion from package manager caches
        if path_str.contains("\\npm\\") || path_str.contains("\\pip\\") || path_str.contains("\\cargo\\") {
            return true;
        }
        
        // Allow deletion from node_modules
        if path_str.contains("\\node_modules\\") {
            return true;
        }
        
        // Allow deletion from target/debug or target/release
        if path_str.contains("\\target\\debug\\") || path_str.contains("\\target\\release\\") {
            return true;
        }
    }
    
    // Safe contexts for .dll files
    if file_name.to_lowercase().ends_with(".dll") {
        // Allow deletion from temp folders
        if path_str.contains("\\temp\\") || path_str.contains("\\tmp\\") {
            return true;
        }
        
        // Allow deletion from cache folders
        if path_str.contains("\\cache\\") || path_str.contains("\\caches\\") {
            return true;
        }
        
        // Allow deletion from package manager caches
        if path_str.contains("\\npm\\") || path_str.contains("\\pip\\") {
            return true;
        }
    }
    
    // Safe contexts for .log files (these are generally safe)
    if file_name.to_lowercase().ends_with(".log") {
        return true;
    }
    
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_never_delete_system32() {
        let path = PathBuf::from(r"C:\Windows\System32\kernel32.dll");
        assert!(should_never_delete(&path));
    }
    
    #[test]
    fn test_never_delete_program_files() {
        let path = PathBuf::from(r"C:\Program Files\SomeApp\app.exe");
        assert!(should_never_delete(&path));
    }
    
    #[test]
    fn test_allow_delete_temp_exe() {
        let path = PathBuf::from(r"C:\Users\user\AppData\Local\Temp\installer.exe");
        assert!(!should_never_delete(&path));
    }
    
    #[test]
    fn test_allow_delete_node_modules() {
        let path = PathBuf::from(r"C:\project\node_modules\some-package\bin\cli.exe");
        assert!(!should_never_delete(&path));
    }
}
