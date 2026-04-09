//! Integration tests for WinSweep
//! 
//! These tests validate the complete functionality of all modules working together.

use std::path::PathBuf;
use tempfile::TempDir;
use winsweep_core::{Scanner, CleanupManager, WindowsEditionDetector, WslDetector, DockerClient, PackageManagerRegistry};
use winsweep_common::{Config, NeverDeleteList};
use winsweep_cli::App;
use tokio::runtime::Runtime;

/// Test basic scanning functionality
#[tokio::test]
async fn test_basic_scanning() {
    // Create a temporary directory with test files
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path();
    
    // Create test files
    std::fs::write(test_path.join("test1.txt"), "Hello World").unwrap();
    std::fs::write(test_path.join("test2.txt"), "Hello World 2").unwrap();
    
    // Create subdirectory
    std::fs::create_dir(test_path.join("subdir")).unwrap();
    std::fs::write(test_path.join("subdir/test3.txt"), "Hello World 3").unwrap();
    
    // Initialize scanner
    let scanner = Scanner::new().await.unwrap();
    
    // Scan the directory
    let mut results = Vec::new();
    scanner.scan_directory(test_path, &mut |item| {
        results.push(item.clone());
        true
    }).await.unwrap();
    
    // Verify results
    assert_eq!(results.len(), 3);
    assert!(results.iter().any(|f| f.path.ends_with("test1.txt")));
    assert!(results.iter().any(|f| f.path.ends_with("test2.txt")));
    assert!(results.iter().any(|f| f.path.ends_with("test3.txt")));
}

/// Test cleanup functionality
#[tokio::test]
async fn test_cleanup_operations() {
    // Create a temporary directory with test files
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path();
    
    // Create test files
    std::fs::write(test_path.join("delete_me.txt"), "Delete this").unwrap();
    std::fs::write(test_path.join("keep_me.txt"), "Keep this").unwrap();
    
    // Initialize cleanup manager
    let cleanup = CleanupManager::new().unwrap();
    
    // Delete specific file
    let file_to_delete = test_path.join("delete_me.txt");
    cleanup.delete_file(&file_to_delete, false).await.unwrap();
    
    // Verify file was deleted
    assert!(!file_to_delete.exists());
    
    // Verify other file still exists
    assert!(test_path.join("keep_me.txt").exists());
}

/// Test Windows edition detection
#[test]
fn test_windows_edition_detection() {
    // This test only runs on Windows
    #[cfg(not(windows))]
    return;
    
    let detector = WindowsEditionDetector::new();
    
    if let Some(detector) = detector {
        let edition = detector.get_edition();
        assert!(!edition.is_empty());
        
        let version = detector.get_version();
        assert!(!version.is_empty());
        
        let is_home = detector.is_home_edition();
        // Should not panic
    }
}

/// Test WSL detection
#[test]
fn test_wsl_detection() {
    // This test only runs on Windows with WSL installed
    #[cfg(not(windows))]
    return;
    
    let detector = WslDetector::new();
    
    if let Some(detector) = detector {
        let is_installed = detector.is_wsl_installed();
        let distributions = detector.get_distributions();
        
        // Should not panic even if WSL is not installed
        if is_installed {
            assert!(!distributions.is_empty());
        }
    }
}

/// Test Docker client
#[tokio::test]
async fn test_docker_client() {
    // This test requires Docker to be installed and running
    let client = match DockerClient::new().await {
        Ok(client) => client,
        Err(_) => {
            println!("Docker not available, skipping test");
            return;
        }
    };
    
    // Test daemon status
    let is_running = client.is_daemon_running();
    println!("Docker daemon running: {}", is_running);
    
    if is_running {
        // Test listing containers
        let containers = client.get_containers().await.unwrap();
        println!("Found {} containers", containers.len());
        
        // Test listing images
        let images = client.get_images().await.unwrap();
        println!("Found {} images", images.len());
    }
}

/// Test package manager registry
#[test]
fn test_package_manager_registry() {
    let registry = PackageManagerRegistry::new();
    
    // Test npm detection
    let npm = registry.get_manager("npm");
    if let Some(npm) = npm {
        let is_installed = npm.is_installed();
        println!("npm installed: {}", is_installed);
        
        if is_installed {
            // Test cache detection
            let cache_paths = npm.get_cache_paths();
            assert!(!cache_paths.is_empty());
        }
    }
    
    // Test pip detection
    let pip = registry.get_manager("pip");
    if let Some(pip) = pip {
        let is_installed = pip.is_installed();
        println!("pip installed: {}", is_installed);
    }
    
    // Test cargo detection
    let cargo = registry.get_manager("cargo");
    if let Some(cargo) = cargo {
        let is_installed = cargo.is_installed();
        println!("cargo installed: {}", is_installed);
    }
    
    // Test nuget detection
    let nuget = registry.get_manager("nuget");
    if let Some(nuget) = nuget {
        let is_installed = nuget.is_installed();
        println!("nuget installed: {}", is_installed);
    }
}

/// Test configuration loading and saving
#[test]
fn test_configuration() {
    let config = Config::default();
    
    // Test default values
    assert_eq!(config.scan_include_hidden, false);
    assert_eq!(config.cleanup.use_recycle_bin, true);
    assert_eq!(config.ui.theme, "system");
    
    // Test serialization
    let toml_str = toml::to_string_pretty(&config).unwrap();
    let parsed: Config = toml::from_str(&toml_str).unwrap();
    
    assert_eq!(config.scan_include_hidden, parsed.scan_include_hidden);
    assert_eq!(config.cleanup.use_recycle_bin, parsed.cleanup.use_recycle_bin);
}

/// Test NEVER_DELETE list
#[test]
fn test_never_delete_list() {
    let never_delete = NeverDeleteList::default();
    
    // Test system paths
    assert!(never_delete.is_protected(&PathBuf::from("C:\\Windows")));
    assert!(never_delete.is_protected(&PathBuf::from("C:\\Program Files")));
    assert!(never_delete.is_protected(&PathBuf::from("C:\\Program Files (x86)")));
    
    // Test file extensions
    assert!(never_delete.is_protected(&PathBuf::from("test.exe")));
    assert!(never_delete.is_protected(&PathBuf::from("test.dll")));
    assert!(never_delete.is_protected(&PathBuf::from("test.sys")));
    
    // Test non-protected paths
    assert!(!never_delete.is_protected(&PathBuf::from("C:\\Temp\\test.txt")));
    assert!(!never_delete.is_protected(&PathBuf::from("test.txt")));
}

/// Test CLI app initialization
#[test]
fn test_cli_app_initialization() {
    let rt = Runtime::new().unwrap();
    
    // This should not panic
    let _app = rt.block_on(async {
        App::new().await
    });
}

/// Test parallel scanning performance
#[tokio::test]
async fn test_parallel_scanning() {
    // Create a temporary directory with many files
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path();
    
    // Create 100 test files
    for i in 0..100 {
        std::fs::write(test_path.join(format!("test_{}.txt", i)), format!("Content {}", i)).unwrap();
    }
    
    // Initialize scanner
    let scanner = Scanner::new().await.unwrap();
    
    // Scan with parallelism
    let start = std::time::Instant::now();
    let mut results = Vec::new();
    scanner.scan_directory(test_path, &mut |item| {
        results.push(item.clone());
        true
    }).await.unwrap();
    let duration = start.elapsed();
    
    // Verify all files were found
    assert_eq!(results.len(), 100);
    
    // Performance should be reasonable (less than 1 second for 100 files)
    assert!(duration.as_secs() < 1);
}

/// Test junction detection
#[test]
fn test_junction_detection() {
    // This test only runs on Windows
    #[cfg(not(windows))]
    return;
    
    use winsweep_core::JunctionDetector;
    
    let detector = JunctionDetector::new();
    
    // Test with a known junction (Windows Documents folder)
    let documents_path = dirs::document_dir().unwrap_or_else(|| PathBuf::from("C:\\Users\\Public\\Documents"));
    
    if let Ok(is_junction) = detector.is_junction(&documents_path) {
        // Should not panic
        println!("Documents folder is junction: {}", is_junction);
    }
}

/// Test audit logging
#[tokio::test]
async fn test_audit_logging() {
    use winsweep_core::AuditLogger;
    
    let temp_dir = TempDir::new().unwrap();
    let log_path = temp_dir.path().join("audit.log");
    
    let logger = AuditLogger::new(&log_path).unwrap();
    
    // Log some operations
    logger.log_scan_start(&["C:\\Temp"]).unwrap();
    logger.log_file_deleted(&PathBuf::from("C:\\Temp\\test.txt")).unwrap();
    logger.log_cleanup_complete(1024, 5).unwrap();
    
    // Verify log was created
    assert!(log_path.exists());
    
    // Read and verify log content
    let log_content = std::fs::read_to_string(&log_path).unwrap();
    assert!(log_content.contains("scan_start"));
    assert!(log_content.contains("file_deleted"));
    assert!(log_content.contains("cleanup_complete"));
}

/// Test error handling
#[tokio::test]
async fn test_error_handling() {
    use winsweep_core::WindowsApiError;
    
    // Test with non-existent path
    let scanner = Scanner::new().await.unwrap();
    let result = scanner.scan_directory(&PathBuf::from("Z:\\nonexistent"), &mut |_| true).await;
    
    // Should handle error gracefully
    assert!(result.is_err());
    
    // Test cleanup with non-existent file
    let cleanup = CleanupManager::new().unwrap();
    let result = cleanup.delete_file(&PathBuf::from("Z:\\nonexistent.txt"), false).await;
    
    // Should handle error gracefully
    assert!(result.is_err());
}
