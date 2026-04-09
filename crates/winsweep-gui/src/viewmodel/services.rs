//! Services view model

use serde::{Deserialize, Serialize};
use winsweep_core::{ServiceManager, ServiceStatus};

/// Services view model
#[derive(Serialize, Deserialize)]
pub struct ServicesViewModel {
    /// Windows services
    pub services: Vec<ServiceInfo>,
    /// Selected service
    pub selected_service: Option<usize>,
    /// Filter text
    pub filter_text: String,
    /// Show only running services
    pub show_running_only: bool,
    /// Status message
    pub status_message: Option<String>,
}

/// Service information
#[derive(Serialize, Deserialize)]
pub struct ServiceInfo {
    pub name: String,
    pub display_name: String,
    pub status: ServiceStatus,
    pub start_type: ServiceStartType,
    pub description: String,
    pub can_stop: bool,
    pub can_start: bool,
}

/// Service start type
#[derive(Serialize, Deserialize)]
pub enum ServiceStartType {
    Automatic,
    Manual,
    Disabled,
    Boot,
    System,
}

impl ServicesViewModel {
    /// Create a new services view model
    pub fn new() -> Self {
        Self {
            services: Vec::new(),
            selected_service: None,
            filter_text: String::new(),
            show_running_only: false,
            status_message: None,
        }
    }

    /// Update the services view model
    pub fn update(&mut self) {
        // TODO: Update service status
    }

    /// Refresh services
    pub fn refresh_services(
        &mut self,
        service_manager: &ServiceManager,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.services.clear();

        // TODO: Get services from ServiceManager

        self.status_message = Some("Services refreshed".to_string());
        Ok(())
    }

    /// Start selected service
    pub fn start_selected(
        &mut self,
        service_manager: &ServiceManager,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(index) = self.selected_service {
            if index < self.services.len() {
                let service_name = &self.services[index].name;
                self.status_message = Some(format!("Starting {}...", service_name));

                // TODO: Start service

                self.status_message = Some(format!("{} started", service_name));
                return Ok(());
            }
        }

        Err("No service selected".into())
    }

    /// Stop selected service
    pub fn stop_selected(
        &mut self,
        service_manager: &ServiceManager,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(index) = self.selected_service {
            if index < self.services.len() {
                let service_name = &self.services[index].name;
                self.status_message = Some(format!("Stopping {}...", service_name));

                // TODO: Stop service

                self.status_message = Some(format!("{} stopped", service_name));
                return Ok(());
            }
        }

        Err("No service selected".into())
    }

    /// Restart selected service
    pub fn restart_selected(
        &mut self,
        service_manager: &ServiceManager,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(index) = self.selected_service {
            if index < self.services.len() {
                let service_name = &self.services[index].name;
                self.status_message = Some(format!("Restarting {}...", service_name));

                // TODO: Restart service

                self.status_message = Some(format!("{} restarted", service_name));
                return Ok(());
            }
        }

        Err("No service selected".into())
    }

    /// Get filtered services
    pub fn filtered_services(&self) -> Vec<&ServiceInfo> {
        self.services
            .iter()
            .filter(|s| {
                let matches_filter = self.filter_text.is_empty()
                    || s.name
                        .to_lowercase()
                        .contains(&self.filter_text.to_lowercase())
                    || s.display_name
                        .to_lowercase()
                        .contains(&self.filter_text.to_lowercase());

                let matches_status =
                    !self.show_running_only || matches!(s.status, ServiceStatus::Running);

                matches_filter && matches_status
            })
            .collect()
    }
}
