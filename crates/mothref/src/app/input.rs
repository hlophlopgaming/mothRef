// SPDX-License-Identifier: GPL-3.0-or-later
//! Groups pointer adaptation, gesture recognition and current native input state.
use moth_platform::input::{Adapter, Gestures, Kind};

#[derive(Default)]
pub(super) struct InputState {
    pub adapter: Adapter,
    pub gestures: Gestures,
    pub cursor: Option<[f32; 2]>,
    pub hover: bool,
    pub modifiers: egui::Modifiers,
    pub super_key: bool,
    pub held: std::collections::HashSet<egui::Key>,
    pub pending_focus: Option<egui::Key>,
    pub pointer_action: Option<super::bindings::Action>,
    pub native_pressed: Option<(Kind, u64)>,
}

/// Physical chords remain usable with non-Latin layouts, as existing MothRef shortcuts did.
pub(super) fn key(event: &winit::event::KeyEvent) -> Option<egui::Key> {
    let winit::keyboard::PhysicalKey::Code(code) = event.physical_key else {
        return None;
    };
    let name = format!("{code:?}");
    egui::Key::from_name(
        name.strip_prefix("Key")
            .or_else(|| name.strip_prefix("Digit"))
            .unwrap_or(&name),
    )
}

/// Wheel and trackpad navigation share the same canvas ownership check.
pub(super) fn canvas_navigation(
    viewport: egui::Rect,
    tools_open: bool,
    position: [f32; 2],
) -> bool {
    !tools_open && viewport.contains(egui::pos2(position[0], position[1]))
}

impl InputState {
    /// Match native pointer coordinates to the points used by the canvas and egui.
    pub fn event(
        &mut self,
        event: &winit::event::WindowEvent,
        native_scale: f64,
        context: &egui::Context,
    ) -> Option<moth_platform::input::Input> {
        self.adapter
            .event(event, native_scale * f64::from(context.zoom_factor()))
    }

    /// egui intentionally omits Super outside macOS; exact matching still needs it.
    pub fn modifiers_event(&mut self, event: &winit::event::WindowEvent) {
        if let winit::event::WindowEvent::ModifiersChanged(modifiers) = event {
            self.super_key = modifiers.state().super_key()
                && egui::os::OperatingSystem::default() != egui::os::OperatingSystem::Mac;
        }
    }
    pub fn mods(&self) -> super::bindings::Mods {
        super::bindings::Mods {
            super_key: self.super_key,
            ..super::bindings::Mods::from_egui(self.modifiers)
        }
    }

    /// Tracks releases even in text fields, but never dispatches actions through their focus.
    pub fn key_action(
        &mut self,
        registry: &super::bindings::Registry,
        key: egui::Key,
        pressed: bool,
        repeat: bool,
        blocked: bool,
        dragging: bool,
    ) -> Option<super::bindings::Action> {
        use super::bindings::Action;
        if pressed && !blocked {
            self.held.insert(key);
        } else {
            self.held.remove(&key);
        }
        if key == egui::Key::Escape || blocked {
            self.pending_focus = None;
            return None;
        }
        let action = registry.key(key, self.mods());
        if !pressed {
            let focus = self
                .pending_focus
                .take_if(|pending| *pending == key)
                .is_some();
            return (focus && action == Some(Action::Focus)).then_some(Action::Focus);
        }
        if dragging {
            return None;
        }
        let action = action.filter(|a| !repeat || a.repeat())?;
        if action == Action::Focus {
            self.pending_focus = Some(key);
        }
        Some(action)
    }
    /// The threshold is in screen points, independent of zoom or input device.
    pub fn focus_drag(&mut self, start: [f32; 2], position: [f32; 2]) -> bool {
        if self.pending_focus.is_some() {
            if (position[0] - start[0]).hypot(position[1] - start[1]) < 4. {
                return false;
            }
            self.pending_focus = None;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::super::bindings::{Action, Registry};
    use super::*;
    use egui::Key;
    #[test]
    fn pointer_matches_canvas_points_at_custom_ui_scale() {
        for native_scale in [1., 2.] {
            for zoom in [0.75, 1., 1.5, 2.] {
                let context = egui::Context::default();
                context.set_zoom_factor(zoom);
                context
                    .run_ui(Default::default(), |_| {})
                    .textures_delta
                    .clear();
                let mut input = InputState::default();
                let event = winit::event::WindowEvent::CursorMoved {
                    device_id: winit::event::DeviceId::dummy(),
                    position: winit::dpi::PhysicalPosition::new(
                        300. * native_scale * f64::from(zoom),
                        200. * native_scale * f64::from(zoom),
                    ),
                };
                let Some(moth_platform::input::Input::Pointer(pointer)) =
                    input.event(&event, native_scale, &context)
                else {
                    panic!("cursor movement must produce a pointer");
                };
                assert_eq!(pointer.position, [300., 200.]);
            }
        }
    }
    #[test]
    fn open_tools_keep_wheel_and_trackpad_navigation_off_the_board() {
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 600.));
        for event in [
            moth_platform::input::Input::Zoom {
                position: [300., 200.],
                factor: 1.25,
                continuous: false,
            },
            moth_platform::input::Input::Zoom {
                position: [300., 200.],
                factor: 1.01,
                continuous: true,
            },
            moth_platform::input::Input::Pan {
                position: [300., 200.],
                delta: [0., 20.],
                continuous: true,
                finished: false,
            },
        ] {
            let position = match event {
                moth_platform::input::Input::Zoom { position, .. }
                | moth_platform::input::Input::Pan { position, .. } => position,
                _ => unreachable!(),
            };
            assert!(canvas_navigation(viewport, false, position));
            assert!(!canvas_navigation(viewport, true, position));
        }
    }
    #[test]
    fn space_tap_focuses_but_a_drag_or_cancel_does_not() {
        let registry = Registry::default();
        let mut input = InputState::default();
        assert_eq!(
            input.key_action(&registry, Key::Space, true, false, false, false),
            Some(Action::Focus)
        );
        assert!(!input.focus_drag([0.; 2], [2., 0.]));
        assert_eq!(
            input.key_action(&registry, Key::Space, false, false, false, false),
            Some(Action::Focus)
        );
        input.key_action(&registry, Key::Space, true, false, false, false);
        assert!(input.focus_drag([0.; 2], [5., 0.]));
        assert_eq!(
            input.key_action(&registry, Key::Space, false, false, false, false),
            None
        );
        input.key_action(&registry, Key::Space, true, false, false, false);
        input.key_action(&registry, Key::Escape, true, false, false, true);
        assert_eq!(
            input.key_action(&registry, Key::Space, false, false, false, false),
            None
        );
    }
    #[test]
    fn space_while_typing_cannot_arm_pan_or_fit() {
        let registry = Registry::default();
        let mut input = InputState::default();
        assert_eq!(
            input.key_action(&registry, Key::Space, true, false, true, false),
            None
        );
        assert!(!input.held.contains(&Key::Space));
        assert_eq!(
            input.key_action(&registry, Key::Space, false, false, false, false),
            None
        );
    }
    #[test]
    fn text_modals_and_repeats_never_leak_destructive_actions() {
        let registry = Registry::default();
        let mut input = InputState::default();
        input.modifiers.command = true;
        input.modifiers.ctrl = true;
        assert_eq!(
            input.key_action(&registry, Key::N, true, false, true, false),
            None
        );
        assert_eq!(
            input.key_action(&registry, Key::N, true, true, false, false),
            None
        );
        assert_eq!(
            input.key_action(&registry, Key::N, true, false, false, true),
            None
        );
        assert_eq!(
            input.key_action(&registry, Key::N, true, false, false, false),
            Some(Action::New)
        );
        assert_eq!(
            input.key_action(&registry, Key::N, false, false, true, false),
            None
        );
        assert!(!input.held.contains(&Key::N));
        input.super_key = true;
        assert_eq!(
            input.key_action(&registry, Key::N, true, false, false, false),
            None
        );
    }
}
