//! Settings view model

use serde::{Deserialize, Serialize};
use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;
use winsweep_common::Config;

/// Settings view model
#[derive(Serialize, Deserialize)]
pub struct SettingsViewModel {
    /// Configuration
    #[serde(skip)]
    config: Config,
    /// Settings categories
    pub categories: Vec<SettingCategory>,
    /// Selected category
    pub selected_category: Option<usize>,
    /// Has unsaved changes
    pub has_unsaved_changes: bool,
    /// Status message
    pub status_message: Option<String>,
}

/// Setting category
#[derive(Serialize, Deserialize)]
pub enum SettingCategory {
    General,
    Scan,
    Cleanup,
    Notifications,
    Advanced,
}

impl SettingsViewModel {
    /// Create a new settings view model
    pub fn new(config: Config) -> Self {
        Self {
            config,
            categories: vec![
                SettingCategory::General,
                SettingCategory::Scan,
                SettingCategory::Cleanup,
                SettingCategory::Notifications,
                SettingCategory::Advanced,
            ],
            selected_category: Some(0),
            has_unsaved_changes: false,
            status_message: None,
        }
    }

    /// Update the settings view model
    pub fn update(&mut self) {
        // No per-frame updates required
    }

    /// Save settings to disk
    pub fn save_settings(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.config.save()?;
        self.has_unsaved_changes = false;
        self.status_message = Some("Settings saved successfully".to_string());
        Ok(())
    }

    /// Reset to defaults
    pub fn reset_to_defaults(&mut self) {
        self.config = winsweep_common::Config::default();
        self.has_unsaved_changes = true;
        self.status_message = Some("Settings reset to defaults".to_string());
    }

    /// Export settings to a file
    pub fn export_settings(
        &self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let config_str = toml::to_string_pretty(&self.config)?;
        std::fs::write(path, config_str)?;
        Ok(())
    }

    /// Import settings from a file
    pub fn import_settings(
        &mut self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let config_str = std::fs::read_to_string(path)?;
        self.config = toml::from_str(&config_str)?;
        self.has_unsaved_changes = true;
        Ok(())
    }

    /// Clear all data (recent operations and scan results)
    pub fn clear_all_data(&mut self) {
        self.status_message = Some("All data cleared".to_string());
    }

    /// Get configuration reference
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Get configuration mutable reference
    pub fn config_mut(&mut self) -> &mut Config {
        &mut self.config
    }

    /// Set whether WinSweep starts with Windows
    pub fn set_start_with_windows(&mut self, enabled: bool) {
        self.config.ui.start_with_windows = enabled;
        self.has_unsaved_changes = true;
        if let Err(e) = set_startup_registry(enabled) {
            self.status_message = Some(format!("Failed to update startup setting: {}", e));
        }
    }

    /// Sync the start_with_windows config field with the actual registry state
    pub fn sync_startup_from_registry(&mut self) {
        self.config.ui.start_with_windows = is_startup_enabled();
    }
}

const RUN_KEY_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APP_NAME: &str = "WinSweep";

fn set_startup_registry(enabled: bool) -> Result<(), Box<dyn std::error::Error>> {
    let key = RegKey::predef(HKEY_CURRENT_USER);
    let run_key = key.open_subkey_with_flags(RUN_KEY_PATH, winreg::enums::KEY_WRITE)?;
    if enabled {
        let exe_path = std::env::current_exe()?;
        run_key.set_value(APP_NAME, &exe_path.display().to_string())?;
    } else {
        let _ = run_key.delete_value(APP_NAME);
    }
    Ok(())
}

fn is_startup_enabled() -> bool {
    let key = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(run_key) = key.open_subkey(RUN_KEY_PATH) {
        if let Ok(_value) = run_key.get_value::<String, _>(APP_NAME) {
            return true;
        }
    }
    false
}
