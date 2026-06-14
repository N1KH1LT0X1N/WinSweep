//! Scan view model

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use winsweep_common::types::{FileType, ScanConfig, ScanResult as CommonScanResult};

/// Column that can be sorted
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortColumn {
    #[default]
    Path,
    Size,
    FileCount,
    LastModified,
}

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
    /// Active scanner handle (not persisted)
    #[serde(skip)]
    pub scan_handle: Option<winsweep_core::ScannerHandle>,
    /// Number of items received so far (for progress heuristics; not persisted)
    #[serde(skip)]
    pub items_received: u64,
    /// Pending category breakdown from the most recent completed scan (not persisted)
    #[serde(skip)]
    pub pending_category_breakdown: Option<super::dashboard::CategoryBreakdown>,
    /// Original common ScanResult values (needed for CleanupManager; not persisted)
    #[serde(skip)]
    pub raw_results: Vec<CommonScanResult>,
    /// Current sort column
    pub sort_column: SortColumn,
    /// Sort descending
    pub sort_descending: bool,
    /// Selected row indices for bulk operations
    #[serde(skip)]
    pub selected_rows: HashSet<usize>,
}

/// Scan result (view model)
#[derive(Serialize, Deserialize, Clone)]
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
            scan_handle: None,
            items_received: 0,
            pending_category_breakdown: None,
            raw_results: Vec::new(),
            sort_column: SortColumn::default(),
            sort_descending: false,
            selected_rows: HashSet::new(),
        }
    }

    /// Sort scan_results (and raw_results) in place according to the current sort state
    pub fn sort_results(&mut self) {
        let col = self.sort_column;
        let desc = self.sort_descending;
        // Build a vector of indices, sort them, then reorder both arrays
        let mut indices: Vec<usize> = (0..self.scan_results.len()).collect();
        indices.sort_by(|&ia, &ib| {
            let a = &self.scan_results[ia];
            let b = &self.scan_results[ib];
            let cmp = match col {
                SortColumn::Path => a.path.cmp(&b.path),
                SortColumn::Size => a.size.cmp(&b.size),
                SortColumn::FileCount => a.file_count.cmp(&b.file_count),
                SortColumn::LastModified => a.last_modified.cmp(&b.last_modified),
            };
            if desc {
                cmp.reverse()
            } else {
                cmp
            }
        });
        let new_scan_results: Vec<ScanResult> = indices
            .iter()
            .map(|&i| self.scan_results[i].clone())
            .collect();
        let new_raw_results: Vec<CommonScanResult> = indices
            .iter()
            .filter_map(|&i| self.raw_results.get(i).cloned())
            .collect();
        self.scan_results = new_scan_results;
        self.raw_results = new_raw_results;
        // After sorting, indices are stale — clear row selection
        self.selected_rows.clear();
    }

    /// Toggle sort on a column
    pub fn toggle_sort(&mut self, column: SortColumn) {
        if self.sort_column == column {
            self.sort_descending = !self.sort_descending;
        } else {
            self.sort_column = column;
            self.sort_descending = false;
        }
        self.sort_results();
    }

    /// Update the scan view model — drain any incoming results
    pub fn update(&mut self) {
        if !self.scan_in_progress {
            return;
        }

        if let Some(ref mut handle) = self.scan_handle {
            let mut received = 0;
            loop {
                match handle.try_recv() {
                    Some(result) => {
                        received += 1;
                        if result.size_bytes >= self.scan_options.min_file_size {
                            self.raw_results.push(result.clone());
                            self.scan_results.push(map_scan_result(&result));
                        }
                    }
                    None => break,
                }
            }
            self.items_received += received;

            if received > 0 {
                // Simple heuristic progress: grow toward 0.99 while items are arriving
                self.scan_progress = (self.scan_progress * 0.9 + 0.05).min(0.99);
            }

            if handle.is_finished() {
                self.scan_in_progress = false;
                self.scan_progress = 1.0;
                self.pending_category_breakdown =
                    Some(compute_category_breakdown(&self.scan_results));
                self.scan_handle = None;
            }
        } else {
            // No handle means the scan failed to start
            self.scan_in_progress = false;
            self.scan_progress = 0.0;
        }
    }

    /// Start a new scan (blocking the calling thread briefly to obtain the handle)
    pub fn start_scan(&mut self, path: &str, runtime: &tokio::runtime::Runtime) {
        self.scan_in_progress = true;
        self.scan_progress = 0.0;
        self.scan_results.clear();
        self.raw_results.clear();
        self.items_received = 0;
        self.selected_result = None;
        self.pending_category_breakdown = None;
        self.scan_handle = None;

        let config = ScanConfig {
            paths: vec![PathBuf::from(path)],
            include_hidden: self.scan_options.include_hidden,
            follow_symlinks: false,
            max_file_size: None,
            exclude_patterns: vec![],
            include_patterns: vec![],
            parallel_jobs: None,
            min_age_days: None,
        };

        let handle = runtime.block_on(async {
            let scanner = winsweep_core::Scanner::new(config)?;
            scanner.scan().await
        });

        match handle {
            Ok(h) => {
                self.scan_handle = Some(h);
            }
            Err(e) => {
                self.scan_in_progress = false;
                tracing::warn!("Failed to start scan: {}", e);
            }
        }
    }

    /// Stop the current scan
    pub fn stop_scan(&mut self) {
        self.scan_in_progress = false;
        self.scan_progress = 0.0;
        self.scan_handle = None;
    }

    /// Delete all scan results
    pub fn delete_all(&mut self) {
        self.scan_results.clear();
        self.raw_results.clear();
        self.selected_result = None;
    }
}

fn map_scan_result(result: &CommonScanResult) -> ScanResult {
    let (file_count, directory_count) = match result.file_type {
        FileType::File | FileType::Symlink | FileType::Junction => (1, 0),
        FileType::Directory => (0, 1),
    };

    ScanResult {
        path: result.path.display().to_string(),
        size: result.size_bytes,
        file_count,
        directory_count,
        last_modified: result.last_modified.format("%Y-%m-%d %H:%M").to_string(),
        file_type: format!("{:?}", result.file_type),
    }
}

/// Classify a path string into a reclaimable-space category.
///
/// Returns one of: `"Artifacts"`, `"Temp"`, `"Package Cache"`, `"Recycle Bin"`, `"Other"`.
pub fn categorize_path(path: &str) -> &'static str {
    let lower = path.to_lowercase();
    if lower.contains("node_modules")
        || lower.contains("target")
        || lower.contains(".gradle")
        || lower.contains("build")
        || lower.contains("__pycache__")
        || lower.contains(".dart_tool")
        || lower.contains(".nuget")
        || lower.contains("library")
        || lower.contains("cmakefiles")
    {
        "Artifacts"
    } else if lower.contains("temp")
        || lower.contains("tmp")
        || lower.contains("prefetch")
        || lower.contains("inetcache")
        || lower.contains("softwaredistribution\\download")
    {
        "Temp"
    } else if lower.contains("cache") || lower.contains(".npm") || lower.contains(".pnpm") {
        "Package Cache"
    } else if lower.contains("$recycle.bin") {
        "Recycle Bin"
    } else {
        "Other"
    }
}

fn compute_category_breakdown(results: &[ScanResult]) -> super::dashboard::CategoryBreakdown {
    let mut breakdown = super::dashboard::CategoryBreakdown::default();
    for r in results {
        match categorize_path(&r.path) {
            "Artifacts" => breakdown.artifact_bytes += r.size,
            "Temp" => breakdown.temp_bytes += r.size,
            "Package Cache" => breakdown.package_cache_bytes += r.size,
            "Recycle Bin" => breakdown.recycle_bin_bytes += r.size,
            _ => breakdown.other_bytes += r.size,
        }
    }
    breakdown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_categorize_artifacts() {
        assert_eq!(
            categorize_path(r"C:\project\node_modules\react"),
            "Artifacts"
        );
        assert_eq!(categorize_path(r"C:\project\target\debug"), "Artifacts");
        assert_eq!(categorize_path(r"C:\project\.gradle\caches"), "Artifacts");
        assert_eq!(categorize_path(r"C:\project\build\classes"), "Artifacts");
        assert_eq!(categorize_path(r"C:\project\__pycache__"), "Artifacts");
    }

    #[test]
    fn test_categorize_temp() {
        assert_eq!(categorize_path(r"C:\Windows\Temp\file.tmp"), "Temp");
        assert_eq!(
            categorize_path(r"C:\Users\user\AppData\Local\Temp\file.tmp"),
            "Temp"
        );
        assert_eq!(categorize_path(r"C:\Windows\Prefetch\some.pf"), "Temp");
    }

    #[test]
    fn test_categorize_package_cache() {
        assert_eq!(
            categorize_path(r"C:\Users\user\.npm\cache\something"),
            "Package Cache"
        );
        assert_eq!(
            categorize_path(r"C:\Users\user\AppData\Roaming\npm\cache"),
            "Package Cache"
        );
    }

    #[test]
    fn test_categorize_recycle_bin() {
        assert_eq!(
            categorize_path(r"C:\$Recycle.Bin\S-1-5-21\file.txt"),
            "Recycle Bin"
        );
    }

    #[test]
    fn test_categorize_other() {
        assert_eq!(
            categorize_path(r"C:\Users\user\Documents\report.pdf"),
            "Other"
        );
        assert_eq!(categorize_path(r"D:\Games\SomeGame\data.bin"), "Other");
    }

    #[test]
    fn test_compute_breakdown_empty() {
        let bd = compute_category_breakdown(&[]);
        assert_eq!(bd.artifact_bytes, 0);
        assert_eq!(bd.temp_bytes, 0);
        assert_eq!(bd.package_cache_bytes, 0);
        assert_eq!(bd.other_bytes, 0);
    }

    #[test]
    fn test_compute_breakdown_artifacts() {
        let r = ScanResult {
            path: r"C:\project\node_modules".to_string(),
            size: 2048,
            file_count: 10,
            directory_count: 1,
            last_modified: "2024-01-01 00:00".to_string(),
            file_type: "Directory".to_string(),
        };
        let bd = compute_category_breakdown(&[r]);
        assert_eq!(bd.artifact_bytes, 2048);
        assert_eq!(bd.temp_bytes, 0);
    }

    #[test]
    fn test_compute_breakdown_mixed() {
        let results = vec![
            ScanResult {
                path: r"C:\project\node_modules".to_string(),
                size: 1000,
                file_count: 5,
                directory_count: 0,
                last_modified: "2024-01-01 00:00".to_string(),
                file_type: "Directory".to_string(),
            },
            ScanResult {
                path: r"C:\Windows\Temp\file.tmp".to_string(),
                size: 500,
                file_count: 1,
                directory_count: 0,
                last_modified: "2024-01-01 00:00".to_string(),
                file_type: "File".to_string(),
            },
            ScanResult {
                path: r"C:\Users\user\Documents\file.pdf".to_string(),
                size: 200,
                file_count: 1,
                directory_count: 0,
                last_modified: "2024-01-01 00:00".to_string(),
                file_type: "File".to_string(),
            },
        ];
        let bd = compute_category_breakdown(&results);
        assert_eq!(bd.artifact_bytes, 1000);
        assert_eq!(bd.temp_bytes, 500);
        assert_eq!(bd.other_bytes, 200);
    }
}
