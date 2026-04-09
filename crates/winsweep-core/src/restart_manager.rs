//! Windows Restart Manager
//!
//! This module provides functionality to use the Windows Restart Manager
//! to handle file locks and restart applications/services.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use tracing::{debug, error, info, warn};
use windows::core::{GUID, PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_SUCCESS, HANDLE, HRESULT};
use windows::Win32::System::RestartManager::{
    RmEndSession, RmGetList, RmRegisterResources, RmRestart, RmShutdown, RmStartSession,
    CCH_RM_MAX_APP_NAME, CCH_RM_MAX_SVC_NAME, CCH_RM_SESSION_KEY, RM_APP_STATUS, RM_APP_TYPE,
    RM_REBOOT_REASON, RM_SESSION_KEY, RM_SHUTDOWN_TYPE, RM_START_PHASE,
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

        let result =
            unsafe { RmStartSession(&mut session_handle, 0, PWSTR(session_key.as_mut_ptr())) };

        if result != ERROR_SUCCESS {
            return Err(anyhow::anyhow!(
                "Failed to start Restart Manager session: error {}",
                result
            ));
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
        let mut file_ptrs: Vec<*const u16> = Vec::with_capacity(files.len());
        let mut wide_paths: Vec<Vec<u16>> = Vec::with_capacity(files.len());

        for file in files {
            let wide_path = file
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect::<Vec<u16>>();
            file_ptrs.push(wide_path.as_ptr());
            wide_paths.push(wide_path);
        }

        // Register the files
        let result = unsafe {
            RmRegisterResources(
                self.session_handle,
                files.len() as u32,
                file_ptrs.as_ptr(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
            )
        };

        if result != ERROR_SUCCESS {
            return Err(anyhow::anyhow!(
                "Failed to register resources: error {}",
                result
            ));
        }

        Ok(())
    }

    /// Get list of applications using the registered resources
    pub fn get_applications(&mut self) -> Result<Vec<RestartApplication>> {
        debug!("Getting applications using registered resources");

        // First call to get required buffer size
        let mut proc_info_needed = 0u32;
        let mut apps_info_size = 0u32;

        let result = unsafe {
            RmGetList(
                self.session_handle,
                &mut proc_info_needed,
                &mut apps_info_size,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };

        if result != ERROR_SUCCESS && result != 234 {
            // 234 = ERROR_MORE_DATA
            return Err(anyhow::anyhow!(
                "Failed to get application list size: error {}",
                result
            ));
        }

        // Allocate buffers
        let mut proc_info = vec![0u32; proc_info_needed as usize];
        let mut apps_info = vec![
            RM_PROCESS_INFO {
                Process: Default::default(),
                strAppName: [0; CCH_RM_MAX_APP_NAME as usize],
                strServiceShortName: [0; CCH_RM_MAX_SVC_NAME as usize],
                ApplicationType: RM_APP_TYPE(0),
                AppStatus: RM_APP_STATUS(0),
                TSSessionId: 0,
                bRestartable: 0,
                bForceRestart: 0,
            };
            apps_info_size as usize
        ];

        // Get the actual list
        let result = unsafe {
            RmGetList(
                self.session_handle,
                &mut proc_info_needed,
                &mut apps_info_size,
                proc_info.as_mut_ptr(),
                apps_info.as_mut_ptr(),
            )
        };

        if result != ERROR_SUCCESS {
            return Err(anyhow::anyhow!(
                "Failed to get application list: error {}",
                result
            ));
        }

        // Parse the results
        let mut applications = Vec::new();

        for (i, app_info) in apps_info.iter().enumerate().take(apps_info_size as usize) {
            let app_name = from_wide_array(&app_info.strAppName);
            let service_name = from_wide_array(&app_info.strServiceShortName);

            // Filter out empty entries
            if app_name.is_empty() {
                continue;
            }

            let application = RestartApplication {
                application_name: app_name,
                application_type: ApplicationType::from(app_info.ApplicationType.0),
                service_short_name: if service_name.is_empty() {
                    None
                } else {
                    Some(service_name)
                },
                application_status: ApplicationStatus::from(app_info.AppStatus.0),
                process_id: if app_info.Process.dwProcessId == 0 {
                    None
                } else {
                    Some(app_info.Process.dwProcessId)
                },
                can_restart: app_info.bRestartable != 0,
                can_shutdown: true, // All can be shutdown
            };

            applications.push(application);
        }

        Ok(applications)
    }

    /// Shutdown applications using the resources
    pub fn shutdown_applications(&mut self) -> Result<()> {
        info!("Shutting down applications using resources");

        let result = unsafe {
            RmShutdown(
                self.session_handle,
                RM_SHUTDOWN_TYPE(RmShutdownTypeNormal as u32),
                RM_REBOOT_REASON(RmRebootReasonNone as u32),
            )
        };

        if result != ERROR_SUCCESS {
            return Err(anyhow::anyhow!(
                "Failed to shutdown applications: error {}",
                result
            ));
        }

        Ok(())
    }

    /// Restart applications after operation
    pub fn restart_applications(&mut self) -> Result<()> {
        info!("Restarting applications");

        let result = unsafe {
            RmRestart(
                self.session_handle,
                RM_REBOOT_REASON(RmRebootReasonNone as u32),
                std::ptr::null_mut(),
            )
        };

        if result != ERROR_SUCCESS {
            return Err(anyhow::anyhow!(
                "Failed to restart applications: error {}",
                result
            ));
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
const RmShutdownTypeNormal: u32 = 1;
const RmRebootReasonNone: u32 = 0;

// Windows API structure
#[repr(C)]
struct RM_PROCESS_INFO {
    Process: windows::Win32::System::Threading::PROCESS_INFORMATION,
    strAppName: [u16; CCH_RM_MAX_APP_NAME as usize],
    strServiceShortName: [u16; CCH_RM_MAX_SVC_NAME as usize],
    ApplicationType: RM_APP_TYPE,
    AppStatus: RM_APP_STATUS,
    TSSessionId: u32,
    bRestartable: i32,
    bForceRestart: i32,
}

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
