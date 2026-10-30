// SPDX-License-Identifier: GPL-3.0-or-later
//! The window: modes, pin, size and moving tabs between windows.
use super::*;
impl App {
    /// Applies an intent of this area. False when the camera must not animate
    /// to where the intent left it.
    pub(in crate::app) fn apply_window(
        &mut self,
        intent: Intent,
        event_loop: &ActiveEventLoop,
    ) -> bool {
        match intent {
            Intent::Overlay => self.toggle_overlay(event_loop),
            Intent::Appearance(value) => {
                self.workspace.board.appearance = value;
                self.remember_appearance();
            }
            Intent::PinChoice(target) => {
                if self.shell.window.as_ref().is_some_and(|w| {
                    if target {
                        moth_platform::Desktop.window_capabilities(w).above_window
                    } else {
                        moth_platform::Desktop.window_capabilities(w).on_top
                    }
                }) {
                    if target {
                        self.pick_window();
                    } else {
                        self.window.picking_window = false;
                        self.window.target_pin = false;
                        self.apply_pin(true);
                    }
                }
            }
            Intent::CancelWindowMode => self.cancel_window_mode(),
            Intent::Pin => {
                if self.window.picking_window {
                    self.cancel_window_mode();
                    return false;
                }
                if self.window.target_pin
                    && !self.window.pinned
                    && !moth_platform::Desktop.pin_target_available(&self.window.pin)
                {
                    self.pick_window();
                } else {
                    self.apply_pin(!self.window.pinned);
                }
            }
            Intent::Fullscreen => {
                if let Some(window) = &self.shell.window {
                    window.set_fullscreen(if window.fullscreen().is_some() {
                        None
                    } else {
                        Some(winit::window::Fullscreen::Borderless(None))
                    });
                }
            }
            Intent::CloseWindow => self.request_close(true),
            Intent::Drag => self.drag_window(None),
            Intent::WindowBar(shown) => self.window.window_bar = shown,
            // Switching the current window mid-frame would render this frame into
            // another window; these run after the frame in `about_to_wait`.
            Intent::DetachTab(slot, at) => self.queue_window_op(panes::WindowOp::Detach(slot, at)),
            Intent::MoveTab(slot, index) => {
                if let Some(target) = self.pane_window(index) {
                    self.queue_window_op(panes::WindowOp::Move(slot, target));
                }
            }
            Intent::GatherTabs => self.queue_window_op(panes::WindowOp::Gather),
            Intent::Minimize => {
                if let Some(window) = &self.shell.window {
                    window.set_minimized(true);
                }
            }
            Intent::Maximize => {
                if let Some(window) = &self.shell.window {
                    if window.fullscreen().is_some() {
                        window.set_fullscreen(None);
                    } else {
                        window.set_maximized(!window.is_maximized());
                    }
                }
            }
            _ => {}
        }
        true
    }
}
