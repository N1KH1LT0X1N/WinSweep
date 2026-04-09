//! Cleanup operations manager
//! 
//! This module handles the actual deletion of files and directories,
//! with support for recycle bin, verification, and rollback.

use crate::windows_api::WindowsApi;
use crate::audit_logger::AuditLogger;
use anyhow::{Context, Result};
use chrono::Utc;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use winsweep_common::{
    never_delete::should_never_delete,
    types::{CleanupResult, ScanResult},
};

/// Manager for cleanup operations
pub struct CleanupManager {
    windows_api: Arc<WindowsApi>,
    audit_logger: Arc<AuditLogger>,
    use_recycle_bin: bool,
    require_confirmation: bool,
}

impl CleanupManager {
    /// Create a new cleanup manager
    pub fn new(
        windows_api: Arc<WindowsApi>,
        audit_logger: Arc<AuditLogger>,
        use_recycle_bin: bool,
        require_confirmation: bool,
    ) -> Self {
        Self {
            windows_api,
            audit_logger,
            use_recycle_bin,
            require_confirmation,
        }
    }
    
    /// Clean up the specified items
    pub async fn cleanup(&self, items: Vec<ScanResult>) -> Result<CleanupResult> {
        let cleanup_id = Uuid::new_v4();
        let start_time = std::time::Instant::now();
        
        info!("Starting cleanup {}", cleanup_id);
        
        // Log cleanup start
        self.audit_logger.log_cleanup_start(
            cleanup_id,
            items.iter().map(|i| i.path.clone()).collect(),
            self.use_recycle_bin,
        )?;
        
        let mut items_deleted = Vec::new();
        let mut items_failed = Vec::new();
        let mut space_freed = 0u64;
        
        for item in &items {
            // Double-check NEVER_DELETE list
            if should_never_delete(&item.path) {
                warn!("Skipping NEVER_DELETE path: {}", item.path.display());
                items_failed.push((item.path.clone(), "In NEVER_DELETE list".to_string()));
                continue;
            }
            
            // Check if file is locked
            if self.windows_api.is_file_locked(&item.path)? {
                warn!("File is locked: {}", item.path.display());
                items_failed.push((item.path.clone(), "File is locked".to_string()));
                continue;
            }
            
            // Perform deletion
            match self.delete_item(&item.path, item.size_bytes).await {
                Ok(_) => {
                    items_deleted.push(item.path.clone());
                    space_freed += item.size_bytes;
                    
                    // Log successful deletion
                    self.audit_logger.log_file_deletion(
                        item.path.clone(),
                        item.size_bytes,
                        self.use_recycle_bin,
                    )?;
                }
                Err(e) => {
                    error!("Failed to delete {}: {}", item.path.display(), e);
                    items_failed.push((item.path.clone(), e.to_string()));
                }
            }
        }
        
        let duration_ms = start_time.elapsed().as_millis() as u64;
        
        let result = CleanupResult {
            scan_id: Uuid::new_v4(), // Will be set by caller
            items_deleted,
            items_failed,
            space_freed_bytes: space_freed,
            duration_ms,
        };
        
        // Log cleanup completion
        self.audit_logger.log_cleanup_complete(cleanup_id, &result)?;
        
        info!(
            "Cleanup {} completed: {} items deleted, {} bytes freed in {}ms",
            cleanup_id,
            result.items_deleted.len(),
            space_freed,
            duration_ms
        );
        
        Ok(result)
    }
    
    /// Delete a single file or directory
    async fn delete_item(&self, path: &Path, size_bytes: u64) -> Result<()> {
        debug!("Deleting: {}", path.display());
        
        if self.use_recycle_bin {
            self.move_to_recycle_bin(path).await
        } else {
            self.permanent_delete(path).await
        }
    }
    
    /// Move item to recycle bin
    async fn move_to_recycle_bin(&self, path: &Path) -> Result<()> {
        // Use Windows Shell API to move to recycle bin
        // For now, we'll implement a simple version using PowerShell
        
        let ps_script = format!(
            "Add-Type -AssemblyName System.Windows.Forms; [Windows.Forms.SendKeys]::SendWait('{{}}'); $shell = New-Object -ComObject Shell.Application; $item = $shell.Namespace('{}').ParseName('{}'); $item.InvokeVerb('Delete')",
            path.parent().unwrap_or_else(|| Path::new("")).display(),
            path.file_name().unwrap_or_default().to_string_lossy()
        );
        
        let output = tokio::process::Command::new("powershell")
            .arg("-Command")
            .arg(ps_script)
            .output()
            .await
            .context("Failed to execute PowerShell for recycle bin")?;
        
        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "PowerShell failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        
        Ok(())
    }
    
    /// Permanently delete an item
    async fn permanent_delete(&self, path: &Path) -> Result<()> {
        if path.is_dir() {
            // Remove directory recursively
            fs::remove_dir_all(path).await
                .with_context(|| format!("Failed to remove directory: {}", path.display()))?;
        } else {
            // Remove file
            fs::remove_file(path).await
                .with_context(|| format!("Failed to remove file: {}", path.display()))?;
        }
        
        Ok(())
    }
    
    /// Verify that items were actually deleted
    pub async fn verify_deletion(&self, items: &[PathBuf]) -> Result<Vec<PathBuf>> {
        let mut not_deleted = Vec::new();
        
        for item in items {
            if item.exists() {
                warn!("Item still exists after deletion: {}", item.display());
                not_deleted.push(item.clone());
            }
        }
        
        Ok(not_deleted)
    }
    
    /// Create a restore point before cleanup (if configured)
    pub async fn create_restore_point(&self, description: &str) -> Result<()> {
        info!("Creating system restore point: {}", description);
        
        let ps_script = format!(
            "Checkpoint-Computer -Description '{}' -RestorePointType 'MODIFY_SETTINGS'",
            description
        );
        
        let output = tokio::process::Command::new("powershell")
            .arg("-Command")
            .arg(ps_script)
            .output()
            .await
            .context("Failed to create restore point")?;
        
        if !output.status.success() {
            warn!("Failed to create restore point: {}", 
                String::from_utf8_lossy(&output.stderr));
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use winsweep_common::types::{FileType, ProjectType};
    
    #[tokio::test]
    async fn test_cleanup_manager() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        tokio::fs::write(&file_path, "test content").await.unwrap();
        
        let windows_api = Arc::new(WindowsApi::new().unwrap());
        let audit_logger = Arc::new(AuditLogger::new().unwrap());
        
        let manager = CleanupManager::new(
            windows_api,
            audit_logger,
            false, // Don't use recycle bin in test
            false, // Don't require confirmation
        );
        
        let scan_result = ScanResult {
            id: Uuid::new_v4(),
            path: file_path.clone(),
            size_bytes: 12,
            file_type: FileType::File,
            project_type: None,
            last_modified: Utc::now(),
            is_safe_to_delete: true,
            deletion_reason: None,
        };
        
        let result = manager.cleanup(vec![scan_result]).await.unwrap();
        
        assert_eq!(result.items_deleted.len(), 1);
        assert_eq!(result.items_deleted[0], file_path);
        assert!(!file_path.exists());
    }
    
    #[tokio::test]
    async fn test_never_delete_protection() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("system32").join("test.txt");
        tokio::fs::create_dir_all(file_path.parent().unwrap()).await.unwrap();
        tokio::fs::write(&file_path, "test content").await.unwrap();
        
        let windows_api = Arc::new(WindowsApi::new().unwrap());
        let audit_logger = Arc::new(AuditLogger::new().unwrap());
        
        let manager = CleanupManager::new(
            windows_api,
            audit_logger,
            false,
            false,
        );
        
        let scan_result = ScanResult {
            id: Uuid::new_v4(),
            path: file_path.clone(),
            size_bytes: 12,
            file_type: FileType::File,
            project_type: None,
            last_modified: Utc::now(),
            is_safe_to_delete: false,
            deletion_reason: Some("In NEVER_DELETE list".to_string()),
        };
        
        let result = manager.cleanup(vec![scan_result]).await.unwrap();
        
        assert_eq!(result.items_deleted.len(), 0);
        assert_eq!(result.items_failed.len(), 1);
        assert!(file_path.exists());
    }
}
