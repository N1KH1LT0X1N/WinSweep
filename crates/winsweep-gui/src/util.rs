//! Windows-specific utility helpers for the GUI

use windows_sys::Win32::UI::Shell::{SHQueryRecycleBinW, SHQUERYRBINFO};

/// Returns the total size (in bytes) of the Recycle Bin on the C: drive.
/// Falls back to 0 on failure.
pub fn recycle_bin_size() -> u64 {
    unsafe {
        let mut info = SHQUERYRBINFO {
            cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32,
            i64Size: 0,
            i64NumItems: 0,
        };
        let path: Vec<u16> =
            std::os::windows::ffi::OsStrExt::encode_wide(std::ffi::OsStr::new("C:\\"))
                .chain(std::iter::once(0))
                .collect();
        let hr = SHQueryRecycleBinW(path.as_ptr(), &mut info);
        if hr >= 0 {
            info.i64Size as u64
        } else {
            0
        }
    }
}

/// Empties the Recycle Bin on the C: drive.
pub fn empty_recycle_bin() {
    unsafe {
        let path: Vec<u16> =
            std::os::windows::ffi::OsStrExt::encode_wide(std::ffi::OsStr::new("C:\\"))
                .chain(std::iter::once(0))
                .collect();
        let _ = windows_sys::Win32::UI::Shell::SHEmptyRecycleBinW(
            std::ptr::null_mut(),
            path.as_ptr(),
            0,
        );
    }
}
