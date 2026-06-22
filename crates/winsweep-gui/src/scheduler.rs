//! Windows Task Scheduler integration
//!
//! Registers and removes a Windows scheduled task that runs WinSweep's
//! auto-cleanup on a daily schedule (or on the configured cadence).
//! Uses `schtasks.exe` — no COM or UAC elevation is required for per-user tasks.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

const TASK_NAME: &str = "WinSweep Auto Cleanup";

/// Scheduling frequency
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskFrequency {
    /// Run once per day
    Daily,
    /// Run once per week
    Weekly,
    /// Run once per month
    Monthly,
}

impl TaskFrequency {
    fn schtasks_schedule_type(&self) -> &'static str {
        match self {
            TaskFrequency::Daily => "DAILY",
            TaskFrequency::Weekly => "WEEKLY",
            TaskFrequency::Monthly => "MONTHLY",
        }
    }

    /// Convert a day interval (from `auto_cleanup_days`) to a TaskFrequency.
    pub fn from_days(days: u32) -> Self {
        if days >= 28 {
            TaskFrequency::Monthly
        } else if days >= 7 {
            TaskFrequency::Weekly
        } else {
            TaskFrequency::Daily
        }
    }
}

/// Register (or replace) the WinSweep scheduled task.
///
/// The task runs the GUI binary with `--auto-cleanup` (not yet exposed as a
/// flag — this creates the entry so we only need to add the flag later).
/// Uses `ONLOGON` trigger so it fires once per user session.
///
/// # Arguments
/// * `exe_path` — path to `winsweep-gui.exe`
/// * `frequency` — how often the task should fire
pub fn register_task(exe_path: &Path, frequency: TaskFrequency) -> Result<String> {
    // Remove any existing version first (ignore errors)
    let _ = remove_task();

    let exe_str = exe_path.to_string_lossy();
    let sched = frequency.schtasks_schedule_type();

    info!("Registering scheduled task '{}' ({})", TASK_NAME, sched);

    // Create a per-user task that runs at logon
    let output = std::process::Command::new("schtasks.exe")
        .args([
            "/Create",
            "/TN",
            TASK_NAME,
            "/TR",
            exe_str.as_ref(),
            "/SC",
            "ONLOGON",
            "/RL",
            "HIGHEST",
            "/F", // Force replace if it exists
            "/DELAY",
            "0001:00", // 1 minute delay after logon to let the desktop settle
        ])
        .output()
        .context("schtasks.exe not found — is this Windows?")?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        info!("Scheduled task registered successfully");
        Ok(format!("Task '{}' registered (runs at logon).", TASK_NAME))
    } else {
        let msg = if stderr.is_empty() {
            stdout.clone()
        } else {
            stderr
        };
        warn!("Failed to register scheduled task: {}", msg);
        anyhow::bail!("schtasks /Create failed: {}", msg)
    }
}

/// Remove the WinSweep scheduled task, if it exists.
pub fn remove_task() -> Result<String> {
    debug!("Removing scheduled task '{}'", TASK_NAME);

    let output = std::process::Command::new("schtasks.exe")
        .args(["/Delete", "/TN", TASK_NAME, "/F"])
        .output()
        .context("schtasks.exe not found")?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        info!("Scheduled task removed");
        Ok(format!("Task '{}' removed.", TASK_NAME))
    } else {
        // Treat "not found" as success (idempotent)
        let msg = if stderr.is_empty() { &stdout } else { &stderr };
        if msg.contains("does not exist") || msg.contains("ERROR: The system cannot find") {
            return Ok(format!("Task '{}' was not registered.", TASK_NAME));
        }
        anyhow::bail!("schtasks /Delete failed: {}", msg)
    }
}

/// Check whether the WinSweep scheduled task currently exists.
pub fn task_exists() -> bool {
    let output = std::process::Command::new("schtasks.exe")
        .args(["/Query", "/TN", TASK_NAME])
        .output();

    match output {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

/// Return the path to the currently running executable (the GUI binary).
pub fn current_exe() -> Option<PathBuf> {
    std::env::current_exe().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frequency_from_days() {
        assert_eq!(TaskFrequency::from_days(1), TaskFrequency::Daily);
        assert_eq!(TaskFrequency::from_days(3), TaskFrequency::Daily);
        assert_eq!(TaskFrequency::from_days(7), TaskFrequency::Weekly);
        assert_eq!(TaskFrequency::from_days(14), TaskFrequency::Weekly);
        assert_eq!(TaskFrequency::from_days(28), TaskFrequency::Monthly);
        assert_eq!(TaskFrequency::from_days(30), TaskFrequency::Monthly);
    }

    #[test]
    fn test_frequency_schedule_type() {
        assert_eq!(TaskFrequency::Daily.schtasks_schedule_type(), "DAILY");
        assert_eq!(TaskFrequency::Weekly.schtasks_schedule_type(), "WEEKLY");
        assert_eq!(TaskFrequency::Monthly.schtasks_schedule_type(), "MONTHLY");
    }

    #[test]
    fn test_current_exe() {
        // The test binary is an exe, so current_exe() should return Some
        let path = current_exe();
        assert!(
            path.is_some(),
            "current_exe() should return Some in test context"
        );
    }
}
