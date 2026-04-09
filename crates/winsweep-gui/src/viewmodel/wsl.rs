//! WSL view model

use serde::{Deserialize, Serialize};
use winsweep_core::{WslDetector, WslDistribution, WslVersion, WslState};

/// WSL view model
#[derive(Serialize, Deserialize)]
pub struct WslViewModel {
    /// WSL distributions
    pub distributions: Vec<WslDistInfo>,
    /// Selected distribution
    pub selected_distribution: Option<usize>,
    /// Compact operation in progress
    pub compact_in_progress: bool,
    /// Compact progress (0.0 to 1.0)
    pub compact_progress: f32,
    /// Status message
    pub status_message: Option<String>,
}

/// WSL distribution information
#[derive(Serialize, Deserialize)]
pub struct WslDistInfo {
    pub name: String,
    pub version: WslVersion,
    pub state: WslState,
    pub size_gb: f32,
    pub path: String,
}

impl WslViewModel {
    /// Create a new WSL view model
    pub fn new() -> Self {
        Self {
            distributions: Vec::new(),
            selected_distribution: None,
            compact_in_progress: false,
            compact_progress: 0.0,
            status_message: None,
        }
    }
    
    /// Update the WSL view model
    pub fn update(&mut self) {
        // TODO: Update distribution status
    }
    
    /// Refresh distributions
    pub fn refresh_distributions(&mut self, wsl_detector: &WslDetector) {
        self.distributions.clear();
        
        for (name, _) in wsl_detector.distributions() {
            if let Ok(dist) = wsl_detector.get_distribution(name) {
                self.distributions.push(WslDistInfo {
                    name: name.clone(),
                    version: dist.version,
                    state: dist.state,
                    size_gb: 0.0, // TODO: Calculate size
                    path: dist.path.to_string_lossy().to_string(),
                });
            }
        }
    }
    
    /// Start compacting selected distribution
    pub fn start_compact(&mut self) {
        if let Some(index) = self.selected_distribution {
            if index < self.distributions.len() {
                self.compact_in_progress = true;
                self.compact_progress = 0.0;
                self.status_message = Some(format!("Compacting {}...", self.distributions[index].name));
            }
        }
    }
    
    /// Stop compacting
    pub fn stop_compact(&mut self) {
        self.compact_in_progress = false;
        self.compact_progress = 0.0;
        self.status_message = Some("Compaction stopped".to_string());
    }
}
