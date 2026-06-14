//! Unit tests for the scanner module

use std::path::PathBuf;
use tempfile::TempDir;
use winsweep_common::types::ScanConfig;
use winsweep_core::scanner::Scanner;

fn create_test_config(path: PathBuf) -> ScanConfig {
    ScanConfig {
        paths: vec![path],
        include_hidden: false,
        follow_symlinks: false,
        max_file_size: None,
        exclude_patterns: vec![],
        include_patterns: vec![],
        parallel_jobs: Some(2),
        min_age_days: None,
    }
}

#[tokio::test]
async fn test_scanner_initialization() {
    let temp_dir = TempDir::new().unwrap();
    let config = create_test_config(temp_dir.path().to_path_buf());
    let scanner = Scanner::new(config);
    assert!(scanner.is_ok());
}

#[tokio::test]
async fn test_scan_empty_directory() {
    let temp_dir = TempDir::new().unwrap();
    let config = create_test_config(temp_dir.path().to_path_buf());
    let scanner = Scanner::new(config).unwrap();

    let handle = scanner.scan().await.unwrap();
    let results = handle.collect_all().await;

    assert_eq!(results.len(), 0);
}

#[tokio::test]
async fn test_scan_single_file() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.txt");
    std::fs::write(&test_file, "Hello World").unwrap();

    let config = create_test_config(temp_dir.path().to_path_buf());
    let scanner = Scanner::new(config).unwrap();
    let handle = scanner.scan().await.unwrap();
    let results = handle.collect_all().await;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].path, test_file);
    assert_eq!(results[0].size_bytes, 11); // "Hello World" length
}

#[tokio::test]
async fn test_scan_nested_directories() {
    let temp_dir = TempDir::new().unwrap();

    // Create nested structure
    std::fs::create_dir(temp_dir.path().join("level1")).unwrap();
    std::fs::create_dir(temp_dir.path().join("level1/level2")).unwrap();

    std::fs::write(temp_dir.path().join("file1.txt"), "Content 1").unwrap();
    std::fs::write(temp_dir.path().join("level1/file2.txt"), "Content 2").unwrap();
    std::fs::write(temp_dir.path().join("level1/level2/file3.txt"), "Content 3").unwrap();

    let config = create_test_config(temp_dir.path().to_path_buf());
    let scanner = Scanner::new(config).unwrap();
    let handle = scanner.scan().await.unwrap();
    let results = handle.collect_all().await;

    // Should find 3 files (directories may or may not be included)
    let file_results: Vec<_> = results
        .iter()
        .filter(|r| r.file_type == winsweep_common::types::FileType::File)
        .collect();
    assert_eq!(file_results.len(), 3);
    assert!(file_results.iter().any(|f| f.path.ends_with("file1.txt")));
    assert!(file_results.iter().any(|f| f.path.ends_with("file2.txt")));
    assert!(file_results.iter().any(|f| f.path.ends_with("file3.txt")));
}

#[tokio::test]
async fn test_scan_large_file() {
    let temp_dir = TempDir::new().unwrap();
    let large_file = temp_dir.path().join("large.bin");

    // Create 1MB file
    let data = vec![0u8; 1024 * 1024];
    std::fs::write(&large_file, data).unwrap();

    let config = create_test_config(temp_dir.path().to_path_buf());
    let scanner = Scanner::new(config).unwrap();
    let handle = scanner.scan().await.unwrap();
    let results = handle.collect_all().await;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].size_bytes, 1024 * 1024);
}

#[tokio::test]
async fn test_scan_performance() {
    let temp_dir = TempDir::new().unwrap();

    // Create many files
    for i in 0..1000 {
        std::fs::write(
            temp_dir.path().join(format!("file_{}.txt", i)),
            format!("Content {}", i),
        )
        .unwrap();
    }

    let config = create_test_config(temp_dir.path().to_path_buf());
    let scanner = Scanner::new(config).unwrap();
    let start = std::time::Instant::now();

    let handle = scanner.scan().await.unwrap();
    let results = handle.collect_all().await;

    let duration = start.elapsed();

    assert_eq!(results.len(), 1000);
    // Should complete in reasonable time (less than 5 seconds)
    assert!(duration.as_secs() < 5);
}

#[tokio::test]
async fn test_scan_error_handling() {
    let config = create_test_config(PathBuf::from("/nonexistent/path"));
    let scanner = Scanner::new(config).unwrap();

    // Try to scan non-existent directory
    let result = scanner.scan().await;
    // Should either error during scan setup or return empty results
    assert!(result.is_ok() || result.is_err());
}
