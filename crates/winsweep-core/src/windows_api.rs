//! Windows API wrapper
//! 
//! This module provides safe wrappers around Windows API functions used by WinSweep.

use anyhow::{Context, Result};
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::Path;
use std::ptr;
use tracing::{debug, error, warn};
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    GetLastError, HANDLE, INVALID_HANDLE_VALUE, BOOL, TRUE, FALSE,
};
use windows::Win32::Security::{
    SecurityDescriptor, SECURITY_ATTRIBUTES, ACL, ACE, Sid,
    GetNamedSecurityInfoW, SE_FILE_OBJECT,
    DACL_SECURITY_INFORMATION,
    SetNamedSecurityInfoW,
    SECURITY_DESCRIPTOR_REVISION,
};
use windows::Win32::Storage::FileSystem::{
    GetFileAttributesW, SetFileAttributesW,
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_HIDDEN,
    GetDiskFreeSpaceExW,
    CreateFileW, OPEN_EXISTING,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_READ, FILE_SHARE_WRITE,
    GetFinalPathNameByHandleW,
    VOLUME_NAME_DOS,
};
use windows::Win32::System::IO::{
    DeviceIoControl,
    FSCTL_GET_REPARSE_POINT,
    REPARSE_DATA_BUFFER,
};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32First, Process32Next,
    PROCESSENTRY32, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Registry::{
    HKEY_LOCAL_MACHINE, RegOpenKeyExW, RegQueryValueExW, RegCloseKey,
    KEY_READ, RRF_RT_REG_SZ,
    HKEY,
};

/// Safe wrapper for Windows API operations
pub struct WindowsApi;

impl WindowsApi {
    /// Create a new WindowsApi instance
    pub fn new() -> Result<Self> {
        Ok(Self)
    }
    
    /// Check if a file or directory has the reparse point attribute
    pub fn is_reparse_point(&self, path: &Path) -> Result<bool> {
        let path_wide = to_wide(path);
        
        unsafe {
            let attributes = GetFileAttributesW(PCWSTR(path_wide.as_ptr()));
            if attributes == INVALID_HANDLE_VALUE {
                let error = GetLastError();
                warn!("GetFileAttributesW failed for {}: error {}", path.display(), error.0);
                return Ok(false);
            }
            
            Ok(attributes & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0)
        }
    }
    
    /// Get the final path name for a file (resolves reparse points)
    pub fn get_final_path_name(&self, path: &Path) -> Result<PathBuf> {
        let path_wide = to_wide(path);
        
        unsafe {
            let handle = CreateFileW(
                PCWSTR(path_wide.as_ptr()),
                windows::Win32::Storage::FileSystem::FILE_READ_ATTRIBUTES,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                Some(ptr::null()),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS,
                None,
            )?;
            
            let mut buffer = [0u16; 32768]; // MAX_PATH * 4
            let result = GetFinalPathNameByHandleW(
                handle,
                PWSTR(buffer.as_mut_ptr()),
                buffer.len() as u32,
                VOLUME_NAME_DOS,
            );
            
            if result == 0 {
                let error = GetLastError();
                error!("GetFinalPathNameByHandleW failed: error {}", error.0);
                return Err(anyhow::anyhow!("GetFinalPathNameByHandleW failed"));
            }
            
            let len = result as usize;
            if len >= buffer.len() {
                return Err(anyhow::anyhow!("Buffer too small for final path"));
            }
            
            let final_path = OsString::from_wide(&buffer[..len])
                .into_string()
                .map_err(|_| anyhow::anyhow!("Invalid UTF-16 in path"))?;
            
            // Remove the \\?\ prefix if present
            let final_path = if final_path.starts_with(r"\\?\") {
                final_path[4..].to_string()
            } else {
                final_path
            };
            
            Ok(PathBuf::from(final_path))
        }
    }
    
    /// Get free disk space for a volume
    pub fn get_disk_free_space(&self, path: &Path) -> Result<(u64, u64, u64)> {
        let path_wide = to_wide(path);
        
        let mut free_bytes_available = 0u64;
        let mut total_bytes = 0u64;
        let mut total_free_bytes = 0u64;
        
        unsafe {
            let result = GetDiskFreeSpaceExW(
                PCWSTR(path_wide.as_ptr()),
                Some(&mut free_bytes_available),
                Some(&mut total_bytes),
                Some(&mut total_free_bytes),
            );
            
            if result.is_err() {
                return Err(anyhow::anyhow!(
                    "GetDiskFreeSpaceExW failed"
                ));
            }
        }
        
        Ok((free_bytes_available, total_bytes, total_free_bytes))
    }
    
    /// Check if a file is hidden
    pub fn is_hidden(&self, path: &Path) -> Result<bool> {
        let path_wide = to_wide(path);
        
        unsafe {
            let attributes = GetFileAttributesW(PCWSTR(path_wide.as_ptr()));
            if attributes == INVALID_HANDLE_VALUE {
                return Ok(false);
            }
            
            Ok(attributes & FILE_ATTRIBUTE_HIDDEN.0 != 0)
        }
    }
    
    /// Set the hidden attribute on a file
    pub fn set_hidden(&self, path: &Path, hidden: bool) -> Result<()> {
        let path_wide = to_wide(path);
        
        unsafe {
            let mut attributes = GetFileAttributesW(PCWSTR(path_wide.as_ptr()));
            if attributes == INVALID_HANDLE_VALUE {
                return Err(anyhow::anyhow!("Failed to get file attributes"));
            }
            
            if hidden {
                attributes |= FILE_ATTRIBUTE_HIDDEN.0;
            } else {
                attributes &= !FILE_ATTRIBUTE_HIDDEN.0;
            }
            
            let result = SetFileAttributesW(PCWSTR(path_wide.as_ptr()), attributes);
            if result.is_err() {
                return Err(anyhow::anyhow!(
                    "Failed to set hidden attribute"
                ));
            }
        }
        
        Ok(())
    }
    
    /// Get a list of running processes
    pub fn get_running_processes(&self) -> Result<Vec<ProcessInfo>> {
        let mut processes = Vec::new();
        
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)?;
            if snapshot.is_invalid() {
                return Err(anyhow::anyhow!("Failed to create process snapshot"));
            }
            
            let mut entry = PROCESSENTRY32 {
                dwSize: std::mem::size_of::<PROCESSENTRY32>() as u32,
                ..Default::default()
            };
            
            let mut success = Process32First(snapshot, &mut entry);
            
            while success.is_ok() {
                let exe_file = OsString::from_wide(&entry.szExeFile)
                    .into_string()
                    .map_err(|_| anyhow::anyhow!("Invalid UTF-16 in process name"))?;
                
                processes.push(ProcessInfo {
                    pid: entry.th32ProcessID,
                    ppid: entry.th32ParentProcessID,
                    name: exe_file,
                });
                
                success = Process32Next(snapshot, &mut entry);
            }
        }
        
        Ok(processes)
    }
    
    /// Check if a process is running by name
    pub fn is_process_running(&self, name: &str) -> Result<bool> {
        let processes = self.get_running_processes()?;
        Ok(processes.iter().any(|p| p.name.to_lowercase() == name.to_lowercase()))
    }
    
    /// Check if a file is locked by a process
    pub fn is_file_locked(&self, path: &Path) -> Result<bool> {
        let path_wide = to_wide(path);
        
        unsafe {
            // Try to open the file with exclusive access
            let handle = CreateFileW(
                PCWSTR(path_wide.as_ptr()),
                windows::Win32::Storage::FileSystem::GENERIC_READ,
                FILE_SHARE_READ,
                Some(ptr::null()),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS,
                None,
            );
            
            match handle {
                Ok(h) => {
                    // Successfully opened, not locked
                    drop(h);
                    Ok(false)
                }
                Err(_) => {
                    // Failed to open, likely locked
                    Ok(true)
                }
            }
        }
    }
    
    /// Read a registry string value
    pub fn read_registry_string(&self, key: &str, value_name: &str) -> Result<String> {
        let key_wide = to_wide(key);
        let value_wide = to_wide(value_name);
        
        unsafe {
            let mut key_handle = HKEY::default();
            
            let result = RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                PCWSTR(key_wide.as_ptr()),
                0,
                KEY_READ,
                &mut key_handle,
            );
            
            if result.0 != 0 {
                return Err(anyhow::anyhow!("Failed to open registry key: {}", result.0));
            }
            
            let mut data_type = 0u32;
            let mut data_size = 0u32;
            
            // First call to get the required buffer size
            let result = RegQueryValueExW(
                key_handle,
                PCWSTR(value_wide.as_ptr()),
                ptr::null_mut(),
                Some(&mut data_type),
                ptr::null_mut(),
                Some(&mut data_size),
            );
            
            if result.is_err() || data_size == 0 {
                RegCloseKey(key_handle);
                return Err(anyhow::anyhow!("Failed to query registry value size"));
            }
            
            let mut buffer = vec![0u16; (data_size / 2) as usize];
            
            let result = RegQueryValueExW(
                key_handle,
                PCWSTR(value_wide.as_ptr()),
                ptr::null_mut(),
                Some(&mut data_type),
                buffer.as_mut_ptr() as *mut _,
                Some(&mut data_size),
            );
            
            RegCloseKey(key_handle);
            
            if result.is_err() {
                return Err(anyhow::anyhow!("Failed to read registry value"));
            }
            
            // Remove null terminator if present
            if buffer.last() == Some(&0) {
                buffer.pop();
            }
            
            let value = OsString::from_wide(&buffer)
                .into_string()
                .map_err(|_| anyhow::anyhow!("Invalid UTF-16 in registry value"))?;
            
            Ok(value)
        }
    }
}

/// Information about a running process
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: u32,
    pub name: String,
}

/// Convert a Rust path to a wide string for Windows API
fn to_wide(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_is_reparse_point() {
        let api = WindowsApi::new().unwrap();
        let temp_dir = TempDir::new().unwrap();
        
        // Regular directory should not be a reparse point
        assert!(!api.is_reparse_point(temp_dir.path()).unwrap());
    }
    
    #[test]
    fn test_get_disk_free_space() {
        let api = WindowsApi::new().unwrap();
        let result = api.get_disk_free_space(Path::new("C:\\"));
        assert!(result.is_ok());
        
        let (available, total, free) = result.unwrap();
        assert!(total > 0);
        assert!(free <= total);
        assert!(available <= free);
    }
    
    #[test]
    fn test_get_running_processes() {
        let api = WindowsApi::new().unwrap();
        let processes = api.get_running_processes().unwrap();
        
        // Should at least have the current process
        assert!(!processes.is_empty());
        
        // Check for common system processes
        let has_system = processes.iter().any(|p| p.name.to_lowercase() == "system");
        assert!(has_system);
    }
}
