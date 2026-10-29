// SPDX-License-Identifier: GPL-3.0-or-later
//! Windows 11 blurs what is behind a window through the DWM system backdrop
//! (acrylic). Earlier builds have no supported API, so Glass keeps its tint there.
use std::sync::OnceLock;
use windows_sys::Wdk::System::SystemServices::RtlGetVersion;
use windows_sys::Win32::{
    Foundation::HWND,
    Graphics::Dwm::{
        DWMSBT_NONE, DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE, DwmSetWindowAttribute,
    },
    System::SystemInformation::OSVERSIONINFOW,
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// `DWMWA_SYSTEMBACKDROP_TYPE` is documented from Windows 11 22H2 (build 22621).
const FIRST_BUILD: u32 = 22621;

/// Whether this Windows build supports the acrylic system backdrop.
pub(crate) fn supported() -> bool {
    static SUPPORTED: OnceLock<bool> = OnceLock::new();
    *SUPPORTED.get_or_init(|| {
        let mut info = OSVERSIONINFOW {
            dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOW>() as u32,
            ..Default::default()
        };
        // SAFETY: the structure is initialized with its size, as the API requires.
        let status = unsafe { RtlGetVersion(&mut info) };
        status == 0 && info.dwBuildNumber >= FIRST_BUILD
    })
}

/// Turns the acrylic backdrop on or off for a window with a transparent surface.
pub(crate) fn set(window: &winit::window::Window, on: bool) {
    if !supported() {
        return;
    }
    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return;
    };
    let value = if on {
        DWMSBT_TRANSIENTWINDOW
    } else {
        DWMSBT_NONE
    };
    // SAFETY: a live window of this process and a pointer to an i32 of the stated size.
    let result = unsafe {
        DwmSetWindowAttribute(
            handle.hwnd.get() as HWND,
            DWMWA_SYSTEMBACKDROP_TYPE as u32,
            (&value as *const i32).cast(),
            std::mem::size_of::<i32>() as u32,
        )
    };
    if result != 0 {
        tracing::debug!(result, "acrylic backdrop unavailable");
    }
}
