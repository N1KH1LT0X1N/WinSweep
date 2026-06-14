//! Integration tests for WinSweep
//!
//! These tests validate the complete functionality of all modules working together.

use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use winsweep_common::{
    never_delete::should_never_delete,
    types::{FileType, ScanConfig, ScanResult},
    Config,
};
use winsweep_core::{
    audit_logger::AuditLogger, cleanup::CleanupManager, junction_detector::JunctionDetector,
    scanner::Scanner, windows_api::WindowsApi, DockerClient, PackageManagerRegistry,
    WindowsEditionDetector, WslDetector,
};

// ── CLI NDJSON mode ────────────────────────────────────────────────────────

/// Test that `winsweep-cli --output ndjson <dir>` emits valid JSON lines with expected fields.
#[test]
fn test_cli_ndjson_output() {
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path();

    std::fs::write(test_path.join("alpha.txt"), "alpha content").unwrap();
    std::fs::write(test_path.join("beta.txt"), "beta content").unwrap();
    std::fs::create_dir(test_path.join("gamma")).unwrap();
    std::fs::write(test_path.join("gamma/delta.txt"), "delta content").unwrap();

    // Run via cargo so the binary is built on demand
    let output = std::process::Command::new("cargo")
        .args([
            "run",
            "-q",
            "-p",
            "winsweep-cli",
            "--",
            "--output",
            "ndjson",
            test_path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to execute winsweep-cli via cargo");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "winsweep-cli exited with non-zero status. stdout: {stdout}\nstderr: {stderr}"
    );

    let lines: Vec<&str> = stdout.lines().filter(|l| !l.trim().is_empty()).collect();
    assert!(
        !lines.is_empty(),
        "ndjson mode should produce at least one line"
    );

    for line in &lines {
        let value: serde_json::Value =
            serde_json::from_str(line).expect("each line should be valid JSON");
        assert!(value.is_object(), "each line should be a JSON object");
        assert!(
            value.get("path").is_some(),
            "each object should have a 'path' field"
        );
        assert!(
            value.get("size_bytes").is_some(),
            "each object should have a 'size_bytes' field"
        );
        assert!(
            value.get("file_type").is_some(),
            "each object should have a 'file_type' field"
        );
    }
}

// ── Scanner ──────────────────────────────────────────────────────────────────

/// Test basic scanning functionality
#[tokio::test]
async fn test_basic_scanning() {
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path();

    std::fs::write(test_path.join("test1.txt"), "Hello World").unwrap();
    std::fs::write(test_path.join("test2.txt"), "Hello World 2").unwrap();
    std::fs::create_dir(test_path.join("subdir")).unwrap();
    std::fs::write(test_path.join("subdir/test3.txt"), "Hello World 3").unwrap();

    let config = ScanConfig {
        paths: vec![test_path.to_path_buf()],
        include_hidden: true,
        follow_symlinks: false,
        max_file_size: None,
        exclude_patterns: vec![],
        include_patterns: vec![],
        parallel_jobs: Some(1),
        min_age_days: None,
    };

    let scanner = Scanner::new(config).unwrap();
    let handle = scanner.scan().await.unwrap();
    let results = handle.collect_all().await;

    assert!(!results.is_empty(), "scan should return results");
    let paths: Vec<&PathBuf> = results.iter().map(|r| &r.path).collect();
    assert!(
        paths.iter().any(|p| p.ends_with("test1.txt")),
        "should find test1.txt"
    );
    assert!(
        paths.iter().any(|p| p.ends_with("test2.txt")),
        "should find test2.txt"
    );
}

/// Test parallel scanning with many files
#[tokio::test]
async fn test_parallel_scanning() {
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path();

    for i in 0..20 {
        std::fs::write(
            test_path.join(format!("file_{}.txt", i)),
            format!("content {}", i),
        )
        .unwrap();
    }

    let config = ScanConfig {
        paths: vec![test_path.to_path_buf()],
        include_hidden: false,
        follow_symlinks: false,
        max_file_size: None,
        exclude_patterns: vec![],
        include_patterns: vec![],
        parallel_jobs: Some(4),
        min_age_days: None,
    };

    let scanner = Scanner::new(config).unwrap();
    let handle = scanner.scan().await.unwrap();
    let results = handle.collect_all().await;

    assert_eq!(results.len(), 20, "should find all 20 files");
}

// ── CleanupManager ───────────────────────────────────────────────────────────

/// Test cleanup operations
#[tokio::test]
async fn test_cleanup_operations() {
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path();

    let file_to_delete = test_path.join("delete_me.txt");
    std::fs::write(&file_to_delete, "Delete this").unwrap();
    std::fs::write(test_path.join("keep_me.txt"), "Keep this").unwrap();

    assert!(file_to_delete.exists());

    let api = Arc::new(WindowsApi::new().unwrap());
    let logger = Arc::new(AuditLogger::new().unwrap());
    let cleanup = CleanupManager::new(api, logger, false, false, false);

    // Build a minimal ScanResult for the file
    let scan_result = ScanResult {
        id: uuid::Uuid::new_v4(),
        path: file_to_delete.clone(),
        size_bytes: 11,
        file_type: FileType::File,
        project_type: None,
        last_modified: chrono::Utc::now(),
        is_safe_to_delete: true,
        deletion_reason: None,
    };

    let result = cleanup.cleanup(vec![scan_result]).await.unwrap();

    assert!(!file_to_delete.exists(), "deleted file should be gone");
    assert!(
        test_path.join("keep_me.txt").exists(),
        "kept file should remain"
    );
    assert_eq!(result.items_deleted.len(), 1);
    assert!(result.items_failed.is_empty());
}

// ── AuditLogger ──────────────────────────────────────────────────────────────

/// Test audit logging
#[test]
fn test_audit_logging() {
    let logger = AuditLogger::new();
    assert!(logger.is_ok(), "AuditLogger::new() should succeed");

    let logger = logger.unwrap();
    let scan_id = uuid::Uuid::new_v4();
    let config = ScanConfig {
        paths: vec![PathBuf::from("C:\\Temp")],
        include_hidden: false,
        follow_symlinks: false,
        max_file_size: None,
        exclude_patterns: vec![],
        include_patterns: vec![],
        parallel_jobs: None,
        min_age_days: None,
    };
    let result = logger.log_scan_start(scan_id, vec![PathBuf::from("C:\\Temp")], &config);
    assert!(result.is_ok(), "log_scan_start should succeed");

    let result = logger.log_scan_complete(scan_id, 5, 1024, 100);
    assert!(result.is_ok(), "log_scan_complete should succeed");
}

// ── WindowsEditionDetector ───────────────────────────────────────────────────

/// Test Windows edition detection
#[test]
fn test_windows_edition_detection() {
    let detector = WindowsEditionDetector::new();
    assert!(
        detector.is_ok(),
        "WindowsEditionDetector::new() should succeed on Windows"
    );

    let detector = detector.unwrap();
    let version = detector.version();
    assert!(!version.is_empty(), "version string should not be empty");

    let build = detector.build_number();
    assert!(build > 0, "build number should be positive");

    let features = detector.features();
    // has_diskpart is available on all Windows editions
    let _ = features.has_diskpart;
}

// ── WslDetector ──────────────────────────────────────────────────────────────

/// Test WSL detection
#[test]
fn test_wsl_detection() {
    let result = WslDetector::new();
    // WslDetector::new() may fail if registry access fails; treat both as valid outcomes
    match result {
        Ok(detector) => {
            let _has_wsl = detector.has_wsl();
            let _dists = detector.distributions();
        }
        Err(_) => {
            println!("WSL detection failed (possibly no WSL installed), skipping assertions");
        }
    }
}

// ── DockerClient ─────────────────────────────────────────────────────────────

/// Test Docker client (skips if Docker is not available)
#[tokio::test]
async fn test_docker_client() {
    let client = match DockerClient::new().await {
        Ok(c) => c,
        Err(_) => {
            println!("Docker not available, skipping test");
            return;
        }
    };

    println!("Docker daemon running: {}", client.is_daemon_running());

    if client.is_daemon_running() {
        let containers = client.get_containers().await;
        assert!(
            containers.is_ok(),
            "get_containers should succeed when daemon is running"
        );

        let images = client.get_images().await;
        assert!(
            images.is_ok(),
            "get_images should succeed when daemon is running"
        );
    }
}

// ── PackageManagerRegistry ───────────────────────────────────────────────────

/// Test package manager registry
#[tokio::test]
async fn test_package_manager_registry() {
    let registry = PackageManagerRegistry::new().await;

    // Just verify the registry initialises without panic.
    // npm/pip/cargo presence depends on CI environment.
    let npm = registry.get_by_name("npm");
    if let Some(npm) = npm {
        let installed = npm.is_installed().await;
        println!("npm installed: {}", installed);
        if installed {
            let paths = npm.get_cache_paths().await;
            assert!(
                paths.is_ok(),
                "get_cache_paths should not error when installed"
            );
        }
    } else {
        println!("npm package manager not in registry");
    }
}

// ── Configuration ────────────────────────────────────────────────────────────

/// Test configuration defaults and serialisation
#[test]
fn test_configuration() {
    let config = Config::default();

    assert!(
        !config.scan_include_hidden,
        "include_hidden should default to false"
    );
    assert!(
        config.cleanup.use_recycle_bin,
        "use_recycle_bin should default to true"
    );
    assert_eq!(config.ui.theme, "system", "default theme should be system");

    let toml_str = toml::to_string_pretty(&config).unwrap();
    let parsed: Config = toml::from_str(&toml_str).unwrap();

    assert_eq!(config.scan_include_hidden, parsed.scan_include_hidden);
    assert_eq!(
        config.cleanup.use_recycle_bin,
        parsed.cleanup.use_recycle_bin
    );
}

// ── NeverDelete ──────────────────────────────────────────────────────────────

/// Test NEVER_DELETE protection list
#[test]
fn test_never_delete_list() {
    assert!(
        should_never_delete(&PathBuf::from(r"C:\Windows")),
        "Windows dir must be protected"
    );
    assert!(
        should_never_delete(&PathBuf::from(r"C:\Windows\System32")),
        "System32 must be protected"
    );
    assert!(
        should_never_delete(&PathBuf::from(r"C:\Program Files")),
        "Program Files must be protected"
    );

    // Temp files should not be protected
    assert!(
        !should_never_delete(&PathBuf::from(r"C:\Temp\my_temp_file.txt")),
        "temp files should not be protected"
    );
}

// ── JunctionDetector ─────────────────────────────────────────────────────────

/// Test junction detection on a regular directory
#[test]
fn test_junction_detection() {
    let detector = JunctionDetector::new();

    let temp_dir = TempDir::new().unwrap();
    let is_junction = detector.is_junction(temp_dir.path());
    assert!(
        is_junction.is_ok(),
        "is_junction should not error on a regular directory"
    );
    assert!(!is_junction.unwrap(), "a temp dir should not be a junction");

    let is_symlink = detector.is_symlink(temp_dir.path());
    assert!(
        is_symlink.is_ok(),
        "is_symlink should not error on a regular directory"
    );
    assert!(!is_symlink.unwrap(), "a temp dir should not be a symlink");
}
