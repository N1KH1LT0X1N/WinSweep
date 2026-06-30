//! Windows Service Manager
//!
//! This module provides functionality to manage Windows services,
//! including starting, stopping, and querying service status.

use crate::home_edition_compat::HomeEditionCompat;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Security::SC_HANDLE;
use windows::Win32::System::Services::{
    ChangeServiceConfigW, CloseServiceHandle, ControlService, DeleteService, EnumServicesStatusExW,
    OpenSCManagerW, OpenServiceW, QueryServiceConfigW, QueryServiceStatusEx, StartServiceW,
    QUERY_SERVICE_CONFIGW, SC_ENUM_PROCESS_INFO, SC_MANAGER_CONNECT, SC_MANAGER_ENUMERATE_SERVICE,
    SC_STATUS_PROCESS_INFO, SERVICE_CHANGE_CONFIG, SERVICE_CONTROL_STOP, SERVICE_QUERY_CONFIG,
    SERVICE_QUERY_STATUS, SERVICE_START, SERVICE_STATE_ALL, SERVICE_STATUS,
    SERVICE_STATUS_CURRENT_STATE, SERVICE_STATUS_PROCESS, SERVICE_STOP, SERVICE_WIN32_OWN_PROCESS,
    SERVICE_WIN32_SHARE_PROCESS,
};

/// Service manager for Windows services
pub struct ServiceManager {
    sc_manager: SC_HANDLE,
    home_compat: HomeEditionCompat,
}

/// Service status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub service_type: u32,
    pub current_state: ServiceState,
    pub controls_accepted: u32,
    pub win32_exit_code: u32,
    pub service_specific_exit_code: u32,
    pub check_point: u32,
    pub wait_hint: u32,
    pub process_id: Option<u32>,
    pub service_flags: u32,
}

/// Service state enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceState {
    Stopped,
    StartPending,
    StopPending,
    Running,
    ContinuePending,
    PausePending,
    Paused,
    Unknown,
}

/// Service information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub name: String,
    pub display_name: String,
    pub status: ServiceStatus,
    pub start_type: ServiceStartType,
    pub can_stop: bool,
    pub can_start: bool,
    pub can_delete: bool,
}

/// Service start type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceStartType {
    Boot,
    System,
    Automatic,
    Manual,
    Disabled,
    Unknown,
}

impl ServiceStartType {
    /// Map a raw Win32 `SERVICE_START_TYPE` value to this enum.
    fn from_raw(value: u32) -> Self {
        match value {
            0 => ServiceStartType::Boot,      // SERVICE_BOOT_START
            1 => ServiceStartType::System,    // SERVICE_SYSTEM_START
            2 => ServiceStartType::Automatic, // SERVICE_AUTO_START
            3 => ServiceStartType::Manual,    // SERVICE_DEMAND_START
            4 => ServiceStartType::Disabled,  // SERVICE_DISABLED
            _ => ServiceStartType::Unknown,
        }
    }
}

impl ServiceManager {
    /// Create a new service manager
    pub fn new() -> Result<Self> {
        let sc_manager = unsafe {
            OpenSCManagerW(
                PCWSTR::null(),
                PCWSTR::null(),
                SC_MANAGER_CONNECT | SC_MANAGER_ENUMERATE_SERVICE,
            )
        };

        match sc_manager {
            Ok(handle) => {
                let home_compat =
                    HomeEditionCompat::new().context("Failed to create HomeEditionCompat")?;

                Ok(Self {
                    sc_manager: handle,
                    home_compat,
                })
            }
            Err(e) => Err(anyhow::anyhow!(
                "Failed to open Service Control Manager: {}",
                e
            )),
        }
    }

    /// Get all services
    pub fn get_all_services(&self) -> Result<Vec<ServiceInfo>> {
        let mut services = Vec::new();

        // First, get the required buffer size
        let mut bytes_needed = 0u32;
        let mut services_returned = 0u32;
        let mut resume_handle = 0u32;

        unsafe {
            let result = EnumServicesStatusExW(
                self.sc_manager,
                SC_ENUM_PROCESS_INFO,
                SERVICE_WIN32_OWN_PROCESS | SERVICE_WIN32_SHARE_PROCESS,
                SERVICE_STATE_ALL,
                None,
                &mut bytes_needed,
                &mut services_returned,
                Some(std::ptr::addr_of_mut!(resume_handle)),
                None,
            );

            if result.is_err() && bytes_needed == 0 {
                return Err(anyhow::anyhow!("Failed to enumerate services"));
            }
        }

        // Allocate buffer
        let mut buffer = vec![0u8; bytes_needed as usize];

        // Enumerate services
        unsafe {
            let result = EnumServicesStatusExW(
                self.sc_manager,
                SC_ENUM_PROCESS_INFO,
                SERVICE_WIN32_OWN_PROCESS | SERVICE_WIN32_SHARE_PROCESS,
                SERVICE_STATE_ALL,
                Some(&mut buffer[..]),
                &mut bytes_needed,
                &mut services_returned,
                Some(std::ptr::addr_of_mut!(resume_handle)),
                None,
            );

            if result.is_err() {
                return Err(anyhow::anyhow!("Failed to enumerate services"));
            }
        }

        // Parse the buffer
        let mut current = buffer.as_ptr() as *const ENUM_SERVICE_STATUS_PROCESS;

        for _ in 0..services_returned {
            let service_info = unsafe { self.parse_service_info(current)? };
            services.push(service_info);

            // Move to next entry
            unsafe {
                current = current.add(1);
            }
        }

        Ok(services)
    }

    /// Get a specific service by name
    pub fn get_service(&self, service_name: &str) -> Result<ServiceInfo> {
        let service_name_wide = to_wide(service_name);

        let service_handle = unsafe {
            OpenServiceW(
                self.sc_manager,
                PCWSTR(service_name_wide.as_ptr()),
                SERVICE_QUERY_STATUS,
            )
        };

        match service_handle {
            Ok(handle) => {
                let info = unsafe { self.get_service_info(handle, service_name)? };
                let _ = unsafe { CloseServiceHandle(handle) };

                Ok(info)
            }
            Err(e) => Err(anyhow::anyhow!(
                "Failed to open service {}: {}",
                service_name,
                e
            )),
        }
    }

    /// Start a service
    pub fn start_service(&self, service_name: &str) -> Result<()> {
        info!("Starting service: {}", service_name);

        let service_name_wide = to_wide(service_name);

        let service_handle = unsafe {
            OpenServiceW(
                self.sc_manager,
                PCWSTR(service_name_wide.as_ptr()),
                SERVICE_START,
            )
        };

        match service_handle {
            Ok(handle) => {
                let result = unsafe { StartServiceW(handle, None) };

                let _ = unsafe { CloseServiceHandle(handle) };

                if let Err(e) = result {
                    return Err(anyhow::anyhow!(
                        "Failed to start service {}: {}",
                        service_name,
                        e
                    ));
                }

                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!(
                "Failed to open service {}: {}",
                service_name,
                e
            )),
        }
    }

    /// Stop a service
    pub fn stop_service(&self, service_name: &str) -> Result<()> {
        info!("Stopping service: {}", service_name);

        let service_name_wide = to_wide(service_name);

        let service_handle = unsafe {
            OpenServiceW(
                self.sc_manager,
                PCWSTR(service_name_wide.as_ptr()),
                SERVICE_STOP,
            )
        };

        match service_handle {
            Ok(handle) => {
                let mut status = SERVICE_STATUS::default();
                let result = unsafe { ControlService(handle, SERVICE_CONTROL_STOP, &mut status) };

                let _ = unsafe { CloseServiceHandle(handle) };

                if let Err(e) = result {
                    return Err(anyhow::anyhow!(
                        "Failed to stop service {}: {}",
                        service_name,
                        e
                    ));
                }

                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!(
                "Failed to open service {}: {}",
                service_name,
                e
            )),
        }
    }

    /// Restart a service
    pub fn restart_service(&self, service_name: &str) -> Result<()> {
        info!("Restarting service: {}", service_name);

        // First stop the service
        self.stop_service(service_name)?;

        // Wait for it to stop
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Then start it
        self.start_service(service_name)?;

        Ok(())
    }

    /// Check if a service exists
    pub fn service_exists(&self, service_name: &str) -> bool {
        let service_name_wide = to_wide(service_name);

        let service_handle = unsafe {
            OpenServiceW(
                self.sc_manager,
                PCWSTR(service_name_wide.as_ptr()),
                SERVICE_QUERY_STATUS,
            )
        };

        match service_handle {
            Ok(handle) => {
                let _ = unsafe { CloseServiceHandle(handle) };
                true
            }
            Err(_) => false,
        }
    }

    /// Parse service information from ENUM_SERVICE_STATUS_PROCESS
    unsafe fn parse_service_info(
        &self,
        entry: *const ENUM_SERVICE_STATUS_PROCESS,
    ) -> Result<ServiceInfo> {
        let name = from_wide((*entry).lpServiceName);
        let display_name = from_wide((*entry).lpDisplayName);

        let status = ServiceStatus {
            service_type: (*entry).ServiceStatusProcess.dwServiceType.0,
            current_state: ServiceState::from((*entry).ServiceStatusProcess.dwCurrentState),
            controls_accepted: (*entry).ServiceStatusProcess.dwControlsAccepted,
            win32_exit_code: (*entry).ServiceStatusProcess.dwWin32ExitCode,
            service_specific_exit_code: (*entry).ServiceStatusProcess.dwServiceSpecificExitCode,
            check_point: (*entry).ServiceStatusProcess.dwCheckPoint,
            wait_hint: (*entry).ServiceStatusProcess.dwWaitHint,
            process_id: if (*entry).ServiceStatusProcess.dwProcessId != 0 {
                Some((*entry).ServiceStatusProcess.dwProcessId)
            } else {
                None
            },
            service_flags: (*entry).ServiceStatusProcess.dwServiceFlags.0,
        };

        // Determine start type via a dedicated config query.
        let start_type = self.query_start_type(&name);

        // Determine capabilities
        let can_stop = status.controls_accepted & SERVICE_ACCEPT_STOP != 0;
        let can_start = true; // Simplified
        let can_delete = self.is_safe_to_disable(&name);

        Ok(ServiceInfo {
            name,
            display_name,
            status,
            start_type,
            can_stop,
            can_start,
            can_delete,
        })
    }

    /// Get service information from service handle
    unsafe fn get_service_info(
        &self,
        service_handle: SC_HANDLE,
        service_name: &str,
    ) -> Result<ServiceInfo> {
        let mut buffer = [0u8; 36]; // Size of SERVICE_STATUS_PROCESS
        let mut bytes_needed = 0u32;

        let result = QueryServiceStatusEx(
            service_handle,
            SC_STATUS_PROCESS_INFO,
            Some(&mut buffer[..]),
            &mut bytes_needed,
        );

        if result.is_err() {
            return Err(anyhow::anyhow!("Failed to query service status"));
        }

        let status_process = &*(buffer.as_ptr() as *const SERVICE_STATUS_PROCESS);

        let status = ServiceStatus {
            service_type: status_process.dwServiceType.0,
            current_state: ServiceState::from(status_process.dwCurrentState),
            controls_accepted: status_process.dwControlsAccepted,
            win32_exit_code: status_process.dwWin32ExitCode,
            service_specific_exit_code: status_process.dwServiceSpecificExitCode,
            check_point: status_process.dwCheckPoint,
            wait_hint: status_process.dwWaitHint,
            process_id: if status_process.dwProcessId != 0 {
                Some(status_process.dwProcessId)
            } else {
                None
            },
            service_flags: status_process.dwServiceFlags.0,
        };

        let can_stop = status.controls_accepted & SERVICE_ACCEPT_STOP != 0;

        Ok(ServiceInfo {
            name: service_name.to_string(),
            display_name: service_name.to_string(), // Simplified
            status,
            start_type: self.query_start_type(service_name),
            can_stop,
            can_start: true,
            can_delete: self.is_safe_to_disable(service_name),
        })
    }
}

impl Drop for ServiceManager {
    fn drop(&mut self) {
        let _ = unsafe { CloseServiceHandle(self.sc_manager) };
    }
}

impl From<u32> for ServiceState {
    fn from(state: u32) -> Self {
        match state {
            1 => ServiceState::Stopped,
            2 => ServiceState::StartPending,
            3 => ServiceState::StopPending,
            4 => ServiceState::Running,
            5 => ServiceState::ContinuePending,
            6 => ServiceState::PausePending,
            7 => ServiceState::Paused,
            _ => ServiceState::Unknown,
        }
    }
}

impl From<SERVICE_STATUS_CURRENT_STATE> for ServiceState {
    fn from(state: SERVICE_STATUS_CURRENT_STATE) -> Self {
        ServiceState::from(state.0)
    }
}

// Windows API constants
const SERVICE_ACCEPT_STOP: u32 = 0x00000001;
/// Standard `DELETE` access right (winnt.h) — required by `DeleteService`.
const DELETE_ACCESS: u32 = 0x0001_0000;

// Windows API structures
#[repr(C)]
#[allow(non_snake_case)]
struct ENUM_SERVICE_STATUS_PROCESS {
    lpServiceName: PWSTR,
    lpDisplayName: PWSTR,
    ServiceStatusProcess: SERVICE_STATUS_PROCESS,
}

/// Convert Rust string to wide string
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Convert wide string to Rust string
fn from_wide(ptr: PWSTR) -> String {
    unsafe {
        if ptr.0.is_null() {
            return String::new();
        }

        let mut len = 0;
        let mut current = ptr.0;
        while *current != 0 {
            len += 1;
            current = current.add(1);
        }

        let slice = std::slice::from_raw_parts(ptr.0, len);
        String::from_utf16_lossy(slice)
    }
}

impl ServiceManager {
    /// Check if a service is safe to disable
    pub fn is_safe_to_disable(&self, service_name: &str) -> bool {
        // Critical system services that should never be disabled
        let critical_services = [
            "Winmgmt",           // WMI
            "RpcSs",             // RPC
            "EventLog",          // Event Log
            "PlugPlay",          // Plug and Play
            "Power",             // Power management
            "ProfSvc",           // User Profile Service
            "DcomLaunch",        // DCOM
            "RpcEptMapper",      // RPC Endpoint Mapper
            "SamSs",             // Security Accounts Manager
            "LSM",               // Local Session Manager
            "WinDefend",         // Windows Defender
            "Mpssvc",            // Windows Firewall
            "BFE",               // Base Filtering Engine
            "PolicyAgent",       // IPsec Policy Agent
            "cryptsvc",          // Cryptographic Services
            "Dnscache",          // DNS Client
            "LanmanServer",      // Server
            "LanmanWorkstation", // Workstation
            "Netlogon",          // Netlogon
            "MpsSvc",            // Windows Firewall
        ];

        !critical_services.contains(&service_name)
    }

    /// Get list of services that are safe to disable for cleanup
    pub fn get_cleanup_safe_services(&self) -> Vec<&'static str> {
        vec![
            "wuauserv",  // Windows Update
            "bits",      // Background Intelligent Transfer Service
            "dosvc",     // Delivery Optimization
            "uspso",     // Update Session Orchestrator
            "srservice", // System Restore Service
            "vss",       // Volume Shadow Copy
            "wbengine",  // Windows Backup Service
            "sdvsvc",    // Shell Hardware Detection
            "Themes",    // Themes (visual)
            "AudioSrv",  // Windows Audio
            "stisvc",    // Windows Image Acquisition (WIA)
            "WSearch",   // Windows Search
            "SysMain",   // Superfetch/Prefetch
        ]
    }

    /// Safely stop a service with validation
    pub fn stop_service_safe(&self, service_name: &str) -> Result<bool> {
        // Check if service is safe to stop
        if !self.is_safe_to_disable(service_name) {
            warn!("Refusing to stop critical service: {}", service_name);
            return Ok(false);
        }

        // Check Home edition limitations
        if self.home_compat.is_home_edition() {
            debug!("Running on Home edition, checking service compatibility");
            // Some services might have different behavior on Home edition
        }

        // Get current service info
        let service_info = self.get_service(service_name)?;

        // Check if service can be stopped
        if !service_info.can_stop {
            warn!("Service {} cannot be stopped", service_name);
            return Ok(false);
        }

        // Check if service is already stopped
        if service_info.status.current_state == ServiceState::Stopped {
            debug!("Service {} is already stopped", service_name);
            return Ok(true);
        }

        // Attempt to stop the service
        match self.stop_service(service_name) {
            Ok(_) => {
                info!("Successfully stopped service: {}", service_name);
                Ok(true)
            }
            Err(e) => {
                error!("Failed to stop service {}: {}", service_name, e);
                Ok(false)
            }
        }
    }

    /// Safely disable a service with validation
    pub fn disable_service_safe(&self, service_name: &str) -> Result<bool> {
        // Check if service is safe to disable
        if !self.is_safe_to_disable(service_name) {
            warn!("Refusing to disable critical service: {}", service_name);
            return Ok(false);
        }

        // First stop the service if it's running
        if let Ok(service_info) = self.get_service(service_name) {
            if service_info.status.current_state != ServiceState::Stopped {
                self.stop_service_safe(service_name)?;
            }
        }

        // Change start type to disabled
        match self.change_service_start_type(service_name, ServiceStartType::Disabled) {
            Ok(_) => {
                info!("Successfully disabled service: {}", service_name);
                Ok(true)
            }
            Err(e) => {
                error!("Failed to disable service {}: {}", service_name, e);
                Ok(false)
            }
        }
    }

    /// Get service state as string
    pub fn state_to_string(state: ServiceState) -> &'static str {
        match state {
            ServiceState::Stopped => "Stopped",
            ServiceState::StartPending => "Starting",
            ServiceState::StopPending => "Stopping",
            ServiceState::Running => "Running",
            ServiceState::ContinuePending => "Resuming",
            ServiceState::PausePending => "Pausing",
            ServiceState::Paused => "Paused",
            ServiceState::Unknown => "Unknown",
        }
    }

    /// Change the start type of a service
    pub fn change_service_start_type(
        &self,
        service_name: &str,
        start_type: ServiceStartType,
    ) -> Result<()> {
        info!(
            "Changing start type of service {} to {:?}",
            service_name, start_type
        );

        let service_name_wide = to_wide(service_name);

        let service_handle = unsafe {
            OpenServiceW(
                self.sc_manager,
                PCWSTR(service_name_wide.as_ptr()),
                SERVICE_CHANGE_CONFIG,
            )
        };

        match service_handle {
            Ok(handle) => {
                let start_type_raw = match start_type {
                    ServiceStartType::Boot => 0u32,      // SERVICE_BOOT_START
                    ServiceStartType::System => 1u32,    // SERVICE_SYSTEM_START
                    ServiceStartType::Automatic => 2u32, // SERVICE_AUTO_START
                    ServiceStartType::Manual => 3u32,    // SERVICE_DEMAND_START
                    ServiceStartType::Disabled => 4u32,  // SERVICE_DISABLED
                    ServiceStartType::Unknown => {
                        let _ = unsafe { CloseServiceHandle(handle) };
                        return Err(anyhow::anyhow!("Cannot change start type to Unknown"));
                    }
                };

                let result = unsafe {
                    ChangeServiceConfigW(
                        handle,
                        windows::Win32::System::Services::ENUM_SERVICE_TYPE(u32::MAX),
                        windows::Win32::System::Services::SERVICE_START_TYPE(start_type_raw),
                        windows::Win32::System::Services::SERVICE_ERROR(u32::MAX),
                        PCWSTR::null(),
                        PCWSTR::null(),
                        None,
                        PCWSTR::null(),
                        PCWSTR::null(),
                        PCWSTR::null(),
                        PCWSTR::null(),
                    )
                };

                let _ = unsafe { CloseServiceHandle(handle) };

                if let Err(e) = result {
                    return Err(anyhow::anyhow!(
                        "Failed to change start type for {}: {}",
                        service_name,
                        e
                    ));
                }

                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!(
                "Failed to open service {}: {}",
                service_name,
                e
            )),
        }
    }

    /// Query the configured start type of a service via `QueryServiceConfigW`.
    ///
    /// Returns [`ServiceStartType::Unknown`] when the service cannot be opened
    /// (e.g. insufficient privileges) or the query fails.
    pub fn query_start_type(&self, service_name: &str) -> ServiceStartType {
        let service_name_wide = to_wide(service_name);

        let handle = match unsafe {
            OpenServiceW(
                self.sc_manager,
                PCWSTR(service_name_wide.as_ptr()),
                SERVICE_QUERY_CONFIG,
            )
        } {
            Ok(h) => h,
            Err(_) => return ServiceStartType::Unknown,
        };

        // First call determines the required buffer size.
        let mut bytes_needed: u32 = 0;
        let _ = unsafe { QueryServiceConfigW(handle, None, 0, &mut bytes_needed) };
        if bytes_needed == 0 {
            let _ = unsafe { CloseServiceHandle(handle) };
            return ServiceStartType::Unknown;
        }

        let mut buffer = vec![0u8; bytes_needed as usize];
        let cfg_ptr = buffer.as_mut_ptr() as *mut QUERY_SERVICE_CONFIGW;
        let result =
            unsafe { QueryServiceConfigW(handle, Some(cfg_ptr), bytes_needed, &mut bytes_needed) };

        let start_type = if result.is_ok() {
            let raw = unsafe { (*cfg_ptr).dwStartType.0 };
            ServiceStartType::from_raw(raw)
        } else {
            ServiceStartType::Unknown
        };

        let _ = unsafe { CloseServiceHandle(handle) };
        start_type
    }

    /// Permanently delete a service from the Service Control Manager database.
    ///
    /// Refuses to act on services classified as critical by
    /// [`Self::is_safe_to_disable`]. The deletion takes effect once all open
    /// handles to the service are closed.
    pub fn delete_service(&self, service_name: &str) -> Result<()> {
        if !self.is_safe_to_disable(service_name) {
            return Err(anyhow::anyhow!(
                "Refusing to delete critical service '{}'",
                service_name
            ));
        }

        info!("Deleting service {}", service_name);
        let service_name_wide = to_wide(service_name);

        let handle = unsafe {
            OpenServiceW(
                self.sc_manager,
                PCWSTR(service_name_wide.as_ptr()),
                DELETE_ACCESS,
            )
        }
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to open service '{}' for delete: {}",
                service_name,
                e
            )
        })?;

        let result = unsafe { DeleteService(handle) };
        let _ = unsafe { CloseServiceHandle(handle) };

        result
            .map_err(|e| anyhow::anyhow!("Failed to delete service '{}': {}", service_name, e))?;
        Ok(())
    }

    /// Re-enable a previously disabled service by restoring automatic start.
    ///
    /// For finer-grained control over the resulting start type, call
    /// [`Self::change_service_start_type`] directly.
    pub fn re_enable_service(&self, service_name: &str) -> Result<()> {
        info!("Re-enabling service {}", service_name);
        self.change_service_start_type(service_name, ServiceStartType::Automatic)
    }

    /// Get start type as string
    pub fn start_type_to_string(start_type: ServiceStartType) -> &'static str {
        match start_type {
            ServiceStartType::Boot => "Boot",
            ServiceStartType::System => "System",
            ServiceStartType::Automatic => "Automatic",
            ServiceStartType::Manual => "Manual",
            ServiceStartType::Disabled => "Disabled",
            ServiceStartType::Unknown => "Unknown",
        }
    }

    /// Check if running on Home edition
    pub fn is_home_edition(&self) -> bool {
        self.home_compat.is_home_edition()
    }

    /// Get Home edition limitations for service management
    pub fn get_home_edition_limitations(&self) -> Vec<String> {
        if self.home_compat.is_home_edition() {
            vec![
                "Limited service management capabilities".to_string(),
                "Some system services may be protected".to_string(),
                "Group Policy-based service configurations not available".to_string(),
            ]
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_manager_creation() {
        // This test requires running on Windows
        #[cfg(windows)]
        {
            let manager = ServiceManager::new();
            assert!(manager.is_ok());
        }
    }

    #[test]
    fn test_service_state_conversion() {
        assert_eq!(ServiceState::from(1), ServiceState::Stopped);
        assert_eq!(ServiceState::from(4), ServiceState::Running);
        assert_eq!(ServiceState::from(99), ServiceState::Unknown);
    }

    #[test]
    fn test_start_type_from_raw() {
        assert_eq!(ServiceStartType::from_raw(0), ServiceStartType::Boot);
        assert_eq!(ServiceStartType::from_raw(1), ServiceStartType::System);
        assert_eq!(ServiceStartType::from_raw(2), ServiceStartType::Automatic);
        assert_eq!(ServiceStartType::from_raw(3), ServiceStartType::Manual);
        assert_eq!(ServiceStartType::from_raw(4), ServiceStartType::Disabled);
        assert_eq!(ServiceStartType::from_raw(99), ServiceStartType::Unknown);
    }
}
