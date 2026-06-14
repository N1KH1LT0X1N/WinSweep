//! Configuration management for WinSweep
//!
//! This module handles loading, saving, and validating configuration.

use crate::types::ScanConfig;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Main WinSweep configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Scan configuration
    pub scan: ScanConfig,
    /// Cleanup settings
    pub cleanup: CleanupConfig,
    /// UI settings
    pub ui: UiConfig,
    /// Logging settings
    pub logging: LoggingConfig,
    /// Telemetry settings
    pub telemetry: TelemetryConfig,
    /// Scan include hidden files
    pub scan_include_hidden: bool,
    /// Scan include system files
    pub scan_include_system: bool,
    /// Scan minimum size
    pub scan_min_size: u64,
    /// Notify cleanup complete
    pub notify_cleanup_complete: bool,
    /// Notify low disk space
    pub notify_low_disk_space: bool,
    /// Low disk space threshold
    pub low_disk_threshold: u8,
    /// Notification duration
    pub notification_duration: u32,
    /// Auto cleanup enabled
    pub auto_cleanup_enabled: bool,
    /// Auto cleanup days
    pub auto_cleanup_days: u32,
    /// Max concurrent operations
    pub max_concurrent_ops: u8,
}

/// Cleanup configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupConfig {
    /// Whether to move to recycle bin instead of permanent deletion
    pub use_recycle_bin: bool,
    /// Whether to require confirmation before deletion
    pub require_confirmation: bool,
    /// Maximum age of files to consider for cleanup (in days)
    pub max_file_age_days: Option<u32>,
    /// Minimum file size to consider (in bytes)
    pub min_file_size: Option<u64>,
    /// Whether to create a restore point before cleanup
    pub create_restore_point: bool,
    /// Confirm before deleting files
    pub cleanup_confirm_delete: bool,
    /// Move files to recycle bin instead of permanent deletion
    pub cleanup_move_to_recycle: bool,
    /// Clean temporary files
    pub clean_temp_files: bool,
    /// Clean recycle bin
    pub clean_recycle_bin: bool,
    /// Clean prefetch files
    pub clean_prefetch: bool,
    /// Clean browser cache
    pub clean_browser_cache: bool,
}

/// UI configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// Theme (light/dark/system)
    pub theme: String,
    /// Language
    pub language: String,
    /// Whether to show hidden files by default
    pub show_hidden_files: bool,
    /// Whether to animate the UI
    pub enable_animations: bool,
    /// Start with Windows
    pub start_with_windows: bool,
    /// Minimize to system tray
    pub minimize_to_tray: bool,
    /// Show notifications
    pub show_notifications: bool,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: String,
    /// Whether to log to file
    pub log_to_file: bool,
    /// Maximum log file size in MB
    pub max_log_size_mb: u32,
    /// Maximum number of log files to keep
    pub max_log_files: u32,
    /// Enable debug mode
    pub debug_mode: bool,
    /// Enable verbose logging
    pub verbose_logging: bool,
}

/// Telemetry configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TelemetryConfig {
    /// Whether the user has opted in to crash reporting and telemetry
    pub opt_in: bool,
    /// DSN or endpoint for telemetry (empty = default)
    pub endpoint: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            scan: ScanConfig::default(),
            cleanup: CleanupConfig::default(),
            ui: UiConfig::default(),
            logging: LoggingConfig::default(),
            telemetry: TelemetryConfig::default(),
            scan_include_hidden: false,
            scan_include_system: false,
            scan_min_size: 1024, // 1KB
            notify_cleanup_complete: true,
            notify_low_disk_space: true,
            low_disk_threshold: 10,   // 10%
            notification_duration: 5, // 5 seconds
            auto_cleanup_enabled: false,
            auto_cleanup_days: 7, // weekly
            max_concurrent_ops: 4,
        }
    }
}

impl Default for CleanupConfig {
    fn default() -> Self {
        Self {
            use_recycle_bin: true,
            require_confirmation: true,
            max_file_age_days: None,
            min_file_size: None,
            create_restore_point: false,
            cleanup_confirm_delete: true,
            cleanup_move_to_recycle: true,
            clean_temp_files: true,
            clean_recycle_bin: false,
            clean_prefetch: true,
            clean_browser_cache: false,
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "system".to_string(),
            language: "en".to_string(),
            show_hidden_files: false,
            enable_animations: true,
            start_with_windows: false,
            minimize_to_tray: true,
            show_notifications: true,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            log_to_file: true,
            max_log_size_mb: 10,
            max_log_files: 5,
            debug_mode: false,
            verbose_logging: false,
        }
    }
}

impl Config {
    /// Load configuration from file
    pub fn load() -> Result<Self> {
        let config_path = get_config_path();

        if !config_path.exists() {
            // Create default config
            let default_config = Config::default();
            default_config.save()?;
            return Ok(default_config);
        }

        let config_str = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;

        let config: Config =
            toml::from_str(&config_str).with_context(|| "Failed to parse config file")?;

        Ok(config)
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<()> {
        let config_path = get_config_path();

        // Ensure config directory exists
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create config directory: {}", parent.display())
            })?;
        }

        let config_str =
            toml::to_string_pretty(self).with_context(|| "Failed to serialize config")?;

        std::fs::write(&config_path, config_str)
            .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;

        Ok(())
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Validate scan paths
        if self.scan.paths.is_empty() {
            anyhow::bail!("At least one scan path must be specified");
        }

        for path in &self.scan.paths {
            if !path.exists() {
                anyhow::bail!("Scan path does not exist: {}", path.display());
            }
        }

        // Validate parallel jobs
        if let Some(jobs) = self.scan.parallel_jobs {
            if jobs == 0 {
                anyhow::bail!("Parallel jobs must be greater than 0");
            }
            if jobs > 256 {
                anyhow::bail!("Parallel jobs should not exceed 256");
            }
        }

        // Validate max file size
        if let Some(size) = self.scan.max_file_size {
            if size == 0 {
                anyhow::bail!("Max file size must be greater than 0");
            }
        }

        Ok(())
    }
}

/// Get the configuration file path
fn get_config_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));

    path.push("WinSweep");
    path.push("config.toml");

    path
}

/// Get the log file path
pub fn get_log_path() -> PathBuf {
    let mut path = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));

    path.push("WinSweep");
    path.push("logs");

    // Ensure log directory exists
    std::fs::create_dir_all(&path).ok();

    path.push("winsweep.log");

    path
}

/// Get the cache directory path
pub fn get_cache_dir() -> PathBuf {
    let mut path = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));

    path.push("WinSweep");
    path.push("cache");

    path
}

/// Get the application data directory
pub fn get_data_dir() -> PathBuf {
    let mut path = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));

    path.push("WinSweep");

    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(config.scan.paths, parsed.scan.paths);
    }

    #[test]
    fn test_config_validation_empty_paths() {
        let mut config = Config::default();
        config.scan.paths.clear();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_telemetry_disabled_by_default() {
        let config = Config::default();
        assert!(!config.telemetry.opt_in);
    }

    #[test]
    fn test_telemetry_opt_in_roundtrip() {
        let mut config = Config::default();
        config.telemetry.opt_in = true;
        config.telemetry.endpoint = "https://example.com".to_string();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert!(parsed.telemetry.opt_in);
        assert_eq!(parsed.telemetry.endpoint, "https://example.com");
    }
}
