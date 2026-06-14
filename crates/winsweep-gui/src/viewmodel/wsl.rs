//! WSL view model

use serde::{Deserialize, Serialize};
use winsweep_core::{WslDetector, WslState, WslVersion};

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
        // No per-frame updates required
    }

    /// Refresh distributions
    pub fn refresh_distributions(&mut self, wsl_detector: &WslDetector) {
        self.distributions.clear();

        for name in wsl_detector.distributions().keys() {
            if let Some(dist) = wsl_detector.get_distribution(name) {
                let size_gb = if let Some(vhdx) = &dist.vhdx_path {
                    std::fs::metadata(vhdx)
                        .map(|m| m.len() as f32 / (1024.0 * 1024.0 * 1024.0))
                        .unwrap_or(0.0)
                } else {
                    0.0
                };
                self.distributions.push(WslDistInfo {
                    name: name.clone(),
                    version: dist.version,
                    state: dist.state,
                    size_gb,
                    path: dist
                        .wsl_path
                        .as_ref()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default(),
                });
            }
        }
    }

    /// Start compacting selected distribution (flags only; caller should spawn elevated task)
    pub fn start_compact(&mut self) {
        if let Some(index) = self.selected_distribution {
            if index < self.distributions.len() {
                self.compact_in_progress = true;
                self.compact_progress = 0.0;
                self.status_message =
                    Some(format!("Compacting {}...", self.distributions[index].name));
            }
        }
    }

    /// Stop a running distribution
    pub fn stop_distribution(&mut self, name: &str) {
        match std::process::Command::new("wsl")
            .args(["--terminate", name])
            .output()
        {
            Ok(o) if o.status.success() => {
                self.status_message = Some(format!("Stopped {}", name));
            }
            Ok(o) => {
                self.status_message = Some(format!(
                    "Failed to stop {}: {}",
                    name,
                    String::from_utf8_lossy(&o.stderr)
                ));
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to stop {}: {}", name, e));
            }
        }
    }

    /// Start a stopped distribution
    pub fn start_distribution(&mut self, name: &str) {
        match std::process::Command::new("wsl").args(["-d", name]).spawn() {
            Ok(_) => {
                self.status_message = Some(format!("Started {}", name));
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to start {}: {}", name, e));
            }
        }
    }

    /// Unregister a distribution
    pub fn unregister_distribution(&mut self, name: &str) {
        match std::process::Command::new("wsl")
            .args(["--unregister", name])
            .output()
        {
            Ok(o) if o.status.success() => {
                self.status_message = Some(format!("Unregistered {}", name));
                self.distributions.retain(|d| d.name != name);
                self.selected_distribution = None;
            }
            Ok(o) => {
                self.status_message = Some(format!(
                    "Failed to unregister {}: {}",
                    name,
                    String::from_utf8_lossy(&o.stderr)
                ));
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to unregister {}: {}", name, e));
            }
        }
    }

    /// Open distribution filesystem in Windows Explorer
    pub fn open_in_explorer(&mut self, name: &str) {
        let path = format!("\\\\wsl$\\{}", name);
        let _ = std::process::Command::new("explorer.exe")
            .arg(&path)
            .spawn();
    }
}
