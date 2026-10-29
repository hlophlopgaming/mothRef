// SPDX-License-Identifier: GPL-3.0-or-later
//! winit turns Windows pen input into touch events: hover, the barrel button and
//! the eraser are lost, and a pen drag pans like a finger. A window subclass sees
//! pen `WM_POINTER*` messages before winit and reports them as native pen
//! pointers, the same way the Wayland tablet adapter does. Mouse and touch
//! messages pass through to winit unchanged.
use super::{Kind, NativeEvent, Phase, Pointer};
use crossbeam_channel::{Receiver, Sender};
use std::cell::RefCell;
use std::sync::Arc;
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM},
    Graphics::Gdi::ScreenToClient,
    UI::{
        HiDpi::GetDpiForWindow,
        Input::Pointer::{
            GetPointerPenInfo, GetPointerType, POINTER_FLAG_INCONTACT, POINTER_FLAG_INRANGE,
            POINTER_PEN_INFO,
        },
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::{
            PEN_FLAG_BARREL, PT_PEN, WM_POINTERCAPTURECHANGED, WM_POINTERDOWN, WM_POINTERENTER,
            WM_POINTERLEAVE, WM_POINTERUP, WM_POINTERUPDATE,
        },
    },
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

const SUBCLASS: usize = 0x4d6f_7468; // "Moth"

/// Owns the subclass; dropping it restores the window procedure.
pub(super) struct Native {
    pub events: Receiver<NativeEvent>,
    hwnd: HWND,
    /// Referenced by the subclass; kept alive until `Drop` removes it.
    _state: Box<RefCell<State>>,
}

struct State {
    send: Sender<NativeEvent>,
    wake: Arc<dyn Fn() + Send + Sync>,
    pen: Pen,
}

/// Turns pen samples into pointer phases. The first of tip contact and barrel
/// button starts a press (button 1 or 2); the other is ignored until it ends.
#[derive(Default)]
struct Pen {
    tip: bool,
    barrel: bool,
    press: Option<u8>,
}
impl Pen {
    fn sample(&mut self, in_range: bool, tip: bool, barrel: bool) -> Vec<(Phase, u8)> {
        let mut phases = Vec::new();
        if !in_range {
            phases.push((Phase::Cancel, self.press.unwrap_or(1)));
            *self = Self::default();
            return phases;
        }
        for (held, now, button) in [(self.tip, tip, 1), (self.barrel, barrel, 2)] {
            if held && !now && self.press == Some(button) {
                self.press = None;
                phases.push((Phase::Up, button));
            }
        }
        for (held, now, button) in [(self.tip, tip, 1), (self.barrel, barrel, 2)] {
            if !held && now && self.press.is_none() {
                self.press = Some(button);
                phases.push((Phase::Down, button));
            }
        }
        self.tip = tip;
        self.barrel = barrel;
        if phases.is_empty() {
            phases.push((Phase::Move, self.press.unwrap_or(1)));
        }
        phases
    }
}

impl Native {
    pub(super) fn new(
        window: &winit::window::Window,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> std::io::Result<Option<Self>> {
        let Ok(handle) = window.window_handle() else {
            return Ok(None);
        };
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return Ok(None);
        };
        let hwnd = handle.hwnd.get() as HWND;
        let (send, events) = crossbeam_channel::bounded(256);
        let state = Box::new(RefCell::new(State {
            send,
            wake,
            pen: Pen::default(),
        }));
        // SAFETY: `hwnd` is our live window on this (event-loop) thread; the boxed
        // state outlives the subclass, which `Drop` removes before freeing it.
        let installed = unsafe {
            SetWindowSubclass(
                hwnd,
                Some(subclass),
                SUBCLASS,
                &*state as *const RefCell<State> as usize,
            )
        };
        if installed == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Some(Self {
            events,
            hwnd,
            _state: state,
        }))
    }
}
impl Drop for Native {
    fn drop(&mut self) {
        // SAFETY: removes the subclass installed in `new` on the same window.
        unsafe {
            RemoveWindowSubclass(self.hwnd, Some(subclass), SUBCLASS);
        }
    }
}

// SAFETY: called by Windows on the window's thread with the reference data set in
// `Native::new`; no panic crosses this boundary (all paths are fallible calls).
unsafe extern "system" fn subclass(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    data: usize,
) -> LRESULT {
    let pointer_message = matches!(
        message,
        WM_POINTERUPDATE
            | WM_POINTERDOWN
            | WM_POINTERUP
            | WM_POINTERENTER
            | WM_POINTERLEAVE
            | WM_POINTERCAPTURECHANGED
    );
    if pointer_message && data != 0 {
        let id = (wparam & 0xffff) as u32;
        let mut kind = 0;
        // SAFETY: the out pointer is valid; an unknown id returns false.
        let pen = unsafe { GetPointerType(id, &mut kind) } != 0 && kind == PT_PEN;
        if pen {
            // SAFETY: `data` is the `RefCell<State>` owned by `Native`, alive while
            // the subclass is installed, and only touched on this thread.
            let state = unsafe { &*(data as *const RefCell<State>) };
            if let Ok(mut state) = state.try_borrow_mut() {
                state.pointer(hwnd, id, message);
            }
            // Handled: winit would otherwise report the pen as a finger.
            return 0;
        }
    }
    // SAFETY: forwards everything else to the next procedure (winit's).
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

impl State {
    fn pointer(&mut self, hwnd: HWND, id: u32, message: u32) {
        let mut info = POINTER_PEN_INFO::default();
        // SAFETY: the out pointer is valid for one POINTER_PEN_INFO.
        if unsafe { GetPointerPenInfo(id, &mut info) } == 0 {
            return;
        }
        let flags = info.pointerInfo.pointerFlags;
        let in_range = flags & POINTER_FLAG_INRANGE != 0
            && !matches!(message, WM_POINTERLEAVE | WM_POINTERCAPTURECHANGED);
        let tip = flags & POINTER_FLAG_INCONTACT != 0;
        let barrel = info.penFlags & PEN_FLAG_BARREL != 0;
        let mut point = POINT {
            x: info.pointerInfo.ptPixelLocation.x,
            y: info.pointerInfo.ptPixelLocation.y,
        };
        // SAFETY: valid window and out pointer; failure leaves screen coordinates.
        unsafe {
            ScreenToClient(hwnd, &mut point);
        }
        // SAFETY: plain query on our window; zero means unknown.
        let scale = match unsafe { GetDpiForWindow(hwnd) } {
            0 => 1.,
            dpi => dpi as f32 / 96.,
        };
        let position = [point.x as f32 / scale, point.y as f32 / scale];
        let pressure = if tip {
            (info.pressure.min(1024) as f32 / 1024.).max(f32::MIN_POSITIVE)
        } else {
            0.
        };
        let mut sent = false;
        for (phase, button) in self.pen.sample(in_range, tip, barrel) {
            let pointer = Pointer {
                id: u64::from(id),
                kind: Kind::Pen,
                position,
                pressure: if matches!(phase, Phase::Up | Phase::Cancel) {
                    0.
                } else {
                    pressure
                },
                phase,
                pan: false,
                button,
            };
            sent |= self.send.try_send(NativeEvent::Pointer(pointer)).is_ok();
        }
        if sent {
            (self.wake)();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hover_barrel_and_tip_become_one_press_at_a_time() {
        let mut pen = Pen::default();
        assert_eq!(pen.sample(true, false, false), [(Phase::Move, 1)]);
        // Barrel while hovering opens the press; the tip landing meanwhile is ignored.
        assert_eq!(pen.sample(true, false, true), [(Phase::Down, 2)]);
        assert_eq!(pen.sample(true, true, true), [(Phase::Move, 2)]);
        assert_eq!(pen.sample(true, true, false), [(Phase::Up, 2)]);
        assert_eq!(pen.sample(true, false, false), [(Phase::Move, 1)]);
        assert_eq!(pen.sample(true, true, false), [(Phase::Down, 1)]);
        // Leaving range mid-press cancels it.
        assert_eq!(pen.sample(false, true, false), [(Phase::Cancel, 1)]);
        assert_eq!(pen.sample(true, false, false), [(Phase::Move, 1)]);
    }
}
