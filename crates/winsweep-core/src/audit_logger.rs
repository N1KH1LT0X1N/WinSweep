//! Audit logging for WinSweep operations
//! 
//! This module provides comprehensive audit logging for all WinSweep operations,
//! including scans, cleanups, and configuration changes.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    /// Unique identifier for the entry
    pub id: Uuid,
    /// Timestamp of the entry
    pub timestamp: DateTime<Utc>,
    /// Type of operation
    pub operation: AuditOperation,
    /// User who performed the operation (if available)
    pub user: Option<String>,
    /// Process ID
    pub process_id: u32,
    /// Operation details
    pub details: AuditDetails,
    /// Success status
    pub success: bool,
    /// Error message (if any)
    pub error_message: Option<String>,
    /// Additional metadata
    pub metadata: serde_json::Value,
}

/// Types of operations that can be audited
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditOperation {
    /// Scan started
    ScanStart,
    /// Scan completed
    ScanComplete,
    /// Cleanup started
    CleanupStart,
    /// Cleanup completed
    CleanupComplete,
    /// File deleted
    FileDeleted,
    /// Directory deleted
    DirectoryDeleted,
    /// Configuration changed
    ConfigChanged,
    /// Process started with elevation
    ElevatedStart,
    /// Process ended
    ProcessEnd,
    /// IPC message sent
    IpcMessageSent,
    /// IPC message received
    IpcMessageReceived,
    /// Security violation
    SecurityViolation,
}

/// Details for audit operations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AuditDetails {
    /// Scan details
    Scan {
        scan_id: Uuid,
        paths: Vec<PathBuf>,
        include_hidden: bool,
        follow_symlinks: bool,
    },
    /// Scan result details
    ScanResult {
        scan_id: Uuid,
        items_found: u64,
        total_size_bytes: u64,
        duration_ms: u64,
    },
    /// Cleanup details
    Cleanup {
        cleanup_id: Uuid,
        items_to_delete: Vec<PathBuf>,
        use_recycle_bin: bool,
    },
    /// Cleanup result details
    CleanupResult {
        cleanup_id: Uuid,
        items_deleted: Vec<PathBuf>,
        items_failed: Vec<(PathBuf, String)>,
        space_freed_bytes: u64,
        duration_ms: u64,
    },
    /// File deletion details
    FileDeletion {
        file_path: PathBuf,
        file_size_bytes: u64,
        moved_to_recycle_bin: bool,
    },
    /// Configuration change details
    ConfigChange {
        key: String,
        old_value: Option<serde_json::Value>,
        new_value: Option<serde_json::Value>,
    },
    /// Process details
    Process {
        command_line: String,
        working_directory: PathBuf,
        elevated: bool,
    },
    /// IPC message details
    IpcMessage {
        message_type: String,
        message_id: Option<Uuid>,
        size_bytes: usize,
    },
    /// Security violation details
    SecurityViolation {
        violation_type: String,
        details: String,
        blocked: bool,
    },
}

/// Audit logger for WinSweep
pub struct AuditLogger {
    log_file: PathBuf,
    process_id: u32,
    user: Option<String>,
}

impl AuditLogger {
    /// Create a new audit logger
    pub fn new() -> Result<Self> {
        let log_dir = winsweep_common::config::get_data_dir();
        std::fs::create_dir_all(&log_dir)?;
        
        let log_file = log_dir.join("audit.log");
        
        // Get current user
        let user = std::env::var("USERNAME").ok();
        
        Ok(Self {
            log_file,
            process_id: std::process::id(),
            user,
        })
    }
    
    /// Log an audit entry
    pub fn log(&self, entry: AuditLogEntry) -> Result<()> {
        debug!("Writing audit log entry: {:?}", entry.operation);
        
        let line = serde_json::to_string(&entry)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file)?;
        
        writeln!(file, "{}", line)?;
        file.sync_all()?;
        
        Ok(())
    }
    
    /// Log a scan start
    pub fn log_scan_start(&self, scan_id: Uuid, paths: Vec<PathBuf>, config: &winsweep_common::types::ScanConfig) -> Result<()> {
        let entry = AuditLogEntry {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            operation: AuditOperation::ScanStart,
            user: self.user.clone(),
            process_id: self.process_id,
            details: AuditDetails::Scan {
                scan_id,
                paths,
                include_hidden: config.include_hidden,
                follow_symlinks: config.follow_symlinks,
            },
            success: true,
            error_message: None,
            metadata: serde_json::json!({
                "parallel_jobs": config.parallel_jobs,
                "max_file_size": config.max_file_size,
            }),
        };
        
        self.log(entry)
    }
    
    /// Log a scan completion
    pub fn log_scan_complete(&self, scan_id: Uuid, items_found: u64, total_size: u64, duration_ms: u64) -> Result<()> {
        let entry = AuditLogEntry {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            operation: AuditOperation::ScanComplete,
            user: self.user.clone(),
            process_id: self.process_id,
            details: AuditDetails::ScanResult {
                scan_id,
                items_found,
                total_size_bytes: total_size,
                duration_ms,
            },
            success: true,
            error_message: None,
            metadata: serde_json::json!({}),
        };
        
        self.log(entry)
    }
    
    /// Log a cleanup start
    pub fn log_cleanup_start(&self, cleanup_id: Uuid, items: Vec<PathBuf>, use_recycle_bin: bool) -> Result<()> {
        let entry = AuditLogEntry {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            operation: AuditOperation::CleanupStart,
            user: self.user.clone(),
            process_id: self.process_id,
            details: AuditDetails::Cleanup {
                cleanup_id,
                items_to_delete: items,
                use_recycle_bin,
            },
            success: true,
            error_message: None,
            metadata: serde_json::json!({}),
        };
        
        self.log(entry)
    }
    
    /// Log a cleanup completion
    pub fn log_cleanup_complete(&self, cleanup_id: Uuid, result: &winsweep_common::types::CleanupResult) -> Result<()> {
        let entry = AuditLogEntry {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            operation: AuditOperation::CleanupComplete,
            user: self.user.clone(),
            process_id: self.process_id,
            details: AuditDetails::CleanupResult {
                cleanup_id,
                items_deleted: result.items_deleted.clone(),
                items_failed: result.items_failed.clone(),
                space_freed_bytes: result.space_freed_bytes,
                duration_ms: result.duration_ms,
            },
            success: true,
            error_message: None,
            metadata: serde_json::json!({}),
        };
        
        self.log(entry)
    }
    
    /// Log a file deletion
    pub fn log_file_deletion(&self, file_path: PathBuf, file_size: u64, moved_to_recycle_bin: bool) -> Result<()> {
        let entry = AuditLogEntry {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            operation: AuditOperation::FileDeleted,
            user: self.user.clone(),
            process_id: self.process_id,
            details: AuditDetails::FileDeletion {
                file_path,
                file_size_bytes: file_size,
                moved_to_recycle_bin,
            },
            success: true,
            error_message: None,
            metadata: serde_json::json!({}),
        };
        
        self.log(entry)
    }
    
    /// Log a security violation
    pub fn log_security_violation(&self, violation_type: String, details: String, blocked: bool) -> Result<()> {
        let entry = AuditLogEntry {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            operation: AuditOperation::SecurityViolation,
            user: self.user.clone(),
            process_id: self.process_id,
            details: AuditDetails::SecurityViolation {
                violation_type,
                details,
                blocked,
            },
            success: !blocked,
            error_message: if blocked {
                Some("Operation blocked for security reasons".to_string())
            } else {
                None
            },
            metadata: serde_json::json!({}),
        };
        
        self.log(entry)
    }
    
    /// Read audit log entries
    pub fn read_entries(&self, limit: Option<usize>) -> Result<Vec<AuditLogEntry>> {
        let content = std::fs::read_to_string(&self.log_file)?;
        let mut entries = Vec::new();
        
        for line in content.lines().rev() {
            if let Ok(entry) = serde_json::from_str::<AuditLogEntry>(line) {
                entries.push(entry);
                
                if let Some(limit) = limit {
                    if entries.len() >= limit {
                        break;
                    }
                }
            }
        }
        
        Ok(entries)
    }
    
    /// Rotate the audit log if it exceeds a certain size
    pub fn rotate_if_needed(&self, max_size_mb: u32) -> Result<()> {
        let metadata = std::fs::metadata(&self.log_file)?;
        let size_mb = metadata.len() / (1024 * 1024);
        
        if size_mb > max_size_mb {
            let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
            let rotated_path = self.log_file.with_extension(format!("log.{}", timestamp));
            
            std::fs::rename(&self.log_file, &rotated_path)?;
            info!("Audit log rotated to {}", rotated_path.display());
        }
        
        Ok(())
    }
}

impl Default for AuditLogger {
    fn default() -> Self {
        Self::new().expect("Failed to create audit logger")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_audit_log_creation() {
        let temp_dir = TempDir::new().unwrap();
        let log_file = temp_dir.path().join("test.log");
        
        let entry = AuditLogEntry {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            operation: AuditOperation::ScanStart,
            user: Some("testuser".to_string()),
            process_id: 1234,
            details: AuditDetails::Scan {
                scan_id: Uuid::new_v4(),
                paths: vec![PathBuf::from("C:\\test")],
                include_hidden: false,
                follow_symlinks: false,
            },
            success: true,
            error_message: None,
            metadata: serde_json::json!({}),
        };
        
        let line = serde_json::to_string(&entry).unwrap();
        assert!(line.contains("ScanStart"));
    }
}
