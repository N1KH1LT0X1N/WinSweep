//! Common types used across WinSweep components

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// A scan result containing information about a file or directory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub id: Uuid,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub file_type: FileType,
    pub project_type: Option<ProjectType>,
    pub last_modified: DateTime<Utc>,
    pub is_safe_to_delete: bool,
    pub deletion_reason: Option<String>,
}

/// File or directory type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileType {
    File,
    Directory,
    Symlink,
    Junction,
}

/// Project type detected from file signatures
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectType {
    // JavaScript/TypeScript
    NodeJs,
    TypeScript,
    React,
    Vue,
    Angular,
    Svelte,
    
    // Rust
    Rust,
    
    // Python
    Python,
    Django,
    Flask,
    FastAPI,
    
    // Java
    Java,
    Maven,
    Gradle,
    
    // Go
    Go,
    
    // C/C++
    Cpp,
    CMake,
    
    // .NET
    DotNet,
    
    // Ruby
    Ruby,
    Rails,
    
    // PHP
    Php,
    Laravel,
    
    // Mobile
    Android,
    Flutter,
    ReactNative,
    
    // Infrastructure
    Docker,
    Kubernetes,
    Terraform,
    Ansible,
    Packer,
    Vagrant,
    
    // Data
    Jupyter,
    R,
    
    // Game Development
    Unity,
    Unreal,
    
    // Other
    Git,
    Hg,
    Svn,
}

/// Cleanup operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupResult {
    pub scan_id: Uuid,
    pub items_deleted: Vec<PathBuf>,
    pub items_failed: Vec<(PathBuf, String)>,
    pub space_freed_bytes: u64,
    pub duration_ms: u64,
}

/// IPC message types between GUI and elevated scanner
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcMessage {
    /// Start a scan request
    StartScan { 
        paths: Vec<PathBuf>,
        include_hidden: bool,
        follow_symlinks: bool,
    },
    /// Scan progress update
    ScanProgress {
        items_scanned: u64,
        current_path: PathBuf,
        results: Vec<ScanResult>,
    },
    /// Scan completed
    ScanComplete {
        results: Vec<ScanResult>,
        total_size_bytes: u64,
    },
    /// Request cleanup of specific items
    CleanupItems {
        items: Vec<PathBuf>,
    },
    /// Cleanup progress update
    CleanupProgress {
        items_processed: u64,
        items_total: u64,
        space_freed: u64,
    },
    /// Cleanup completed
    CleanupComplete {
        result: CleanupResult,
    },
    /// Error occurred
    Error {
        message: String,
        code: Option<String>,
    },
    /// Ping/pong for connection health
    Ping,
    Pong,
}

/// Configuration for scanning behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanConfig {
    pub paths: Vec<PathBuf>,
    pub include_hidden: bool,
    pub follow_symlinks: bool,
    pub max_file_size: Option<u64>,
    pub exclude_patterns: Vec<String>,
    pub include_patterns: Vec<String>,
    pub parallel_jobs: Option<usize>,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            paths: vec![PathBuf::from(".")],
            include_hidden: false,
            follow_symlinks: false,
            max_file_size: Some(1024 * 1024 * 1024), // 1GB
            exclude_patterns: vec![
                "*.tmp".to_string(),
                "*.temp".to_string(),
                "*.log".to_string(),
            ],
            include_patterns: vec![],
            parallel_jobs: None,
        }
    }
}
