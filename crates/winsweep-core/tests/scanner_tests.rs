//! Unit tests for the scanner module

use std::path::PathBuf;
use tempfile::TempDir;
use winsweep_core::scanner::{Scanner, ScanItem};
use winsweep_common::NeverDeleteList;

#[tokio::test]
async fn test_scanner_initialization() {
    let scanner = Scanner::new().await;
    assert!(scanner.is_ok());
}

#[tokio::test]
async fn test_scan_empty_directory() {
    let temp_dir = TempDir::new().unwrap();
    let scanner = Scanner::new().await.unwrap();
    
    let mut results = Vec::new();
    scanner.scan_directory(temp_dir.path(), &mut |item| {
        results.push(item.clone());
        true
    }).await.unwrap();
    
    assert_eq!(results.len(), 0);
}

#[tokio::test]
async fn test_scan_single_file() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.txt");
    std::fs::write(&test_file, "Hello World").unwrap();
    
    let scanner = Scanner::new().await.unwrap();
    let mut results = Vec::new();
    scanner.scan_directory(temp_dir.path(), &mut |item| {
        results.push(item.clone());
        true
    }).await.unwrap();
    
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].path, test_file);
    assert_eq!(results[0].size, 11); // "Hello World" length
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
    
    let scanner = Scanner::new().await.unwrap();
    let mut results = Vec::new();
    scanner.scan_directory(temp_dir.path(), &mut |item| {
        results.push(item.clone());
        true
    }).await.unwrap();
    
    assert_eq!(results.len(), 3);
    assert!(results.iter().any(|f| f.path.ends_with("file1.txt")));
    assert!(results.iter().any(|f| f.path.ends_with("file2.txt")));
    assert!(results.iter().any(|f| f.path.ends_with("file3.txt")));
}

#[tokio::test]
async fn test_scan_filter_callback() {
    let temp_dir = TempDir::new().unwrap();
    
    std::fs::write(temp_dir.path().join("keep.txt"), "Keep this").unwrap();
    std::fs::write(temp_dir.path().join("delete.txt"), "Delete this").unwrap();
    
    let scanner = Scanner::new().await.unwrap();
    let mut results = Vec::new();
    scanner.scan_directory(temp_dir.path(), &mut |item| {
        let should_keep = item.path.ends_with("keep.txt");
        if should_keep {
            results.push(item.clone());
        }
        should_keep
    }).await.unwrap();
    
    assert_eq!(results.len(), 1);
    assert!(results[0].path.ends_with("keep.txt"));
}

#[tokio::test]
async fn test_scan_hidden_files() {
    let temp_dir = TempDir::new().unwrap();
    
    // Create hidden file (Windows)
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        let hidden_file = temp_dir.path().join("hidden.txt");
        std::fs::write(&hidden_file, "Hidden content").unwrap();
        
        // Set hidden attribute
        let mut attrs = std::fs::metadata(&hidden_file).unwrap().file_attributes();
        attrs |= 0x2; // FILE_ATTRIBUTE_HIDDEN
        std::fs::set_file_attributes(&hidden_file, attrs).unwrap();
    }
    
    let scanner = Scanner::new().await.unwrap();
    let mut results = Vec::new();
    scanner.scan_directory(temp_dir.path(), &mut |item| {
        results.push(item.clone());
        true
    }).await.unwrap();
    
    // Should find hidden files by default
    #[cfg(windows)]
    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn test_scan_large_file() {
    let temp_dir = TempDir::new().unwrap();
    let large_file = temp_dir.path().join("large.bin");
    
    // Create 1MB file
    let data = vec![0u8; 1024 * 1024];
    std::fs::write(&large_file, data).unwrap();
    
    let scanner = Scanner::new().await.unwrap();
    let mut results = Vec::new();
    scanner.scan_directory(temp_dir.path(), &mut |item| {
        results.push(item.clone());
        true
    }).await.unwrap();
    
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].size, 1024 * 1024);
}

#[tokio::test]
async fn test_scan_with_never_delete_list() {
    let temp_dir = TempDir::new().unwrap();
    let never_delete = NeverDeleteList::default();
    
    // Create a protected file type
    let protected_file = temp_dir.path().join("test.exe");
    std::fs::write(&protected_file, "Executable content").unwrap();
    
    // Create a normal file
    let normal_file = temp_dir.path().join("test.txt");
    std::fs::write(&normal_file, "Normal content").unwrap();
    
    let scanner = Scanner::new().await.unwrap();
    let mut results = Vec::new();
    scanner.scan_directory(temp_dir.path(), &mut |item| {
        // Filter out protected files
        if !never_delete.is_protected(&item.path) {
            results.push(item.clone());
        }
        true
    }).await.unwrap();
    
    // Should only find the normal file
    assert_eq!(results.len(), 1);
    assert!(results[0].path.ends_with("test.txt"));
}

#[tokio::test]
async fn test_scan_performance() {
    let temp_dir = TempDir::new().unwrap();
    
    // Create many files
    for i in 0..1000 {
        std::fs::write(
            temp_dir.path().join(format!("file_{}.txt", i)),
            format!("Content {}", i)
        ).unwrap();
    }
    
    let scanner = Scanner::new().await.unwrap();
    let start = std::time::Instant::now();
    
    let mut count = 0;
    scanner.scan_directory(temp_dir.path(), &mut |_| {
        count += 1;
        true
    }).await.unwrap();
    
    let duration = start.elapsed();
    
    assert_eq!(count, 1000);
    // Should complete in reasonable time (less than 5 seconds)
    assert!(duration.as_secs() < 5);
}

#[test]
fn test_scan_item_properties() {
    let path = PathBuf::from("/test/file.txt");
    let item = ScanItem {
        path: path.clone(),
        size: 1024,
        is_directory: false,
        is_hidden: false,
        last_modified: std::time::SystemTime::now(),
    };
    
    assert_eq!(item.path, path);
    assert_eq!(item.size, 1024);
    assert!(!item.is_directory);
    assert!(!item.is_hidden);
}

#[tokio::test]
async fn test_scan_error_handling() {
    let scanner = Scanner::new().await.unwrap();
    
    // Try to scan non-existent directory
    let result = scanner.scan_directory(
        &PathBuf::from("/nonexistent/path"),
        &mut |_| true
    ).await;
    
    assert!(result.is_err());
}

#[tokio::test]
async fn test_scan_symbolic_links() {
    #[cfg(unix)]
    {
        let temp_dir = TempDir::new().unwrap();
        
        // Create target file
        let target_file = temp_dir.path().join("target.txt");
        std::fs::write(&target_file, "Target content").unwrap();
        
        // Create symbolic link
        let link_file = temp_dir.path().join("link.txt");
        std::os::unix::fs::symlink(&target_file, &link_file).unwrap();
        
        let scanner = Scanner::new().await.unwrap();
        let mut results = Vec::new();
        scanner.scan_directory(temp_dir.path(), &mut |item| {
            results.push(item.clone());
            true
        }).await.unwrap();
        
        // Should find both files (or handle symlinks appropriately)
        assert!(results.len() >= 1);
    }
}
