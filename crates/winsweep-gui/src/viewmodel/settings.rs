//! Settings view model

use serde::{Deserialize, Serialize};
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
        // TODO: Update settings if needed
    }

    /// Save settings
    pub fn save_settings(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: Save configuration
        self.has_unsaved_changes = false;
        self.status_message = Some("Settings saved successfully".to_string());
        Ok(())
    }

    /// Reset to defaults
    pub fn reset_to_defaults(&mut self) {
        // TODO: Reset configuration to defaults
        self.has_unsaved_changes = true;
        self.status_message = Some("Settings reset to defaults".to_string());
    }

    /// Get configuration reference
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Get configuration mutable reference
    pub fn config_mut(&mut self) -> &mut Config {
        &mut self.config
    }

    /// Mark as having unsaved changes
    pub fn mark_dirty(&mut self) {
        self.has_unsaved_changes = true;
    }
}
