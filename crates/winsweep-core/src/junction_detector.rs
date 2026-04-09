//! Junction vs symlink detection module
//! 
//! This module provides functionality to distinguish between NTFS junctions
//! and symbolic links on Windows systems.

use crate::windows_api::WindowsApi;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, warn};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    GetFileAttributesW, FILE_ATTRIBUTE_REPARSE_POINT,
};
use windows::Win32::System::IO::{
    DeviceIoControl, FSCTL_GET_REPARSE_POINT,
    REPARSE_DATA_BUFFER, IO_REPARSE_TAG_MOUNT_POINT, IO_REPARSE_TAG_SYMLINK,
};

/// Detector for distinguishing between junctions and symlinks
pub struct JunctionDetector {
    windows_api: Arc<WindowsApi>,
}

impl JunctionDetector {
    /// Create a new junction detector
    pub fn new(windows_api: Arc<WindowsApi>) -> Self {
        Self { windows_api }
    }
    
    /// Check if a path is a reparse point (junction or symlink)
    pub fn is_reparse_point(&self, path: &Path) -> Result<bool> {
        let path_wide = to_wide(path);
        
        unsafe {
            let attributes = GetFileAttributesW(PCWSTR(path_wide.as_ptr()));
            if attributes == INVALID_HANDLE_VALUE {
                return Ok(false);
            }
            
            Ok((attributes & FILE_ATTRIBUTE_REPARSE_POINT.0) != 0)
        }
    }
    
    /// Check if a reparse point is a junction
    pub fn is_junction(&self, path: &Path) -> Result<bool> {
        if !self.is_reparse_point(path)? {
            return Ok(false);
        }
        
        let reparse_tag = self.get_reparse_tag(path)?;
        Ok(reparse_tag == IO_REPARSE_TAG_MOUNT_POINT)
    }
    
    /// Check if a reparse point is a symbolic link
    pub fn is_symlink(&self, path: &Path) -> Result<bool> {
        if !self.is_reparse_point(path)? {
            return Ok(false);
        }
        
        let reparse_tag = self.get_reparse_tag(path)?;
        Ok(reparse_tag == IO_REPARSE_TAG_SYMLINK)
    }
    
    /// Get the reparse tag for a file/directory
    fn get_reparse_tag(&self, path: &Path) -> Result<u32> {
        let path_wide = to_wide(path);
        
        unsafe {
            // Open the file with reparse point flag
            let handle = CreateFileW(
                PCWSTR(path_wide.as_ptr()),
                windows::Win32::Storage::FileSystem::FILE_READ_ATTRIBUTES,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                Some(std::ptr::null()),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                None,
            )?;
            
            if handle.is_invalid() {
                return Err(anyhow::anyhow!("Failed to open file for reparse query"));
            }
            
            // Allocate buffer for reparse data
            let mut buffer = vec![0u8; 16384]; // MAXIMUM_REPARSE_DATA_BUFFER_SIZE
            
            let mut bytes_returned = 0u32;
            let result = DeviceIoControl(
                handle,
                FSCTL_GET_REPARSE_POINT,
                Some(std::ptr::null()),
                0,
                Some(buffer.as_mut_ptr() as *mut _),
                buffer.len() as u32,
                Some(&mut bytes_returned),
                Some(std::ptr::null()),
            );
            
            drop(handle);
            
            if result.is_err() {
                return Err(anyhow::anyhow!("Failed to get reparse point data"));
            }
            
            // Parse the reparse data buffer
            let reparse_data = &buffer as *const _ as *const REPARSE_DATA_BUFFER;
            let reparse_tag = (*reparse_data).ReparseTag;
            
            Ok(reparse_tag)
        }
    }
    
    /// Get the target of a junction or symlink
    pub fn get_target(&self, path: &Path) -> Result<PathBuf> {
        if !self.is_reparse_point(path)? {
            return Err(anyhow::anyhow!("Path is not a reparse point"));
        }
        
        let path_wide = to_wide(path);
        
        unsafe {
            // Open the file with reparse point flag
            let handle = CreateFileW(
                PCWSTR(path_wide.as_ptr()),
                windows::Win32::Storage::FileSystem::FILE_READ_ATTRIBUTES,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                Some(std::ptr::null()),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                None,
            )?;
            
            if handle.is_invalid() {
                return Err(anyhow::anyhow!("Failed to open file for reparse query"));
            }
            
            // Allocate buffer for reparse data
            let mut buffer = vec![0u8; 16384];
            
            let mut bytes_returned = 0u32;
            let result = DeviceIoControl(
                handle,
                FSCTL_GET_REPARSE_POINT,
                Some(std::ptr::null()),
                0,
                Some(buffer.as_mut_ptr() as *mut _),
                buffer.len() as u32,
                Some(&mut bytes_returned),
                Some(std::ptr::null()),
            );
            
            drop(handle);
            
            if result.is_err() {
                return Err(anyhow::anyhow!("Failed to get reparse point data"));
            }
            
            // Parse the reparse data based on type
            let reparse_data = &buffer as *const _ as *const REPARSE_DATA_BUFFER;
            let reparse_tag = (*reparse_data).ReparseTag;
            
            match reparse_tag {
                IO_REPARSE_TAG_MOUNT_POINT => {
                    // Junction point
                    let mount_point_data = &*(&buffer as *const _ as *const MountPointReparseBuffer);
                    let path_offset = mount_point_data.PathBufferOffset as usize;
                    let path_length = mount_point_data.PathBufferLength as usize;
                    
                    let path_bytes = &mount_point_data.PathBuffer[path_offset..path_offset + path_length];
                    let path_wide = std::slice::from_raw_parts(
                        path_bytes.as_ptr() as *const u16,
                        path_length / 2,
                    );
                    
                    // Remove the "\??\" prefix
                    let path_str = String::from_utf16_lossy(path_wide);
                    let trimmed = if path_str.starts_with(r"\??\") {
                        &path_str[4..]
                    } else {
                        &path_str
                    };
                    
                    Ok(PathBuf::from(trimmed))
                }
                IO_REPARSE_TAG_SYMLINK => {
                    // Symbolic link
                    let symlink_data = &*(&buffer as *const _ as *const SymbolicLinkReparseBuffer);
                    let path_offset = symlink_data.PathBufferOffset as usize;
                    let path_length = symlink_data.PathBufferLength as usize;
                    
                    let path_bytes = &symlink_data.PathBuffer[path_offset..path_offset + path_length];
                    let path_wide = std::slice::from_raw_parts(
                        path_bytes.as_ptr() as *const u16,
                        path_length / 2,
                    );
                    
                    let path_str = String::from_utf16_lossy(path_wide);
                    
                    // Remove the "\??\" prefix if present
                    let trimmed = if path_str.starts_with(r"\??\") {
                        &path_str[4..]
                    } else {
                        &path_str
                    };
                    
                    Ok(PathBuf::from(trimmed))
                }
                _ => Err(anyhow::anyhow!("Unsupported reparse tag: {}", reparse_tag)),
            }
        }
    }
    
    /// Resolve the final target of a chain of junctions/symlinks
    pub fn resolve_target(&self, path: &Path) -> Result<PathBuf> {
        let mut current = path.to_path_buf();
        let mut seen = std::collections::HashSet::new();
        
        loop {
            if seen.contains(&current) {
                return Err(anyhow::anyhow!("Circular reference in reparse points"));
            }
            seen.insert(current.clone());
            
            if !self.is_reparse_point(&current)? {
                return Ok(current);
            }
            
            current = self.get_target(&current)?;
            
            // If it's an absolute path, use it directly
            if current.is_absolute() {
                continue;
            }
            
            // Otherwise, resolve relative to parent
            if let Some(parent) = path.parent() {
                current = parent.join(current);
            }
        }
    }
}

/// Convert a Rust path to a wide string for Windows API
fn to_wide(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

// Reparse data buffer structures (simplified)
#[repr(C)]
struct MountPointReparseBuffer {
    SubstituteNameOffset: u16,
    SubstituteNameLength: u16,
    PrintNameOffset: u16,
    PrintNameLength: u16,
    PathBuffer: [u8; 0],
}

#[repr(C)]
struct SymbolicLinkReparseBuffer {
    SubstituteNameOffset: u16,
    SubstituteNameLength: u16,
    PrintNameOffset: u16,
    PrintNameLength: u16,
    Flags: u32,
    PathBuffer: [u8; 0],
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tempfile::TempDir;
    
    #[test]
    fn test_is_reparse_point() {
        let windows_api = Arc::new(WindowsApi::new().unwrap());
        let detector = JunctionDetector::new(windows_api);
        let temp_dir = TempDir::new().unwrap();
        
        // Regular directory should not be a reparse point
        assert!(!detector.is_reparse_point(temp_dir.path()).unwrap());
    }
    
    #[test]
    fn test_regular_file() {
        let windows_api = Arc::new(WindowsApi::new().unwrap());
        let detector = JunctionDetector::new(windows_api);
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "test").unwrap();
        
        assert!(!detector.is_reparse_point(&file_path).unwrap());
        assert!(!detector.is_junction(&file_path).unwrap());
        assert!(!detector.is_symlink(&file_path).unwrap());
    }
}
