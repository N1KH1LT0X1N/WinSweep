//! Windows Service Manager
//!
//! This module provides functionality to manage Windows services,
//! including starting, stopping, and querying service status.

use crate::home_edition_compat::HomeEditionCompat;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use tracing::{debug, error, info, warn};
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_SERVICE_DOES_NOT_EXIST, HANDLE, INVALID_HANDLE_VALUE,
};
use windows::Win32::System::Services::{
    CloseServiceHandle, ControlService, DeleteService, EnumServicesStatusExW, OpenSCManagerW,
    OpenServiceW, QueryServiceStatusEx, StartServiceW, SC_ENUM_PROCESS, SC_ENUM_TYPE, SC_HANDLE,
    SC_MANAGER_ALL_ACCESS, SERVICE_ALL_ACCESS, SERVICE_CONTROL_STOP, SERVICE_ENUMERATE_PROCESS,
    SERVICE_QUERY_STATUS, SERVICE_START, SERVICE_STATE_ALL, SERVICE_STATE_TYPE, SERVICE_STATUS,
    SERVICE_STATUS_PROCESS, SERVICE_STOP, SERVICE_WIN32_OWN_PROCESS, SERVICE_WIN32_SHARE_PROCESS,
};

/// Service manager for Windows services
pub struct ServiceManager {
    sc_manager: HANDLE,
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

impl ServiceManager {
    /// Create a new service manager
    pub fn new() -> Result<Self> {
        let sc_manager =
            unsafe { OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_ALL_ACCESS) };

        match sc_manager {
            Ok(handle) => {
                if handle == INVALID_HANDLE_VALUE {
                    let error = GetLastError();
                    return Err(anyhow::anyhow!(
                        "Failed to open Service Control Manager: error {}",
                        error.0
                    ));
                }

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
                SC_ENUM_TYPE(SC_ENUM_PROCESS),
                SERVICE_WIN32_OWN_PROCESS | SERVICE_WIN32_SHARE_PROCESS,
                SERVICE_STATE_ALL,
                Some(std::ptr::null_mut()),
                0,
                &mut bytes_needed,
                &mut services_returned,
                &mut resume_handle,
                Some(std::ptr::null()),
            );

            if result.is_ok() || GetLastError().0 != ERROR_MORE_DATA.0 {
                return Err(anyhow::anyhow!("Failed to enumerate services"));
            }
        }

        // Allocate buffer
        let mut buffer = vec![0u8; bytes_needed as usize];

        // Enumerate services
        unsafe {
            let result = EnumServicesStatusExW(
                self.sc_manager,
                SC_ENUM_TYPE(SC_ENUM_PROCESS),
                SERVICE_WIN32_OWN_PROCESS | SERVICE_WIN32_SHARE_PROCESS,
                SERVICE_STATE_ALL,
                Some(buffer.as_mut_ptr() as *mut _),
                bytes_needed,
                &mut bytes_needed,
                &mut services_returned,
                &mut resume_handle,
                Some(std::ptr::null()),
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
                if handle == INVALID_HANDLE_VALUE {
                    return Err(anyhow::anyhow!("Service not found: {}", service_name));
                }

                let info = unsafe { self.get_service_info(handle, service_name)? };
                unsafe { CloseServiceHandle(handle) };

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
                if handle == INVALID_HANDLE_VALUE {
                    return Err(anyhow::anyhow!("Service not found: {}", service_name));
                }

                let result = unsafe { StartServiceW(handle, 0, std::ptr::null()) };

                unsafe { CloseServiceHandle(handle) };

                if result.is_err() {
                    let error = GetLastError();
                    return Err(anyhow::anyhow!(
                        "Failed to start service {}: error {}",
                        service_name,
                        error.0
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
                if handle == INVALID_HANDLE_VALUE {
                    return Err(anyhow::anyhow!("Service not found: {}", service_name));
                }

                let mut status = SERVICE_STATUS::default();
                let result = unsafe { ControlService(handle, SERVICE_CONTROL_STOP, &mut status) };

                unsafe { CloseServiceHandle(handle) };

                if result.is_err() {
                    let error = GetLastError();
                    return Err(anyhow::anyhow!(
                        "Failed to stop service {}: error {}",
                        service_name,
                        error.0
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
                let exists = handle != INVALID_HANDLE_VALUE;
                unsafe { CloseServiceHandle(handle) };
                exists
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
            service_type: (*entry).ServiceStatusProcess.dwServiceType,
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
            service_flags: (*entry).ServiceStatusProcess.dwServiceFlags,
        };

        // Determine start type (would need additional query)
        let start_type = ServiceStartType::Unknown;

        // Determine capabilities
        let can_stop = status.controls_accepted & SERVICE_ACCEPT_STOP != 0;
        let can_start = true; // Simplified
        let can_delete = false; // Would need additional check

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
            Some(buffer.as_mut_ptr() as *mut _),
            buffer.len() as u32,
            &mut bytes_needed,
        );

        if result.is_err() {
            return Err(anyhow::anyhow!("Failed to query service status"));
        }

        let status_process = &*(buffer.as_ptr() as *const SERVICE_STATUS_PROCESS);

        let status = ServiceStatus {
            service_type: status_process.dwServiceType,
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
            service_flags: status_process.dwServiceFlags,
        };

        Ok(ServiceInfo {
            name: service_name.to_string(),
            display_name: service_name.to_string(), // Simplified
            status,
            start_type: ServiceStartType::Unknown,
            can_stop: status.controls_accepted & SERVICE_ACCEPT_STOP != 0,
            can_start: true,
            can_delete: false,
        })
    }
}

impl Drop for ServiceManager {
    fn drop(&mut self) {
        if self.sc_manager != INVALID_HANDLE_VALUE {
            unsafe {
                CloseServiceHandle(self.sc_manager);
            }
        }
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

// Windows API constants
const SERVICE_WIN32_OWN_PROCESS: u32 = 0x00000010;
const SERVICE_WIN32_SHARE_PROCESS: u32 = 0x00000020;
const SERVICE_STATE_ALL: u32 = SERVICE_STATE_TYPE(0x00000003);
const SERVICE_ACCEPT_STOP: u32 = 0x00000001;
const SC_STATUS_PROCESS_INFO: u32 = 0;

// Windows API structures
#[repr(C)]
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
        if ptr.is_null() {
            return String::new();
        }

        let mut len = 0;
        let mut current = ptr;
        while *current != 0 {
            len += 1;
            current = current.add(1);
        }

        let slice = std::slice::from_raw_parts(ptr, len);
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
}
