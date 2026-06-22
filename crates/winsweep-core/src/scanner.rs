//! Parallel file system scanner
//!
//! This module implements a high-performance parallel scanner using tokio,
//! capable of scanning large directory structures efficiently.

use crate::junction_detector::JunctionDetector;
use crate::windows_api::WindowsApi;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use futures::stream::{self, StreamExt};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Semaphore};
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use winsweep_common::{
    never_delete::should_never_delete,
    project_signatures::detect_project_type,
    types::{FileType, ProjectType, ScanConfig, ScanResult},
};

/// Known artifact directories that should be reported as a single aggregated entry
/// rather than recursed into file-by-file.
///
/// Each entry is `(directory_name, lock_file_hint, project_type)`.
/// `lock_file_hint` is the name of the adjacent lock/manifest file whose mtime is
/// used as the "last project activity" age proxy.
static ARTIFACT_DIRS: &[(&str, &str, ProjectType)] = &[
    // JavaScript / Node
    ("node_modules", "package-lock.json", ProjectType::NodeJs),
    (".npm", "package.json", ProjectType::NodeJs),
    (".pnpm-store", "pnpm-lock.yaml", ProjectType::NodeJs),
    // Rust
    ("target", "Cargo.lock", ProjectType::Rust),
    // Python
    (".venv", "requirements.txt", ProjectType::Python),
    ("venv", "requirements.txt", ProjectType::Python),
    (".tox", "tox.ini", ProjectType::Python),
    ("__pycache__", "*.py", ProjectType::Python),
    (".pytest_cache", "pytest.ini", ProjectType::Python),
    (".mypy_cache", "mypy.ini", ProjectType::Python),
    // Java / JVM
    (".gradle", "build.gradle", ProjectType::Gradle),
    ("build", "build.gradle", ProjectType::Gradle),
    (".m2", "pom.xml", ProjectType::Maven),
    ("target", "pom.xml", ProjectType::Maven),
    // Go
    (".cache", "go.sum", ProjectType::Go),
    // .NET
    ("bin", "*.csproj", ProjectType::DotNet),
    ("obj", "*.csproj", ProjectType::DotNet),
    (".nuget", "*.csproj", ProjectType::DotNet),
    // Flutter / Dart
    (".dart_tool", "pubspec.lock", ProjectType::Flutter),
    ("build", "pubspec.yaml", ProjectType::Flutter),
    // Android
    (".gradle", "settings.gradle", ProjectType::Android),
    ("build", "AndroidManifest.xml", ProjectType::Android),
    // Nx / Turborepo
    (".nx", "nx.json", ProjectType::NodeJs),
    (".turbo", "turbo.json", ProjectType::NodeJs),
    // C/C++
    ("CMakeFiles", "CMakeLists.txt", ProjectType::CMake),
    // Unity
    ("Library", "ProjectSettings", ProjectType::Unity),
    ("Temp", "ProjectSettings", ProjectType::Unity),
];

/// Scanner for traversing file systems in parallel
pub struct Scanner {
    config: ScanConfig,
    windows_api: Arc<WindowsApi>,
    junction_detector: Arc<JunctionDetector>,
}

/// Handle for controlling an active scan
pub struct ScannerHandle {
    pub scan_id: Uuid,
    receiver: mpsc::UnboundedReceiver<ScanResult>,
    _join_handle: tokio::task::JoinHandle<Result<()>>,
}

impl Scanner {
    /// Create a new scanner with the given configuration
    pub fn new(config: ScanConfig) -> Result<Self> {
        let windows_api = Arc::new(WindowsApi::new()?);
        let junction_detector = Arc::new(JunctionDetector::new());

        Ok(Self {
            config,
            windows_api,
            junction_detector,
        })
    }

    /// Start scanning the configured paths
    pub async fn scan(&self) -> Result<ScannerHandle> {
        let scan_id = Uuid::new_v4();
        info!("Starting scan {}", scan_id);

        let (sender, receiver) = mpsc::unbounded_channel();

        let config = self.config.clone();
        let windows_api = self.windows_api.clone();
        let junction_detector = self.junction_detector.clone();

        let join_handle = tokio::spawn(async move {
            let start_time = Instant::now();
            let mut items_scanned = 0u64;

            // Determine parallelism
            let parallelism = config.parallel_jobs.unwrap_or_else(|| {
                std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(4)
            });

            info!("Using {} parallel workers", parallelism);
            let semaphore = Arc::new(Semaphore::new(parallelism));

            // Create stream of all directories to scan
            let mut directories = Vec::new();
            for path in &config.paths {
                if path.is_dir() {
                    directories.push(path.clone());
                } else {
                    // Single file
                    if let Ok(result) = scan_file(path, &windows_api, &junction_detector).await {
                        let _ = sender.send(result);
                        items_scanned += 1;
                    }
                }
            }

            // Process directories in parallel
            let dir_stream = stream::iter(directories)
                .map(move |dir| {
                    let semaphore = semaphore.clone();
                    let sender = sender.clone();
                    let config = config.clone();
                    let windows_api = windows_api.clone();
                    let junction_detector = junction_detector.clone();

                    async move {
                        let _permit = semaphore.acquire().await?;
                        scan_directory_recursive(
                            &dir,
                            &config,
                            &windows_api,
                            &junction_detector,
                            &sender,
                        )
                        .await
                    }
                })
                .buffer_unordered(parallelism);

            // Wait for all directories to complete
            let results: Vec<Result<()>> = dir_stream.collect().await;

            // Check for errors
            for result in results {
                if let Err(e) = result {
                    error!("Directory scan error: {}", e);
                }
            }

            let duration = start_time.elapsed();
            info!(
                "Scan {} completed in {:?}, scanned {} items",
                scan_id, duration, items_scanned
            );

            Ok(())
        });

        Ok(ScannerHandle {
            scan_id,
            receiver,
            _join_handle: join_handle,
        })
    }
}

impl ScannerHandle {
    /// Get the next scan result
    pub async fn next_result(&mut self) -> Option<ScanResult> {
        self.receiver.recv().await
    }

    /// Try to get the next scan result without blocking
    pub fn try_recv(&mut self) -> Option<ScanResult> {
        self.receiver.try_recv().ok()
    }

    /// Check whether the background scan task has finished
    pub fn is_finished(&self) -> bool {
        self._join_handle.is_finished()
    }

    /// Get all remaining scan results
    pub async fn collect_all(self) -> Vec<ScanResult> {
        let mut results = Vec::new();
        let mut handle = self;

        while let Some(result) = handle.next_result().await {
            results.push(result);
        }

        // Wait for the scan to complete
        let _ = handle._join_handle.await;

        results
    }
}

/// Recursively scan a directory, emitting a single aggregated `ScanResult` for every
/// known artifact sub-directory encountered (e.g. `node_modules`, `target`, `.gradle`).
/// Regular files and unknown directories are also reported individually so callers have
/// full visibility.
async fn scan_directory_recursive(
    dir: &Path,
    config: &ScanConfig,
    _windows_api: &WindowsApi,
    junction_detector: &JunctionDetector,
    sender: &mpsc::UnboundedSender<ScanResult>,
) -> Result<()> {
    debug!("Scanning directory: {}", dir.display());

    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(e) => e,
        Err(e) => {
            warn!("Failed to read directory {}: {}", dir.display(), e);
            return Ok(());
        }
    };

    let mut subdirs = Vec::new();

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        // Skip hidden files if requested (but not dot-named artifact dirs)
        if !config.include_hidden && is_hidden(&path) && !is_known_artifact_dir(&path) {
            continue;
        }

        // Check if it's a reparse point (junction/symlink)
        if junction_detector.is_reparse_point(&path)? {
            let file_type = if junction_detector.is_junction(&path)? {
                FileType::Junction
            } else {
                FileType::Symlink
            };

            // Don't follow symlinks unless requested
            if file_type == FileType::Symlink && !config.follow_symlinks {
                continue;
            }

            // Always scan junctions as they're local
            if let Ok(result) = scan_file(&path, _windows_api, junction_detector).await {
                let _ = sender.send(result);
            }

            continue;
        }

        if path.is_dir() {
            // Check if this is a well-known artifact directory
            if let Some(artifact_result) =
                try_scan_artifact_dir(&path, dir, config, junction_detector).await
            {
                let _ = sender.send(artifact_result);
                // Do NOT recurse — we already aggregated the whole subtree
            } else {
                subdirs.push(path);
            }
        } else {
            // Scan regular file
            if let Ok(result) = scan_file(&path, _windows_api, junction_detector).await {
                let _ = sender.send(result);
            }
        }
    }

    // Recursively scan non-artifact subdirectories
    for subdir in subdirs {
        if let Err(e) = Box::pin(scan_directory_recursive(
            &subdir,
            config,
            _windows_api,
            junction_detector,
            sender,
        ))
        .await
        {
            error!("Error scanning {}: {}", subdir.display(), e);
        }
    }

    Ok(())
}

/// Return `true` if `path` is one of the well-known artifact directory names.
fn is_known_artifact_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|name| {
            ARTIFACT_DIRS
                .iter()
                .any(|(artifact, _, _)| *artifact == name)
        })
        .unwrap_or(false)
}

/// If `path` is a known artifact directory, compute its total size and return a
/// single aggregated `ScanResult`.  Returns `None` if it is not a recognised
/// artifact directory or if the age filter rejects it.
async fn try_scan_artifact_dir(
    path: &Path,
    parent: &Path,
    config: &ScanConfig,
    junction_detector: &JunctionDetector,
) -> Option<ScanResult> {
    let dir_name = path.file_name()?.to_str()?;

    // Find the first matching artifact rule where the sibling lock file also exists
    let matched = ARTIFACT_DIRS.iter().find(|(artifact, lock_hint, _)| {
        if *artifact != dir_name {
            return false;
        }
        // If the lock hint contains a glob, just match by artifact name
        if lock_hint.contains('*') {
            return true;
        }
        // Otherwise require the sibling file to exist
        parent.join(lock_hint).exists()
    });

    let (_, lock_hint, project_type) = matched?;

    // Age filter: find the sibling lock/manifest file and compare its mtime
    if let Some(min_age) = config.min_age_days {
        let lock_path = if lock_hint.contains('*') {
            // No specific lock file — use the artifact dir mtime itself
            path.to_path_buf()
        } else {
            parent.join(lock_hint)
        };

        if let Ok(meta) = std::fs::metadata(&lock_path) {
            if let Ok(modified) = meta.modified() {
                let age = modified.elapsed().unwrap_or(Duration::ZERO);
                let threshold = Duration::from_secs(min_age as u64 * 86_400);
                if age < threshold {
                    debug!(
                        "Skipping {} — lock file newer than {} days",
                        path.display(),
                        min_age
                    );
                    return None;
                }
            }
        }
    }

    // Skip junctions/symlinks inside artifact dirs
    if junction_detector.is_reparse_point(path).unwrap_or(false) {
        return None;
    }

    // Aggregate size of the entire subtree
    let size_bytes = dir_size_sync(path);

    let last_modified = std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .map(DateTime::<Utc>::from)
        .unwrap_or_else(Utc::now);

    let is_safe_to_delete = !should_never_delete(path);
    let deletion_reason = if !is_safe_to_delete {
        Some("In NEVER_DELETE list".to_string())
    } else {
        None
    };

    Some(ScanResult {
        id: Uuid::new_v4(),
        path: path.to_path_buf(),
        size_bytes,
        file_type: FileType::Directory,
        project_type: Some(*project_type),
        last_modified,
        is_safe_to_delete,
        deletion_reason,
    })
}

/// Synchronously walk a directory tree and sum file sizes.
fn dir_size_sync(path: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                total += dir_size_sync(&p);
            } else if let Ok(meta) = entry.metadata() {
                total += meta.len();
            }
        }
    }
    total
}

/// Scan a single file or directory
async fn scan_file(
    path: &Path,
    _windows_api: &WindowsApi,
    junction_detector: &JunctionDetector,
) -> Result<ScanResult> {
    let metadata = tokio::fs::metadata(path)
        .await
        .with_context(|| format!("Failed to get metadata for {}", path.display()))?;

    let size_bytes = metadata.len();
    let last_modified = metadata
        .modified()
        .map(DateTime::<Utc>::from)
        .unwrap_or_else(|_| Utc::now());

    let file_type = if path.is_dir() {
        FileType::Directory
    } else if junction_detector.is_reparse_point(path)? {
        if junction_detector.is_junction(path)? {
            FileType::Junction
        } else {
            FileType::Symlink
        }
    } else {
        FileType::File
    };

    // Detect project type if it's a directory
    let project_type = if file_type == FileType::Directory {
        detect_project_type(path)
    } else {
        None
    };

    // Check if it's safe to delete
    let is_safe_to_delete = !should_never_delete(path);
    let deletion_reason = if !is_safe_to_delete {
        Some("In NEVER_DELETE list".to_string())
    } else {
        None
    };

    Ok(ScanResult {
        id: Uuid::new_v4(),
        path: path.to_path_buf(),
        size_bytes,
        file_type,
        project_type,
        last_modified,
        is_safe_to_delete,
        deletion_reason,
    })
}

/// Check if a path is hidden
fn is_hidden(path: &Path) -> bool {
    // Check Windows hidden attribute
    if let Ok(metadata) = std::fs::metadata(path) {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x2 != 0 {
            return true;
        }
    }

    // Check if name starts with .
    if let Some(name) = path.file_name() {
        if let Some(name_str) = name.to_str() {
            if name_str.starts_with('.') {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_scan_single_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        tokio::fs::write(&file_path, "test content").await.unwrap();

        let config = ScanConfig {
            paths: vec![file_path.clone()],
            ..Default::default()
        };

        let scanner = Scanner::new(config).unwrap();
        let mut handle = scanner.scan().await.unwrap();

        let result = handle.next_result().await.unwrap();
        assert_eq!(result.path, file_path);
        assert_eq!(result.size_bytes, 12);
        assert_eq!(result.file_type, FileType::File);
    }

    #[tokio::test]
    async fn test_scan_directory() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path();

        // Create some files
        tokio::fs::write(dir_path.join("file1.txt"), "content1")
            .await
            .unwrap();
        tokio::fs::write(dir_path.join("file2.txt"), "content2")
            .await
            .unwrap();

        let config = ScanConfig {
            paths: vec![dir_path.to_path_buf()],
            ..Default::default()
        };

        let scanner = Scanner::new(config).unwrap();
        let mut handle = scanner.scan().await.unwrap();

        let mut results = Vec::new();
        while let Some(result) = handle.next_result().await {
            results.push(result);
        }

        // Should have 2 results (2 files; directory itself is not reported)
        assert_eq!(results.len(), 2);
    }
}
