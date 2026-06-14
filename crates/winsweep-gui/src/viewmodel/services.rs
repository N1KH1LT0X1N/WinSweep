//! Services view model

use serde::{Deserialize, Serialize};
use winsweep_core::service_manager::{
    ServiceInfo as CoreServiceInfo, ServiceStartType as CoreServiceStartType,
};
use winsweep_core::{ServiceManager, ServiceState, ServiceStatus};

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
#[derive(Clone, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Serialize, Deserialize)]
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
        // Service status is refreshed on demand via refresh_services
    }

    /// Refresh services from the ServiceManager
    pub fn refresh_services(
        &mut self,
        service_manager: &ServiceManager,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.services.clear();

        let core_services = service_manager
            .get_all_services()
            .map_err(|e| format!("Failed to enumerate services: {}", e))?;

        for core in core_services {
            self.services.push(map_core_service(&core));
        }

        self.status_message = Some(format!(
            "Services refreshed ({} services)",
            self.services.len()
        ));
        Ok(())
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

                let matches_status = !self.show_running_only
                    || matches!(s.status.current_state, ServiceState::Running);

                matches_filter && matches_status
            })
            .collect()
    }
}

fn map_core_service(core: &CoreServiceInfo) -> ServiceInfo {
    ServiceInfo {
        name: core.name.clone(),
        display_name: core.display_name.clone(),
        status: core.status.clone(),
        start_type: map_core_start_type(core.start_type),
        description: String::new(),
        can_stop: core.can_stop,
        can_start: core.can_start,
    }
}

fn map_core_start_type(core: CoreServiceStartType) -> ServiceStartType {
    match core {
        CoreServiceStartType::Automatic => ServiceStartType::Automatic,
        CoreServiceStartType::Manual => ServiceStartType::Manual,
        CoreServiceStartType::Disabled => ServiceStartType::Disabled,
        CoreServiceStartType::Boot => ServiceStartType::Boot,
        CoreServiceStartType::System => ServiceStartType::System,
        CoreServiceStartType::Unknown => ServiceStartType::Manual,
    }
}
