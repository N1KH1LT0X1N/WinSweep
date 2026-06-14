//! Docker view model

use serde::{Deserialize, Serialize};
use winsweep_core::{ContainerInfo, ImageInfo, NetworkInfo, VolumeInfo};

/// Docker view model
#[derive(Serialize, Deserialize)]
pub struct DockerViewModel {
    /// Docker resources
    pub resources: DockerResources,
    /// Selected tab
    pub selected_tab: DockerTab,
    /// Operation in progress
    pub operation_in_progress: bool,
    /// Operation progress (0.0 to 1.0)
    pub operation_progress: f32,
    /// Status message
    pub status_message: Option<String>,
}

/// Docker resources
#[derive(Serialize, Deserialize)]
pub struct DockerResources {
    pub containers: Vec<ContainerInfo>,
    pub images: Vec<ImageInfo>,
    pub volumes: Vec<VolumeInfo>,
    pub networks: Vec<NetworkInfo>,
}

/// Docker tabs
#[derive(Serialize, Deserialize)]
pub enum DockerTab {
    Containers,
    Images,
    Volumes,
    Networks,
}

impl DockerViewModel {
    /// Create a new Docker view model
    pub fn new() -> Self {
        Self {
            resources: DockerResources {
                containers: Vec::new(),
                images: Vec::new(),
                volumes: Vec::new(),
                networks: Vec::new(),
            },
            selected_tab: DockerTab::Containers,
            operation_in_progress: false,
            operation_progress: 0.0,
            status_message: None,
        }
    }

    /// Update the Docker view model
    pub fn update(&mut self) {
        // No per-frame updates required
    }
}
