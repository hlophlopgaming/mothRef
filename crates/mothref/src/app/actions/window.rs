// SPDX-License-Identifier: GPL-3.0-or-later
//! The main window: View bar, Over mode, pin, click-through, size, opacity
//! and placement.
use super::*;
impl App {
    /// Whether an action of this area is available once its [`Gate`] is passed.
    pub(super) fn window_enabled(&self, action: Action) -> bool {
        use Action::*;
        match action {
            WindowBar => self.compact(),
            ClickThrough => {
                self.window.overlay.is_some()
                    && self.shell.window.as_ref().is_some_and(|w| {
                        moth_platform::Desktop.window_capabilities(w).click_through
                    })
            }
            MoveWindow => !self.window_position_locked(),
            LockWindow => self
                .shell
                .window
                .as_ref()
                .is_some_and(|w| w.outer_position().is_ok()),
            OverlaySelection => {
                !self.workspace.library
                    && !self.workspace.board.intake.loading
                    && self.workspace.board.selection.iter().any(|id| {
                        self.workspace
                            .board
                            .scene
                            .get(*id)
                            .is_some_and(|item| item.blob().is_some())
                    })
            }
            ResizeSelection | ResizeAll => {
                !self.workspace.library
                    && !self.workspace.board.intake.loading
                    && !self.workspace.board.scene.is_empty()
                    && (action == ResizeAll || !self.workspace.board.selection.is_empty())
                    && self
                        .shell
                        .window
                        .as_ref()
                        .is_some_and(|w| w.fullscreen().is_none() && !w.is_maximized())
            }
            OpacityUp | OpacityDown => !self.workspace.library,
            Pin | Bottom => {
                self.window.overlay.is_none()
                    && self.shell.window.as_ref().is_some_and(|w| {
                        let caps = moth_platform::Desktop.window_capabilities(w);
                        if action == Pin {
                            caps.on_top || caps.above_window
                        } else {
                            caps.bottom
                        }
                    })
            }
            _ => true,
        }
    }
    /// Runs an action of this area; `None` when it finished without an intent.
    pub(super) fn run_window(
        &mut self,
        action: Action,
        event_loop: &ActiveEventLoop,
    ) -> Option<Intent> {
        use Action::*;
        let intent = match action {
            WindowBar => {
                self.window.window_bar = !self.window.window_bar;
                return None;
            }
            Overlay => Intent::Overlay,
            Pin => Intent::Pin,
            ClickThrough => {
                if let Some(overlay) = &mut self.window.overlay {
                    overlay.click_through = !overlay.click_through;
                    overlay.window.request_redraw();
                }
                return None;
            }
            OverlaySelection => {
                self.float_selection(event_loop);
                return None;
            }
            Bottom => {
                let enabled = !self.window.bottom;
                self.apply_pin(false);
                self.window.bottom = enabled;
                if let Some(window) = &self.shell.window {
                    moth_platform::Desktop.window_bottom(window, enabled);
                }
                return None;
            }
            LockWindow => {
                self.window.locked_position = if self.window.locked_position.is_some() {
                    None
                } else {
                    self.shell
                        .window
                        .as_ref()
                        .and_then(|w| w.outer_position().ok())
                };
                self.cancel_window_mode();
                return None;
            }
            ResizeSelection | ResizeAll => {
                self.resize_to_items(action == ResizeSelection);
                return None;
            }
            OpacityUp | OpacityDown => {
                let appearance = &mut self.workspace.board.appearance;
                appearance.window =
                    window_mode::adjust_opacity(appearance.window, action == OpacityUp);
                if let Some(overlay) = &mut self.window.overlay {
                    overlay.appearance = *appearance;
                }
                self.remember_appearance();
                return None;
            }
            MoveWindow => {
                self.cancel_drag();
                let moving = !self.window.moving_window;
                self.cancel_window_mode();
                self.window.moving_window = moving;
                return None;
            }
            Fullscreen => Intent::Fullscreen,
            Minimize => Intent::Minimize,
            Maximize => Intent::Maximize,
            _ => return None,
        };
        Some(intent)
    }
}
