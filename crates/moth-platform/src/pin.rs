// SPDX-License-Identifier: GPL-3.0-or-later
//! A foreign window never owns MothRef: destroying it must not destroy a board.
use winit::window::Window;

/// Remembers a chosen window while the user toggles its pin off and on.
#[derive(Default)]
pub struct Pin {
    #[cfg(target_os = "windows")]
    native: native::Pin,
    #[cfg(target_os = "linux")]
    native: x11::Pin,
}
impl Pin {
    /// Wakes the event loop when a native target closes, without polling idle boards.
    pub fn new(wake: std::sync::Arc<dyn Fn() + Send + Sync>) -> Self {
        #[cfg(target_os = "linux")]
        {
            Self {
                native: x11::Pin::new(wake),
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = wake;
            Self::default()
        }
    }
    /// Checks whether the saved target still exists before enabling it again.
    pub(crate) fn available(&self) -> bool {
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        {
            self.native.available()
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            false
        }
    }
    /// Installs a foreground notification, with no polling while pinned.
    pub(crate) fn enable(&mut self, window: &Window, enabled: bool) -> Result<(), String> {
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        {
            self.native.enable(window, enabled)
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            let _ = window;
            if enabled {
                Err("Window attachment is unavailable".into())
            } else {
                Ok(())
            }
        }
    }
    /// Samples only during explicit selection; Escape cancels even outside the app.
    pub(crate) fn pick(&mut self, window: &Window) -> Option<bool> {
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        {
            self.native.pick(window)
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            let _ = window;
            Some(false)
        }
    }
}

#[cfg(target_os = "windows")]
mod native {
    use super::*;
    use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
    use windows_sys::Win32::{
        Foundation::HWND,
        UI::{
            Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent},
            Input::KeyboardAndMouse::{GetAsyncKeyState, VK_ESCAPE, VK_LBUTTON},
            WindowsAndMessaging::*,
        },
    };
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    static BOARD: AtomicUsize = AtomicUsize::new(0);
    static TARGET: AtomicUsize = AtomicUsize::new(0);
    static PROCESS: AtomicU32 = AtomicU32::new(0);
    #[derive(Default)]
    pub(super) struct Pin {
        target: usize,
        process: u32,
        hook: usize,
    }
    fn handle(window: &Window) -> Option<HWND> {
        match window.window_handle().ok()?.as_raw() {
            RawWindowHandle::Win32(h) => Some(h.hwnd.get() as HWND),
            _ => None,
        }
    }
    // SAFETY: Windows invokes this on the registering event-loop thread; no Rust panics or borrows cross FFI.
    unsafe extern "system" fn foreground(
        _: HWINEVENTHOOK,
        _: u32,
        window: HWND,
        _: i32,
        _: i32,
        _: u32,
        _: u32,
    ) {
        if window as usize == TARGET.load(Ordering::Relaxed) {
            let mut process = 0;
            // SAFETY: the output pointer is valid; a destroyed window returns zero.
            unsafe {
                GetWindowThreadProcessId(window, &mut process);
            }
            if process == 0 || process != PROCESS.load(Ordering::Relaxed) {
                return;
            }
            let board = BOARD.load(Ordering::Relaxed) as HWND;
            // shortcut: normal z-order targets only; qualify topmost targets before extending this mode.
            // SAFETY: handles are validated by Windows; failure leaves stacking unchanged.
            unsafe {
                SetWindowPos(
                    board,
                    HWND_TOP,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
        }
    }
    impl Pin {
        pub(super) fn available(&self) -> bool {
            let mut process = 0;
            // SAFETY: the output pointer is valid; stale handles return zero.
            unsafe {
                GetWindowThreadProcessId(self.target as HWND, &mut process) != 0
                    && process == self.process
                    && self.target != 0
            }
        }
        pub(super) fn enable(&mut self, window: &Window, enabled: bool) -> Result<(), String> {
            self.disable();
            if !enabled {
                return Ok(());
            }
            if !self.available() {
                return Err("The chosen window has closed".into());
            }
            let board = handle(window).ok_or("Missing native window")?;
            BOARD.store(board as usize, Ordering::Relaxed);
            TARGET.store(self.target, Ordering::Relaxed);
            PROCESS.store(self.process, Ordering::Relaxed);
            // SAFETY: an out-of-context callback lives for the process lifetime and runs on this thread.
            let hook = unsafe {
                SetWinEventHook(
                    EVENT_SYSTEM_FOREGROUND,
                    EVENT_SYSTEM_FOREGROUND,
                    std::ptr::null_mut(),
                    Some(foreground),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT,
                )
            };
            if hook.is_null() {
                return Err(std::io::Error::last_os_error().to_string());
            }
            self.hook = hook as usize;
            // SAFETY: this is our own live top-level window; this never transfers ownership.
            unsafe {
                SetWindowPos(
                    board,
                    HWND_TOP,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
            Ok(())
        }
        pub(super) fn pick(&mut self, window: &Window) -> Option<bool> {
            // SAFETY: key and foreground queries require no borrowed pointers.
            unsafe {
                if GetAsyncKeyState(VK_ESCAPE as i32) < 0 {
                    return Some(false);
                }
                if GetAsyncKeyState(VK_LBUTTON as i32) >= 0 {
                    return None;
                }
                let selected = GetForegroundWindow();
                if selected.is_null() || Some(selected) == handle(window) {
                    return None;
                }
                let mut process = 0;
                GetWindowThreadProcessId(selected, &mut process);
                // Ignore every MothRef window, including the overlay controller.
                if process == std::process::id() {
                    return None;
                }
                self.target = selected as usize;
                self.process = process;
                Some(true)
            }
        }
        fn disable(&mut self) {
            TARGET.store(0, Ordering::Relaxed);
            if self.hook != 0 {
                // SAFETY: this hook was created and is removed on the same event-loop thread.
                unsafe {
                    UnhookWinEvent(self.hook as HWINEVENTHOOK);
                }
                self.hook = 0;
            }
        }
    }
    impl Drop for Pin {
        fn drop(&mut self) {
            self.disable();
        }
    }
}

#[cfg(target_os = "linux")]
#[path = "pin_x11.rs"]
mod x11;
