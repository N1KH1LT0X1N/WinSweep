//! Elevated Operation Coordinator
//!
//! Handles communication between the GUI (running as user) and elevated backend processes
//! for operations that require administrator privileges.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tokio::process::{Child, Command};
use tracing::{debug, error, info, warn};
use winsweep_common::Config;

/// Types of elevated operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ElevatedOperation {
    /// Clean Windows Update cache
    CleanWindowsUpdate {
        remove_downloads: bool,
        compress_backups: bool,
        remove_old_versions: bool,
    },
    /// Clean system temporary files
    CleanSystemTemp {
        include_user_temp: bool,
        include_system_temp: bool,
    },
    /// Clean prefetch files
    CleanPrefetch,
    /// Stop and start Windows services
    ManageService {
        service_name: String,
        action: ServiceAction,
    },
    /// Delete protected system files
    DeleteSystemFiles {
        paths: Vec<PathBuf>,
        use_recycle_bin: bool,
    },
    /// Compact WSL VHDX files
    CompactWslVhdx { distribution_name: Option<String> },
}

/// Service management actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServiceAction {
    Stop,
    Start,
    Restart,
    Disable,
    Enable,
}

/// Elevated operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElevatedOperationResult {
    /// Whether the operation succeeded
    pub success: bool,
    /// Error message if failed
    pub error_message: Option<String>,
    /// Number of files deleted
    pub files_deleted: u64,
    /// Total space freed in bytes
    pub space_freed: u64,
    /// Additional operation-specific data
    pub details: serde_json::Value,
}

/// Progress update for long-running operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressUpdate {
    /// Progress percentage (0-100)
    pub percentage: u8,
    /// Current operation description
    pub message: String,
    /// Estimated remaining time in seconds
    pub eta_seconds: Option<u32>,
}

/// Elevated operation coordinator
pub struct ElevatedCoordinator {
    config: Config,
}

impl ElevatedCoordinator {
    /// Create a new elevated coordinator
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Execute an elevated operation
    pub async fn execute_operation(
        &self,
        operation: ElevatedOperation,
        progress_callback: impl Fn(ProgressUpdate),
    ) -> Result<ElevatedOperationResult> {
        info!("Executing elevated operation: {:?}", operation);

        // Check if we're already running as administrator
        if self.is_running_as_admin() {
            debug!("Already running as admin, executing directly");
            self.execute_operation_direct(operation, progress_callback)
                .await
        } else {
            debug!("Not running as admin, spawning elevated process");
            self.execute_operation_elevated(operation, progress_callback)
                .await
        }
    }

    /// Check if the current process is running as administrator
    fn is_running_as_admin(&self) -> bool {
        #[cfg(windows)]
        {
            use std::mem;
            use windows_sys::Win32::Foundation::{FALSE, HANDLE};
            use windows_sys::Win32::Security::{TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
            use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

            unsafe {
                let mut token: HANDLE = 0;
                if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == FALSE {
                    return false;
                }

                let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
                let mut size = mem::size_of::<TOKEN_ELEVATION>() as u32;

                let result = windows_sys::Win32::Security::GetTokenInformation(
                    token,
                    TokenElevation,
                    &mut elevation as *mut _ as *mut _,
                    size,
                    &mut size,
                );

                result != FALSE && elevation.TokenIsElevated != 0
            }
        }
        #[cfg(not(windows))]
        {
            false
        }
    }

    /// Execute operation directly when already elevated
    async fn execute_operation_direct(
        &self,
        operation: ElevatedOperation,
        mut progress_callback: impl Fn(ProgressUpdate),
    ) -> Result<ElevatedOperationResult> {
        match operation {
            ElevatedOperation::CleanWindowsUpdate {
                remove_downloads,
                compress_backups,
                remove_old_versions,
            } => {
                self.clean_windows_update_direct(
                    remove_downloads,
                    compress_backups,
                    remove_old_versions,
                    &mut progress_callback,
                )
                .await
            }
            ElevatedOperation::CleanSystemTemp {
                include_user_temp,
                include_system_temp,
            } => {
                self.clean_system_temp_direct(
                    include_user_temp,
                    include_system_temp,
                    &mut progress_callback,
                )
                .await
            }
            ElevatedOperation::CleanPrefetch => {
                self.clean_prefetch_direct(&mut progress_callback).await
            }
            ElevatedOperation::ManageService {
                service_name,
                action,
            } => {
                self.manage_service_direct(&service_name, action, &mut progress_callback)
                    .await
            }
            ElevatedOperation::DeleteSystemFiles {
                paths,
                use_recycle_bin,
            } => {
                self.delete_system_files_direct(paths, use_recycle_bin, &mut progress_callback)
                    .await
            }
            ElevatedOperation::CompactWslVhdx { distribution_name } => {
                self.compact_wsl_vhdx_direct(distribution_name, &mut progress_callback)
                    .await
            }
        }
    }

    /// Execute operation by spawning an elevated process
    async fn execute_operation_elevated(
        &self,
        operation: ElevatedOperation,
        progress_callback: impl Fn(ProgressUpdate),
    ) -> Result<ElevatedOperationResult> {
        // Create temporary files for communication
        let temp_dir = std::env::temp_dir();
        let request_id = uuid::Uuid::new_v4().to_string();
        let request_file = temp_dir.join(format!("winsweep_request_{}.json", request_id));
        let response_file = temp_dir.join(format!("winsweep_response_{}.json", request_id));

        // Write operation to request file
        let operation_json =
            serde_json::to_string(&operation).context("Failed to serialize operation")?;
        tokio::fs::write(&request_file, operation_json)
            .await
            .context("Failed to write request file")?;

        // Start the elevated helper with file paths
        let helper_path =
            std::env::current_exe().context("Failed to get current executable path")?;

        let mut child = Command::new("powershell")
            .arg("-Command")
            .arg(format!("Start-Process '{}' -ArgumentList '--elevated-operation', '{}', '{}' -Verb RunAs -Wait", 
                helper_path.display(),
                request_file.display(),
                response_file.display()))
            .spawn()
            .context("Failed to spawn elevated process")?;

        // Wait for process to complete
        let status = child
            .wait()
            .context("Failed to wait for elevated process")?;

        // Read response file
        let result = if status.success() {
            // Wait a moment for the response file to be written
            tokio::time::sleep(Duration::from_millis(100)).await;

            if response_file.exists() {
                let response_json = tokio::fs::read_to_string(&response_file)
                    .await
                    .context("Failed to read response file")?;

                serde_json::from_str::<ElevatedOperationResult>(&response_json)
                    .context("Failed to parse elevated operation result")
            } else {
                Err(anyhow::anyhow!(
                    "Elevated process did not create response file"
                ))
            }
        } else {
            Err(anyhow::anyhow!(
                "Elevated process failed with exit code: {:?}",
                status.code()
            ))
        };

        // Clean up temporary files
        let _ = tokio::fs::remove_file(&request_file).await;
        let _ = tokio::fs::remove_file(&response_file).await;

        result
    }

    /// Direct implementation of Windows Update cleanup
    async fn clean_windows_update_direct(
        &self,
        remove_downloads: bool,
        compress_backups: bool,
        remove_old_versions: bool,
        progress_callback: &mut impl Fn(ProgressUpdate),
    ) -> Result<ElevatedOperationResult> {
        progress_callback(ProgressUpdate {
            percentage: 10,
            message: "Stopping Windows Update service...".to_string(),
            eta_seconds: Some(30),
        });

        // Stop wuauserv service
        self.stop_service("wuauserv").await?;

        progress_callback(ProgressUpdate {
            percentage: 30,
            message: "Cleaning download cache...".to_string(),
            eta_seconds: Some(20),
        });

        let mut files_deleted = 0;
        let mut space_freed = 0;

        if remove_downloads {
            let download_path = PathBuf::from(r"C:\Windows\SoftwareDistribution\Download");
            if let Ok((deleted, freed)) =
                self.delete_directory_contents(&download_path, false).await
            {
                files_deleted += deleted;
                space_freed += freed;
            }
        }

        progress_callback(ProgressUpdate {
            percentage: 60,
            message: "Compressing backup files...".to_string(),
            eta_seconds: Some(15),
        });

        if compress_backups {
            // Implement backup compression
            // This is a placeholder - real implementation would compress DataStore folders
        }

        progress_callback(ProgressUpdate {
            percentage: 80,
            message: "Removing old versions...".to_string(),
            eta_seconds: Some(10),
        });

        if remove_old_versions {
            // Remove old Windows Update folders
            let windows_path = PathBuf::from("C:\\Windows");
            if let Ok(entries) = std::fs::read_dir(&windows_path) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    if let Some(name_str) = name.to_str() {
                        if name_str.starts_with("$NtUninstall")
                            || name_str.starts_with("SoftwareDistribution")
                        {
                            if let Ok((deleted, freed)) =
                                self.delete_directory_contents(&entry.path(), false).await
                            {
                                files_deleted += deleted;
                                space_freed += freed;
                            }
                        }
                    }
                }
            }
        }

        progress_callback(ProgressUpdate {
            percentage: 90,
            message: "Restarting Windows Update service...".to_string(),
            eta_seconds: Some(5),
        });

        // Restart wuauserv service
        self.start_service("wuauserv").await?;

        progress_callback(ProgressUpdate {
            percentage: 100,
            message: "Windows Update cleanup complete".to_string(),
            eta_seconds: Some(0),
        });

        Ok(ElevatedOperationResult {
            success: true,
            error_message: None,
            files_deleted,
            space_freed,
            details: serde_json::json!({
                "remove_downloads": remove_downloads,
                "compress_backups": compress_backups,
                "remove_old_versions": remove_old_versions,
            }),
        })
    }

    /// Direct implementation of system temp cleanup
    async fn clean_system_temp_direct(
        &self,
        include_user_temp: bool,
        include_system_temp: bool,
        progress_callback: &mut impl Fn(ProgressUpdate),
    ) -> Result<ElevatedOperationResult> {
        let mut files_deleted = 0;
        let mut space_freed = 0;
        let mut progress = 0;

        if include_user_temp {
            progress_callback(ProgressUpdate {
                percentage: progress,
                message: "Cleaning user temp folders...".to_string(),
                eta_seconds: Some(30),
            });

            // Clean all user temp folders
            if let Ok(users_path) = std::fs::read_dir("C:\\Users") {
                for user_entry in users_path.flatten() {
                    let temp_path = user_entry.path().join("AppData\\Local\\Temp");
                    if temp_path.exists() {
                        if let Ok((deleted, freed)) =
                            self.delete_directory_contents(&temp_path, true).await
                        {
                            files_deleted += deleted;
                            space_freed += freed;
                        }
                    }
                }
            }
            progress += 50;
        }

        if include_system_temp {
            progress_callback(ProgressUpdate {
                percentage: progress,
                message: "Cleaning system temp folder...".to_string(),
                eta_seconds: Some(20),
            });

            let system_temp = PathBuf::from("C:\\Windows\\Temp");
            if system_temp.exists() {
                if let Ok((deleted, freed)) =
                    self.delete_directory_contents(&system_temp, true).await
                {
                    files_deleted += deleted;
                    space_freed += freed;
                }
            }
            progress += 50;
        }

        progress_callback(ProgressUpdate {
            percentage: 100,
            message: "System temp cleanup complete".to_string(),
            eta_seconds: Some(0),
        });

        Ok(ElevatedOperationResult {
            success: true,
            error_message: None,
            files_deleted,
            space_freed,
            details: serde_json::json!({
                "include_user_temp": include_user_temp,
                "include_system_temp": include_system_temp,
            }),
        })
    }

    /// Direct implementation of prefetch cleanup
    async fn clean_prefetch_direct(
        &self,
        progress_callback: &mut impl Fn(ProgressUpdate),
    ) -> Result<ElevatedOperationResult> {
        progress_callback(ProgressUpdate {
            percentage: 50,
            message: "Cleaning prefetch files...".to_string(),
            eta_seconds: Some(10),
        });

        let prefetch_path = PathBuf::from("C:\\Windows\\Prefetch");
        let (files_deleted, space_freed) = if prefetch_path.exists() {
            self.delete_directory_contents(&prefetch_path, false)
                .await
                .unwrap_or((0, 0))
        } else {
            (0, 0)
        };

        progress_callback(ProgressUpdate {
            percentage: 100,
            message: "Prefetch cleanup complete".to_string(),
            eta_seconds: Some(0),
        });

        Ok(ElevatedOperationResult {
            success: true,
            error_message: None,
            files_deleted,
            space_freed,
            details: serde_json::json!({}),
        })
    }

    /// Direct implementation of service management
    async fn manage_service_direct(
        &self,
        service_name: &str,
        action: ServiceAction,
        progress_callback: &mut impl Fn(ProgressUpdate),
    ) -> Result<ElevatedOperationResult> {
        let action_str = match action {
            ServiceAction::Stop => "Stopping",
            ServiceAction::Start => "Starting",
            ServiceAction::Restart => "Restarting",
            ServiceAction::Disable => "Disabling",
            ServiceAction::Enable => "Enabling",
        };

        progress_callback(ProgressUpdate {
            percentage: 50,
            message: format!("{} service {}...", action_str, service_name),
            eta_seconds: Some(10),
        });

        let result = match action {
            ServiceAction::Stop => self.stop_service(service_name).await,
            ServiceAction::Start => self.start_service(service_name).await,
            ServiceAction::Restart => {
                self.stop_service(service_name).await?;
                tokio::time::sleep(Duration::from_secs(2)).await;
                self.start_service(service_name).await
            }
            ServiceAction::Disable => self.disable_service(service_name).await,
            ServiceAction::Enable => self.enable_service(service_name).await,
        };

        progress_callback(ProgressUpdate {
            percentage: 100,
            message: format!("Service {} complete", action_str.to_lowercase()),
            eta_seconds: Some(0),
        });

        Ok(ElevatedOperationResult {
            success: result.is_ok(),
            error_message: result.err().map(|e| e.to_string()),
            files_deleted: 0,
            space_freed: 0,
            details: serde_json::json!({
                "service_name": service_name,
                "action": format!("{:?}", action),
            }),
        })
    }

    /// Delete system files directly
    async fn delete_system_files_direct(
        &self,
        paths: Vec<PathBuf>,
        use_recycle_bin: bool,
        progress_callback: &mut impl Fn(ProgressUpdate),
    ) -> Result<ElevatedOperationResult> {
        let mut files_deleted = 0;
        let mut space_freed = 0;
        let total_paths = paths.len();

        for (index, path) in paths.iter().enumerate() {
            progress_callback(ProgressUpdate {
                percentage: ((index * 100) / total_paths) as u8,
                message: format!("Deleting {}...", path.display()),
                eta_seconds: Some((total_paths - index) as u32),
            });

            if path.is_dir() {
                let (deleted, freed) = self
                    .delete_directory_contents(path, use_recycle_bin)
                    .await?;
                files_deleted += deleted;
                space_freed += freed;
            } else if path.is_file() {
                let size = path.metadata().ok().map(|m| m.len()).unwrap_or(0);
                if self.delete_file(path, use_recycle_bin).await? {
                    files_deleted += 1;
                    space_freed += size;
                }
            }
        }

        progress_callback(ProgressUpdate {
            percentage: 100,
            message: "System file deletion complete".to_string(),
            eta_seconds: Some(0),
        });

        Ok(ElevatedOperationResult {
            success: true,
            error_message: None,
            files_deleted,
            space_freed,
            details: serde_json::json!({
                "use_recycle_bin": use_recycle_bin,
            }),
        })
    }

    /// Compact WSL VHDX files directly
    async fn compact_wsl_vhdx_direct(
        &self,
        distribution_name: Option<String>,
        progress_callback: &mut impl Fn(ProgressUpdate),
    ) -> Result<ElevatedOperationResult> {
        progress_callback(ProgressUpdate {
            percentage: 20,
            message: "Shutting down WSL...".to_string(),
            eta_seconds: Some(30),
        });

        // Shutdown WSL
        let _ = Command::new("wsl").arg("--shutdown").output().await;

        progress_callback(ProgressUpdate {
            percentage: 40,
            message: "Locating VHDX files...".to_string(),
            eta_seconds: Some(20),
        });

        // Find VHDX files
        let mut vhdx_files = Vec::new();
        let wsl_base = PathBuf::from("C:\\Users")
            .join(std::env::var("USERNAME").unwrap_or_else(|_| "Default".to_string()))
            .join("AppData\\Local\\Packages");

        if let Ok(entries) = std::fs::read_dir(&wsl_base) {
            for entry in entries.flatten() {
                let vhdx_path = entry.path().join("LocalState\\ext4.vhdx");
                if vhdx_path.exists() {
                    if let Some(ref name) = distribution_name {
                        if entry.file_name().to_string_lossy().contains(name) {
                            vhdx_files.push(vhdx_path);
                        }
                    } else {
                        vhdx_files.push(vhdx_path);
                    }
                }
            }
        }

        progress_callback(ProgressUpdate {
            percentage: 60,
            message: "Compacting VHDX files...".to_string(),
            eta_seconds: Some(60),
        });

        let mut total_freed = 0;
        for (index, vhdx_path) in vhdx_files.iter().enumerate() {
            // Use diskpart to compact the VHD
            let script = format!(
                "select vdisk file=\"{}\"\ncompact vdisk",
                vhdx_path.display()
            );

            let output = Command::new("diskpart")
                .arg("/s")
                .arg(script)
                .output()
                .await?;

            if output.status.success() {
                // Calculate space freed (simplified)
                if let Ok(metadata) = vhdx_path.metadata() {
                    total_freed += metadata.len();
                }
            }

            progress_callback(ProgressUpdate {
                percentage: 60 + ((index * 30) / vhdx_files.len()) as u8,
                message: format!("Compacted {}", vhdx_path.display()),
                eta_seconds: Some((vhdx_files.len() - index) as u32 * 10),
            });
        }

        progress_callback(ProgressUpdate {
            percentage: 100,
            message: "WSL VHDX compaction complete".to_string(),
            eta_seconds: Some(0),
        });

        Ok(ElevatedOperationResult {
            success: true,
            error_message: None,
            files_deleted: 0,
            space_freed: total_freed,
            details: serde_json::json!({
                "vhdx_count": vhdx_files.len(),
                "distribution": distribution_name,
            }),
        })
    }

    /// Helper function to stop a Windows service
    async fn stop_service(&self, service_name: &str) -> Result<()> {
        let output = Command::new("sc")
            .arg("stop")
            .arg(service_name)
            .output()
            .await?;

        if !output.status.success() {
            warn!(
                "Failed to stop service {}: {}",
                service_name,
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(())
    }

    /// Helper function to start a Windows service
    async fn start_service(&self, service_name: &str) -> Result<()> {
        let output = Command::new("sc")
            .arg("start")
            .arg(service_name)
            .output()
            .await?;

        if !output.status.success() {
            warn!(
                "Failed to start service {}: {}",
                service_name,
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(())
    }

    /// Helper function to disable a Windows service
    async fn disable_service(&self, service_name: &str) -> Result<()> {
        let output = Command::new("sc")
            .arg("config")
            .arg(service_name)
            .arg("start=disabled")
            .output()
            .await?;

        if !output.status.success() {
            warn!(
                "Failed to disable service {}: {}",
                service_name,
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(())
    }

    /// Helper function to enable a Windows service
    async fn enable_service(&self, service_name: &str) -> Result<()> {
        let output = Command::new("sc")
            .arg("config")
            .arg(service_name)
            .arg("start=auto")
            .output()
            .await?;

        if !output.status.success() {
            warn!(
                "Failed to enable service {}: {}",
                service_name,
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(())
    }

    /// Helper function to delete directory contents
    async fn delete_directory_contents(
        &self,
        path: &PathBuf,
        use_recycle_bin: bool,
    ) -> Result<(u64, u64)> {
        let mut files_deleted = 0;
        let mut space_freed = 0;

        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.is_dir() {
                    let (deleted, freed) = self
                        .delete_directory_contents(&entry_path, use_recycle_bin)
                        .await?;
                    files_deleted += deleted;
                    space_freed += freed;

                    // Remove the empty directory
                    std::fs::remove_dir(&entry_path).ok();
                } else {
                    let size = entry.metadata().ok().map(|m| m.len()).unwrap_or(0);
                    if self.delete_file(&entry_path, use_recycle_bin).await? {
                        files_deleted += 1;
                        space_freed += size;
                    }
                }
            }
        }

        Ok((files_deleted, space_freed))
    }

    /// Helper function to delete a file
    async fn delete_file(&self, path: &PathBuf, use_recycle_bin: bool) -> Result<bool> {
        if use_recycle_bin {
            // Use Windows shell to move to recycle bin
            #[cfg(windows)]
            {
                use std::os::windows::ffi::OsStrExt;
                use windows_sys::Win32::Shell::FOF_NO_UI;
                use windows_sys::Win32::UI::Shell::SHFileOperationW;

                let mut path_wide: Vec<u16> = path.as_os_str().encode_wide().collect();
                path_wide.push(0); // Null terminator

                let mut operation = windows_sys::Win32::UI::Shell::SHFILEOPSTRUCTW {
                    hwnd: 0,
                    wFunc: windows_sys::Win32::UI::Shell::FO_DELETE,
                    pFrom: path_wide.as_ptr(),
                    pTo: std::ptr::null(),
                    fFlags: FOF_NO_UI,
                    fAnyOperationsAborted: 0,
                    hNameMappings: std::ptr::null_mut(),
                    lpszProgressTitle: std::ptr::null(),
                };

                unsafe {
                    let result = SHFileOperationW(&mut operation);
                    Ok(result == 0)
                }
            }
            #[cfg(not(windows))]
            {
                std::fs::remove_file(path).map(|_| true).map_err(Into::into)
            }
        } else {
            std::fs::remove_file(path).map(|_| true).map_err(Into::into)
        }
    }
}
