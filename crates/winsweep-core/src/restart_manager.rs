//! Windows Restart Manager
//!
//! This module provides functionality to use the Windows Restart Manager
//! to handle file locks and restart applications/services.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use tracing::{debug, info, warn};
use windows::core::PWSTR;
use windows::Win32::System::RestartManager::{
    RmEndSession, RmGetList, RmRegisterResources, RmRestart, RmShutdown, RmStartSession,
    CCH_RM_SESSION_KEY, RM_PROCESS_INFO,
};

/// Restart Manager for handling file locks and application restarts
pub struct RestartManager {
    session_key: String,
    session_handle: u32,
}

/// Information about an application using a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestartApplication {
    pub application_name: String,
    pub application_type: ApplicationType,
    pub service_short_name: Option<String>,
    pub application_status: ApplicationStatus,
    pub process_id: Option<u32>,
    pub can_restart: bool,
    pub can_shutdown: bool,
}

/// Application type in Restart Manager
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApplicationType {
    Unknown,
    Application,
    Service,
    Explorer,
    Console,
    Critical,
}

/// Application status in Restart Manager
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApplicationStatus {
    Unknown,
    Running,
    Stopped,
    Restarted,
    Shutdown,
    Failed,
}

/// Restart session information
#[derive(Debug, Clone)]
pub struct RestartSession {
    pub session_key: String,
    pub applications: Vec<RestartApplication>,
    pub reboot_reason: RebootReason,
}

/// Reason for reboot
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RebootReason {
    None,
    ApplicationPending,
    ApplicationCritical,
    SystemCritical,
    PermissionDenied,
    SessionMismatch,
}

impl RestartManager {
    /// Create a new restart manager session
    pub fn new() -> Result<Self> {
        let mut session_key = [0u16; CCH_RM_SESSION_KEY as usize];
        let mut session_handle = 0u32;

        unsafe {
            RmStartSession(&mut session_handle, 0, PWSTR(session_key.as_mut_ptr()))
                .map_err(|e| anyhow::anyhow!("Failed to start Restart Manager session: {}", e))?;
        }

        let key_string = String::from_utf16_lossy(&session_key)
            .trim_end_matches('\0')
            .to_string();

        info!("Started Restart Manager session: {}", key_string);

        Ok(Self {
            session_key: key_string,
            session_handle,
        })
    }

    /// Register resources (files) with the restart manager
    pub fn register_files(&mut self, files: &[PathBuf]) -> Result<()> {
        info!("Registering {} files with Restart Manager", files.len());

        // Convert paths to wide strings
        let mut wide_paths: Vec<Vec<u16>> = Vec::with_capacity(files.len());
        for file in files {
            let wide_path = file
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect::<Vec<u16>>();
            wide_paths.push(wide_path);
        }
        let file_ptrs: Vec<windows::core::PCWSTR> = wide_paths
            .iter()
            .map(|p| windows::core::PCWSTR(p.as_ptr()))
            .collect();

        // Register the files
        unsafe {
            RmRegisterResources(self.session_handle, Some(&file_ptrs), None, None)
                .map_err(|e| anyhow::anyhow!("Failed to register resources: {}", e))?;
        }

        Ok(())
    }

    /// Get list of applications using the registered resources
    pub fn get_applications(&mut self) -> Result<Vec<RestartApplication>> {
        debug!("Getting applications using registered resources");

        // First call to get required buffer size
        let mut proc_info_needed = 0u32;
        let mut proc_info = 0u32;
        let mut reboot_reasons = 0u32;
        unsafe {
            RmGetList(
                self.session_handle,
                &mut proc_info_needed,
                &mut proc_info,
                None,
                &mut reboot_reasons,
            )
            .map_err(|e| anyhow::anyhow!("Failed to get application list size: {}", e))?;
        }

        // Allocate buffer
        let mut apps_info: Vec<RM_PROCESS_INFO> = (0..proc_info_needed)
            .map(|_| unsafe { std::mem::zeroed() })
            .collect();
        proc_info = proc_info_needed;

        // Get the actual list
        unsafe {
            RmGetList(
                self.session_handle,
                &mut proc_info_needed,
                &mut proc_info,
                if apps_info.is_empty() {
                    None
                } else {
                    Some(apps_info.as_mut_ptr())
                },
                &mut reboot_reasons,
            )
            .map_err(|e| anyhow::anyhow!("Failed to get application list: {}", e))?;
        }

        // Parse the results
        let mut applications = Vec::new();

        for (_i, app_info) in apps_info.iter().enumerate().take(proc_info as usize) {
            let app_name = from_wide_array(&app_info.strAppName);
            let service_name = from_wide_array(&app_info.strServiceShortName);

            // Filter out empty entries
            if app_name.is_empty() {
                continue;
            }

            let application = RestartApplication {
                application_name: app_name,
                application_type: ApplicationType::from(app_info.ApplicationType.0 as u32),
                service_short_name: if service_name.is_empty() {
                    None
                } else {
                    Some(service_name)
                },
                application_status: ApplicationStatus::from(app_info.AppStatus),
                process_id: if app_info.Process.dwProcessId == 0 {
                    None
                } else {
                    Some(app_info.Process.dwProcessId)
                },
                can_restart: app_info.bRestartable.as_bool(),
                can_shutdown: true, // All can be shutdown
            };

            applications.push(application);
        }

        Ok(applications)
    }

    /// Shutdown applications using the resources
    pub fn shutdown_applications(&mut self) -> Result<()> {
        info!("Shutting down applications using resources");

        unsafe {
            RmShutdown(self.session_handle, RM_SHUTDOWN_TYPE_NORMAL, None)
                .map_err(|e| anyhow::anyhow!("Failed to shutdown applications: {}", e))?;
        }

        Ok(())
    }

    /// Restart applications after operation
    pub fn restart_applications(&mut self) -> Result<()> {
        info!("Restarting applications");

        unsafe {
            RmRestart(self.session_handle, RM_REBOOT_REASON_NONE, None)
                .map_err(|e| anyhow::anyhow!("Failed to restart applications: {}", e))?;
        }

        Ok(())
    }

    /// Get a complete restart session with applications
    pub fn get_session(&mut self, files: &[PathBuf]) -> Result<RestartSession> {
        // Register files
        self.register_files(files)?;

        // Get applications
        let applications = self.get_applications()?;

        // Determine reboot reason
        let reboot_reason = self.determine_reboot_reason(&applications);

        Ok(RestartSession {
            session_key: self.session_key.clone(),
            applications,
            reboot_reason,
        })
    }

    /// Determine if a reboot is required
    pub fn determine_reboot_reason(&self, applications: &[RestartApplication]) -> RebootReason {
        for app in applications {
            match app.application_status {
                ApplicationStatus::Failed => {
                    return RebootReason::ApplicationCritical;
                }
                ApplicationStatus::Running if !app.can_shutdown => {
                    return RebootReason::ApplicationCritical;
                }
                _ => {}
            }

            if app.application_type == ApplicationType::Critical && app.can_restart {
                return RebootReason::ApplicationCritical;
            }
        }

        RebootReason::None
    }

    /// Check if any applications are using the specified files
    pub async fn check_file_usage(&mut self, files: &[PathBuf]) -> Result<Vec<RestartApplication>> {
        let session = self.get_session(files)?;
        Ok(session.applications)
    }

    /// Safely shutdown and restart applications for an operation
    pub async fn safe_operation<F, R>(&mut self, files: &[PathBuf], operation: F) -> Result<R>
    where
        F: FnOnce() -> Result<R>,
    {
        info!("Starting safe operation with {} files", files.len());

        // Get applications using the files
        let session = self.get_session(files)?;

        if session.applications.is_empty() {
            // No applications using the files, just run the operation
            info!("No applications using the files, proceeding directly");
            return operation();
        }

        // Shutdown applications
        self.shutdown_applications()?;

        // Wait a moment for shutdown to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        // Run the operation
        let result = operation();

        // Restart applications
        if let Err(e) = self.restart_applications() {
            warn!("Failed to restart some applications: {}", e);
        }

        result
    }
}

impl Drop for RestartManager {
    fn drop(&mut self) {
        if self.session_handle != 0 {
            unsafe {
                let _ = RmEndSession(self.session_handle);
            }
        }
    }
}

impl From<u32> for ApplicationType {
    fn from(value: u32) -> Self {
        match value {
            1 => ApplicationType::Application,
            2 => ApplicationType::Service,
            3 => ApplicationType::Explorer,
            4 => ApplicationType::Console,
            5 => ApplicationType::Critical,
            _ => ApplicationType::Unknown,
        }
    }
}

impl From<u32> for ApplicationStatus {
    fn from(value: u32) -> Self {
        match value {
            1 => ApplicationStatus::Running,
            2 => ApplicationStatus::Stopped,
            3 => ApplicationStatus::Restarted,
            4 => ApplicationStatus::Shutdown,
            5 => ApplicationStatus::Failed,
            _ => ApplicationStatus::Unknown,
        }
    }
}

// Constants for Restart Manager
const RM_SHUTDOWN_TYPE_NORMAL: u32 = 1;
const RM_REBOOT_REASON_NONE: u32 = 0;

/// Convert wide array to string
fn from_wide_array(wide_array: &[u16]) -> String {
    let end = wide_array
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(wide_array.len());
    String::from_utf16_lossy(&wide_array[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_restart_manager_session() {
        // This test requires running on Windows with appropriate permissions
        #[cfg(windows)]
        {
            let manager = RestartManager::new();
            assert!(manager.is_ok());
        }
    }

    #[test]
    fn test_application_type_conversion() {
        assert_eq!(ApplicationType::from(1), ApplicationType::Application);
        assert_eq!(ApplicationType::from(2), ApplicationType::Service);
        assert_eq!(ApplicationType::from(99), ApplicationType::Unknown);
    }

    #[test]
    fn test_application_status_conversion() {
        assert_eq!(ApplicationStatus::from(1), ApplicationStatus::Running);
        assert_eq!(ApplicationStatus::from(2), ApplicationStatus::Stopped);
        assert_eq!(ApplicationStatus::from(99), ApplicationStatus::Unknown);
    }
}
