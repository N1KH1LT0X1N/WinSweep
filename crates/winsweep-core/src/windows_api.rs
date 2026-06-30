//! Windows API wrapper
//!
//! This module provides safe wrappers around Windows API functions used by WinSweep.

use anyhow::Result;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::ptr;
use tracing::{error, warn};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, GetLastError, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, GetDiskFreeSpaceExW, GetFileAttributesW, GetFinalPathNameByHandleW,
    SetFileAttributesW, FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_REPARSE_POINT,
    FILE_FLAGS_AND_ATTRIBUTES, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_READ, FILE_SHARE_WRITE,
    INVALID_FILE_ATTRIBUTES, OPEN_EXISTING, VOLUME_NAME_DOS,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32First, Process32Next, PROCESSENTRY32, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_LOCAL_MACHINE,
    KEY_ENUMERATE_SUB_KEYS, KEY_READ,
};

/// RAII guard that closes a Windows `HANDLE` on drop, guaranteeing the handle is
/// released on every exit path (including early returns/errors).
struct ScopedHandle(HANDLE);

impl Drop for ScopedHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

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
            if attributes == INVALID_FILE_ATTRIBUTES {
                let error = GetLastError();
                warn!(
                    "GetFileAttributesW failed for {}: error {:?}",
                    path.display(),
                    error
                );
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
                windows::Win32::Storage::FileSystem::FILE_READ_ATTRIBUTES.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                Some(ptr::null()),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS,
                None,
            )?;
            // Ensure the handle is closed on every return path below.
            let _handle_guard = ScopedHandle(handle);

            let mut buffer = [0u16; 32768]; // MAX_PATH * 4
            let result = GetFinalPathNameByHandleW(handle, &mut buffer, VOLUME_NAME_DOS);

            if result == 0 {
                let error = GetLastError();
                error!("GetFinalPathNameByHandleW failed: error {:?}", error);
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
            let final_path = final_path
                .strip_prefix(r"\\?\")
                .map(|s| s.to_string())
                .unwrap_or(final_path);

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
                return Err(anyhow::anyhow!("GetDiskFreeSpaceExW failed"));
            }
        }

        Ok((free_bytes_available, total_bytes, total_free_bytes))
    }

    /// Check if a file is hidden
    pub fn is_hidden(&self, path: &Path) -> Result<bool> {
        let path_wide = to_wide(path);

        unsafe {
            let attributes = GetFileAttributesW(PCWSTR(path_wide.as_ptr()));
            if attributes == INVALID_FILE_ATTRIBUTES {
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
            if attributes == INVALID_FILE_ATTRIBUTES {
                return Err(anyhow::anyhow!("Failed to get file attributes"));
            }

            if hidden {
                attributes |= FILE_ATTRIBUTE_HIDDEN.0;
            } else {
                attributes &= !FILE_ATTRIBUTE_HIDDEN.0;
            }

            let result = SetFileAttributesW(
                PCWSTR(path_wide.as_ptr()),
                FILE_FLAGS_AND_ATTRIBUTES(attributes),
            );
            if result.is_err() {
                return Err(anyhow::anyhow!("Failed to set hidden attribute"));
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
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|&b| b == 0)
                    .unwrap_or(entry.szExeFile.len());
                let exe_file = String::from_utf8_lossy(&entry.szExeFile[..len]).to_string();

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
        Ok(processes
            .iter()
            .any(|p| p.name.to_lowercase() == name.to_lowercase()))
    }

    /// Check if a file is locked by a process
    pub fn is_file_locked(&self, path: &Path) -> Result<bool> {
        let path_wide = to_wide(path);

        unsafe {
            // Try to open the file with exclusive access
            let handle = CreateFileW(
                PCWSTR(path_wide.as_ptr()),
                windows::Win32::Storage::FileSystem::FILE_READ_ATTRIBUTES.0,
                FILE_SHARE_READ,
                Some(ptr::null()),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS,
                None,
            );

            match handle {
                Ok(h) => {
                    // Successfully opened, not locked
                    let _ = h;
                    Ok(false)
                }
                Err(_) => {
                    // Failed to open, likely locked
                    Ok(true)
                }
            }
        }
    }

    /// Enumerate the immediate subkey names of a registry key.
    ///
    /// Returns an empty `Vec` (not an error) when the key exists but has no subkeys.
    pub fn enumerate_registry_subkeys(&self, key: &str) -> Result<Vec<String>> {
        let key_wide = to_wide(key);
        let mut key_handle = HKEY::default();

        unsafe {
            RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                PCWSTR(key_wide.as_ptr()),
                0,
                KEY_ENUMERATE_SUB_KEYS,
                &mut key_handle,
            )
            .map_err(|e| anyhow::anyhow!("Failed to open registry key '{}': {}", key, e))?;

            let mut subkeys = Vec::new();
            let mut index = 0u32;
            loop {
                // Max subkey name length on Windows is 255 characters + null.
                let mut name_buf = vec![0u16; 256];
                let mut name_len = name_buf.len() as u32;

                let result = RegEnumKeyExW(
                    key_handle,
                    index,
                    windows::core::PWSTR(name_buf.as_mut_ptr()),
                    &mut name_len,
                    None,
                    windows::core::PWSTR::null(),
                    None,
                    None,
                );

                if result.is_err() {
                    // ERROR_NO_MORE_ITEMS terminates the loop cleanly.
                    break;
                }

                let name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
                subkeys.push(name);
                index += 1;
            }

            let _ = RegCloseKey(key_handle);
            Ok(subkeys)
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

            if result.is_err() {
                return Err(anyhow::anyhow!("Failed to open registry key"));
            }

            let mut data_type = windows::Win32::System::Registry::REG_VALUE_TYPE(0);
            let mut data_size = 0u32;

            // First call to get the required buffer size
            let result = RegQueryValueExW(
                key_handle,
                PCWSTR(value_wide.as_ptr()),
                None,
                Some(&mut data_type),
                None,
                Some(&mut data_size),
            );

            if result.is_err() || data_size == 0 {
                let _ = RegCloseKey(key_handle);
                return Err(anyhow::anyhow!("Failed to query registry value size"));
            }

            let mut buffer = vec![0u16; (data_size / 2) as usize];

            let result = RegQueryValueExW(
                key_handle,
                PCWSTR(value_wide.as_ptr()),
                None,
                Some(&mut data_type),
                Some(buffer.as_mut_ptr() as *mut u8),
                Some(&mut data_size),
            );

            let _ = RegCloseKey(key_handle);

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
fn to_wide(path: impl AsRef<OsStr>) -> Vec<u16> {
    path.as_ref()
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
