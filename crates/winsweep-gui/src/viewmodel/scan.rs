//! Scan view model

use serde::{Deserialize, Serialize};
use winsweep_core::Scanner;

/// Scan view model
#[derive(Serialize, Deserialize)]
pub struct ScanViewModel {
    /// Scan results
    pub scan_results: Vec<ScanResult>,
    /// Currently selected scan result
    pub selected_result: Option<usize>,
    /// Scan in progress
    pub scan_in_progress: bool,
    /// Scan progress (0.0 to 1.0)
    pub scan_progress: f32,
    /// Scan options
    pub scan_options: ScanOptions,
}

/// Scan result
#[derive(Serialize, Deserialize)]
pub struct ScanResult {
    pub path: String,
    pub size: u64,
    pub file_count: u64,
    pub directory_count: u64,
    pub last_modified: String,
    pub file_type: String,
}

/// Scan options
#[derive(Serialize, Deserialize)]
pub struct ScanOptions {
    pub path: String,
    pub include_hidden: bool,
    pub include_system: bool,
    pub min_file_size: u64,
    pub file_types: Vec<String>,
}

impl ScanViewModel {
    /// Create a new scan view model
    pub fn new() -> Self {
        Self {
            scan_results: Vec::new(),
            selected_result: None,
            scan_in_progress: false,
            scan_progress: 0.0,
            scan_options: ScanOptions {
                path: "C:\\".to_string(),
                include_hidden: false,
                include_system: false,
                min_file_size: 1024, // 1KB
                file_types: vec![],
            },
        }
    }

    /// Update the scan view model
    pub fn update(&mut self) {
        // TODO: Update scan progress if scanning
    }

    /// Start a new scan
    pub fn start_scan(&mut self, path: &str) {
        self.scan_in_progress = true;
        self.scan_progress = 0.0;
        self.scan_results.clear();

        // TODO: Implement actual scanning
    }

    /// Stop the current scan
    pub fn stop_scan(&mut self) {
        self.scan_in_progress = false;
        self.scan_progress = 0.0;
    }

    /// Delete selected scan result
    pub fn delete_selected(&mut self) {
        if let Some(index) = self.selected_result {
            if index < self.scan_results.len() {
                self.scan_results.remove(index);
                self.selected_result = None;
            }
        }
    }
}
