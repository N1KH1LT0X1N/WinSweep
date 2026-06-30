//! Self-updater with Authenticode signature verification

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::info;

/// Current version from Cargo
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Update status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum UpdateStatus {
    UpToDate,
    UpdateAvailable { version: String, url: String },
    Downloaded { path: PathBuf },
    Applied,
    Error(String),
}

/// Release info from GitHub
#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

/// Check GitHub Releases for a newer version
pub async fn check_for_update() -> Result<UpdateStatus> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("WinSweep-Updater")
        .build()?;

    let resp = client
        .get("https://api.github.com/repos/N1KH1LT0X1N/WinSweep/releases/latest")
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => {
            let release: GithubRelease = r.json().await?;
            let latest = release.tag_name.trim_start_matches('v');
            if version_gt(latest, CURRENT_VERSION) {
                let asset = release.assets.iter().find(|a| {
                    a.name.contains("x86_64")
                        && (a.name.ends_with(".exe") || a.name.ends_with(".zip"))
                });
                if let Some(a) = asset {
                    return Ok(UpdateStatus::UpdateAvailable {
                        version: latest.to_string(),
                        url: a.browser_download_url.clone(),
                    });
                }
                return Ok(UpdateStatus::UpToDate);
            }
            Ok(UpdateStatus::UpToDate)
        }
        Ok(r) => Ok(UpdateStatus::Error(format!("HTTP {}", r.status()))),
        Err(e) => Ok(UpdateStatus::Error(e.to_string())),
    }
}

/// Download update to temp path
pub async fn download_update(url: &str) -> Result<PathBuf> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;
    let resp = client.get(url).send().await?;
    let bytes = resp.bytes().await?;

    let temp_dir = std::env::temp_dir().join("winsweep-update");
    std::fs::create_dir_all(&temp_dir)?;
    let temp_path = temp_dir.join("winsweep-update.exe");
    std::fs::write(&temp_path, &bytes)?;
    info!("Downloaded update to {}", temp_path.display());
    Ok(temp_path)
}

/// Verify Authenticode signature via WinVerifyTrust (Windows only)
pub fn verify_signature(path: &std::path::Path) -> Result<bool> {
    #[cfg(windows)]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::Foundation::{ERROR_SUCCESS, WIN32_ERROR};
        use windows::Win32::Security::WinTrust::{
            WinVerifyTrust, WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_FILE_INFO,
            WTD_CHOICE_FILE, WTD_REVOKE_NONE, WTD_STATEACTION_CLOSE, WTD_STATEACTION_VERIFY,
            WTD_UI_NONE,
        };

        let wide: Vec<u16> = OsStr::new(path.as_os_str())
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let file_info = WINTRUST_FILE_INFO {
            cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
            pcwszFilePath: windows::core::PCWSTR(wide.as_ptr()),
            hFile: windows::Win32::Foundation::HANDLE(0),
            pgKnownSubject: std::ptr::null_mut(),
        };

        let mut trust_data = WINTRUST_DATA {
            cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
            pPolicyCallbackData: std::ptr::null_mut(),
            pSIPClientData: std::ptr::null_mut(),
            dwUIChoice: WTD_UI_NONE,
            fdwRevocationChecks: WTD_REVOKE_NONE,
            dwUnionChoice: WTD_CHOICE_FILE,
            ..Default::default()
        };
        trust_data.Anonymous.pFile = &file_info as *const _ as *mut _;
        trust_data.dwStateAction = WTD_STATEACTION_VERIFY;

        let mut action_guid = WINTRUST_ACTION_GENERIC_VERIFY_V2;
        let hwnd = windows::Win32::Foundation::HWND(0);
        let result =
            unsafe { WinVerifyTrust(hwnd, &mut action_guid, &mut trust_data as *mut _ as *mut _) };

        trust_data.dwStateAction = WTD_STATEACTION_CLOSE;
        unsafe {
            let _ = WinVerifyTrust(hwnd, &mut action_guid, &mut trust_data as *mut _ as *mut _);
        }

        Ok(WIN32_ERROR(result as u32) == ERROR_SUCCESS)
    }
    #[cfg(not(windows))]
    {
        Ok(true)
    }
}

/// Apply update on restart (rename-in-place strategy)
pub fn apply_update_on_restart(update_path: &std::path::Path) -> Result<()> {
    let current_exe = std::env::current_exe()?;
    let old_path = current_exe.with_extension("old.exe");

    // Write a batch script that replaces the exe on next start
    let batch = format!(
        "@echo off\ntimeout /t 2 /nobreak >nul\nmove /Y \"{}\" \"{}\"\nmove /Y \"{}\" \"{}\"\ndel \"{}\"\nstart \"\" \"{}\"\n",
        current_exe.display(),
        old_path.display(),
        update_path.display(),
        current_exe.display(),
        old_path.display(),
        current_exe.display(),
    );

    let batch_path = std::env::temp_dir().join("winsweep-update.bat");
    std::fs::write(&batch_path, batch)?;

    std::process::Command::new("cmd")
        .args(["/C", batch_path.to_str().unwrap_or("")])
        .spawn()?;

    info!("Scheduled update on restart: {}", update_path.display());
    Ok(())
}

/// Compare two semantic version strings
fn version_gt(a: &str, b: &str) -> bool {
    let parse = |s: &str| {
        s.split('.')
            .filter_map(|p| p.parse::<u32>().ok())
            .collect::<Vec<_>>()
    };
    let av = parse(a);
    let bv = parse(b);
    for i in 0..av.len().max(bv.len()) {
        let ai = av.get(i).copied().unwrap_or(0);
        let bi = bv.get(i).copied().unwrap_or(0);
        if ai != bi {
            return ai > bi;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_gt() {
        assert!(version_gt("1.0.1", "1.0.0"));
        assert!(version_gt("2.0.0", "1.9.9"));
        assert!(!version_gt("1.0.0", "1.0.0"));
        assert!(!version_gt("0.9.0", "1.0.0"));
    }

    #[test]
    fn test_verify_signature_missing_file() {
        let path = PathBuf::from(r"C:\does\not\exist.exe");
        assert!(!verify_signature(&path).unwrap_or(false));
    }
}
