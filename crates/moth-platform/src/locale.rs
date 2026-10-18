// SPDX-License-Identifier: GPL-3.0-or-later
//! Best-effort detection of the user's interface language, used only as the
//! first-launch default before a choice is persisted in the settings file.
/// Returns the OS language tag; the UI resolves it against bundled catalogs.
pub fn detect() -> String {
    raw_language()
}

#[cfg(not(target_os = "windows"))]
fn raw_language() -> String {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(var)
            && !value.is_empty()
            && value != "C"
            && value != "POSIX"
        {
            return value.to_ascii_lowercase();
        }
    }
    String::new()
}

#[cfg(target_os = "windows")]
fn raw_language() -> String {
    let mut buffer = [0u16; 85];
    // SAFETY: both functions accept these values; the writable buffer matches its capacity.
    let length = unsafe {
        windows_sys::Win32::Globalization::LCIDToLocaleName(
            u32::from(windows_sys::Win32::Globalization::GetUserDefaultUILanguage()),
            buffer.as_mut_ptr(),
            buffer.len() as i32,
            0,
        )
    };
    if length > 0 {
        String::from_utf16_lossy(&buffer[..length as usize - 1])
    } else {
        String::new()
    }
}
