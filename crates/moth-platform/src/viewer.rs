// SPDX-License-Identifier: GPL-3.0-or-later
//! OS associations launch only validated images prepared by the storage worker.
use std::{io, path::Path};

#[cfg(not(target_os = "windows"))]
pub(super) fn open(path: &Path) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(not(target_os = "macos"))]
    let program = "xdg-open";
    let status = std::process::Command::new(program).arg(path).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(
            "System image viewer could not open the image",
        ))
    }
}

#[cfg(target_os = "windows")]
pub(super) fn open(path: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: path is NUL-terminated and lives through the call; no owner window is needed.
    let result = unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            std::ptr::null_mut(),
            std::ptr::null(),
            path.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    };
    if result as isize > 32 {
        Ok(())
    } else {
        Err(io::Error::other(
            "System image viewer could not open the image",
        ))
    }
}
