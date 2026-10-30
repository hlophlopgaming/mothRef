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
    pub pan_time: Option<std::time::Instant>,
    pub pending_focus: Option<egui::Key>,
    pub pointer_action: Option<super::bindings::Action>,
    pub native_pressed: Option<(Kind, u64)>,
    /// A pen is near the screen: touches meanwhile are a resting palm.
    pub pen_near: Option<std::time::Instant>,
    /// Touch contacts rejected at their start, ignored until they end.
    pub rejected: std::collections::HashSet<u64>,
    /// A pen or touch press moved past [`TAP_SLOP`] and became a drag.
    pub engaged: bool,
}

/// A pen tip or finger always wobbles a little on a tap; below this distance in
/// screen points the press is still a click and moves nothing.
pub(super) const TAP_SLOP: f32 = 6.;
/// Zoom factor per screen point of vertical zoom-drag movement (100 points ≈ 2.7×).
pub(super) const ZOOM_DRAG_RATE: f32 = 0.01;
/// A pen that sent nothing for this long is considered gone, even without a
/// proximity-out event from the backend.
const PEN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

impl InputState {
    /// Tracks pen proximity from its events; leaving proximity cancels hover.
    pub fn pen(&mut self, p: &moth_platform::input::Pointer) {
        if p.kind != Kind::Pen {
            return;
        }
        self.pen_near = if p.phase == moth_platform::input::Phase::Cancel {
            None
        } else {
            Some(std::time::Instant::now())
        };
    }
    /// Palm rejection: a touch that starts while a pen is near is dropped for
    /// its whole lifetime, both for the canvas and for the UI.
    pub fn reject_touch(&mut self, id: u64, start: bool, end: bool) -> bool {
        if start
            && self
                .pen_near
                .is_some_and(|seen| seen.elapsed() < PEN_TIMEOUT)
        {
            self.rejected.insert(id);
            // A bounded set: a broken backend cannot grow it indefinitely.
            if self.rejected.len() > 64 {
                self.rejected.clear();
            }
        }
        let rejected = self.rejected.contains(&id);
        if end {
            self.rejected.remove(&id);
        }
        rejected
    }
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

impl super::App {
    pub(super) fn drag_window(&mut self, edge: Option<winit::window::ResizeDirection>) {
        let Some(window) = self.shell.window.clone() else {
            return;
        };
        if edge.is_none() && self.window_position_locked() {
            return;
        }
        let result = match edge {
            Some(edge) => window.drag_resize_window(edge),
            None => window.drag_window(),
        };
        match result {
            Ok(()) => {
                self.cancel_drag();
                self.input.adapter.release_mouse();
                let context = self.shell.context().clone();
                if let Some(state) = &mut self.shell.egui {
                    release_window_pointer(&context, state.egui_input_mut());
                }
            }
            Err(error) => self.workspace.board.notices.message = error.to_string().into(),
        }
    }

    pub(super) fn resize_edge(
        &self,
        position: [f32; 2],
        kind: Kind,
    ) -> Option<winit::window::ResizeDirection> {
        let window = self.shell.window.as_ref()?;
        if !window.is_resizable() || window.is_maximized() || window.fullscreen().is_some() {
            return None;
        }
        let tokens = moth_ui::tokens(self.shell.context());
        let border = if kind == Kind::Mouse {
            tokens.space_s
        } else {
            tokens.space_xl
        };
        let size = window.inner_size();
        let scale = self.shell.context().pixels_per_point();
        window_edge(
            position,
            [size.width as f32 / scale, size.height as f32 / scale],
            f32::from(border),
        )
    }
}

pub(super) fn release_window_pointer(context: &egui::Context, pending: &mut egui::RawInput) {
    // The compositor now owns the gesture and may consume its release event.
    pending
        .events
        .retain(|event| !matches!(event, egui::Event::PointerButton { .. }));
    context.input_mut(|input| input.pointer = Default::default());
    context.stop_dragging();
}

fn window_edge(
    [x, y]: [f32; 2],
    [width, height]: [f32; 2],
    border: f32,
) -> Option<winit::window::ResizeDirection> {
    use winit::window::ResizeDirection::*;
    if x < 0. || y < 0. || x >= width || y >= height {
        return None;
    }
    match (
        x < border,
        x >= width - border,
        y < border,
        y >= height - border,
    ) {
        (true, _, true, _) => Some(NorthWest),
        (_, true, true, _) => Some(NorthEast),
        (true, _, _, true) => Some(SouthWest),
        (_, true, _, true) => Some(SouthEast),
        (true, _, _, _) => Some(West),
        (_, true, _, _) => Some(East),
        (_, _, true, _) => Some(North),
        (_, _, _, true) => Some(South),
        _ => None,
    }
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
        // Physical modifier events duplicate `mods`; they are not held-key gestures.
        let modifier = matches!(
            key,
            egui::Key::ShiftLeft
                | egui::Key::ShiftRight
                | egui::Key::ControlLeft
                | egui::Key::ControlRight
                | egui::Key::AltLeft
                | egui::Key::AltRight
                | egui::Key::SuperLeft
                | egui::Key::SuperRight
        );
        if pressed && !blocked && !modifier {
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
    /// Integrates held navigation keys independently of OS key-repeat timing.
    pub fn flying_pan(
        &mut self,
        registry: &super::bindings::Registry,
        now: std::time::Instant,
        speed: f32,
        blocked: bool,
    ) -> Option<[f32; 2]> {
        if blocked {
            self.held.clear();
        }
        let direction = self
            .held
            .iter()
            .filter_map(|key| registry.key(*key, self.mods()))
            .filter_map(|action| action.pan_direction())
            .fold([0., 0.], |a, b| [a[0] + b[0], a[1] + b[1]]);
        let length = direction[0].hypot(direction[1]);
        if length == 0. {
            self.pan_time = None;
            return None;
        }
        let dt = self.pan_time.replace(now).map_or(0., |last| {
            now.saturating_duration_since(last).as_secs_f32().min(0.05)
        });
        Some([
            direction[0] / length * speed * dt,
            direction[1] / length * speed * dt,
        ])
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
    fn touches_that_start_while_a_pen_is_near_are_rejected_until_they_end() {
        use moth_platform::input::{Phase, Pointer};
        let mut state = InputState::default();
        let pen = |phase| Pointer {
            id: 9,
            kind: Kind::Pen,
            position: [0., 0.],
            pressure: 0.,
            phase,
            pan: false,
            button: 1,
        };
        assert!(!state.reject_touch(1, true, false));
        state.pen(&pen(Phase::Move));
        assert!(state.reject_touch(2, true, false));
        // The palm stays rejected after the pen leaves, until it lifts.
        state.pen(&pen(Phase::Cancel));
        assert!(state.reject_touch(2, false, false));
        assert!(state.reject_touch(2, false, true));
        assert!(!state.reject_touch(2, true, false));
        // A finger already down before the pen arrived keeps working.
        assert!(!state.reject_touch(1, false, true));
    }
    #[test]
    fn native_window_handoff_releases_egui_without_a_button_up() {
        let context = egui::Context::default();
        let press = egui::Event::PointerButton {
            pos: egui::pos2(30., 30.),
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        };
        for _ in 0..3 {
            let mut output = context.run_ui(
                egui::RawInput {
                    events: vec![
                        egui::Event::PointerMoved(egui::pos2(30., 30.)),
                        press.clone(),
                    ],
                    ..Default::default()
                },
                |_| {},
            );
            output.textures_delta.clear();
            assert!(context.input(|input| input.pointer.primary_down()));
            context.set_dragged_id(egui::Id::new("native titlebar"));
            let mut pending = egui::RawInput {
                events: vec![press.clone(), egui::Event::PointerGone],
                ..Default::default()
            };
            release_window_pointer(&context, &mut pending);
            assert!(!context.input(|input| input.pointer.any_down()));
            assert!(context.dragged_id().is_none());
            assert!(matches!(
                pending.events.as_slice(),
                [egui::Event::PointerGone]
            ));
        }
    }

    #[test]
    fn frameless_resize_edges_include_corners_and_leave_the_canvas_alone() {
        use winit::window::ResizeDirection::*;
        for (position, expected) in [
            ([1., 1.], Some(NorthWest)),
            ([799., 1.], Some(NorthEast)),
            ([1., 599.], Some(SouthWest)),
            ([799., 599.], Some(SouthEast)),
            ([400., 1.], Some(North)),
            ([400., 599.], Some(South)),
            ([1., 300.], Some(West)),
            ([799., 300.], Some(East)),
            ([400., 300.], None),
            ([8., 8.], None),
            ([-1., 0.], None),
            ([800., 300.], None),
        ] {
            assert_eq!(window_edge(position, [800., 600.], 8.), expected);
        }
        assert_eq!(window_edge([16., 300.], [800., 600.], 24.), Some(West));
        assert_eq!(window_edge([400., 300.], [0., 0.], 8.), None);
    }

    #[test]
    fn physical_modifiers_do_not_block_selection_or_pan_chords() {
        let registry = Registry::default();
        for (key, modifiers, action) in [
            (
                Key::ControlLeft,
                egui::Modifiers::CTRL | egui::Modifiers::COMMAND,
                Action::ToggleDrag,
            ),
            (
                Key::ControlRight,
                egui::Modifiers::CTRL | egui::Modifiers::COMMAND,
                Action::ToggleDrag,
            ),
            (
                Key::SuperLeft,
                egui::Modifiers::MAC_CMD | egui::Modifiers::COMMAND,
                Action::ToggleDrag,
            ),
            (
                Key::SuperRight,
                egui::Modifiers::MAC_CMD | egui::Modifiers::COMMAND,
                Action::ToggleDrag,
            ),
            (Key::ShiftLeft, egui::Modifiers::SHIFT, Action::ExtendDrag),
            (Key::ShiftRight, egui::Modifiers::SHIFT, Action::ExtendDrag),
            (Key::AltLeft, egui::Modifiers::ALT, Action::DuplicateDrag),
            (Key::AltRight, egui::Modifiers::ALT, Action::DuplicateDrag),
        ] {
            let mut input = InputState {
                modifiers,
                ..Default::default()
            };
            input.key_action(&registry, key, true, false, false, false);
            assert_eq!(
                registry.pointer(1, input.mods(), &input.held),
                Some(action),
                "physical {key:?} must not count as an extra held key"
            );
            input.key_action(&registry, key, false, false, false, false);
            assert!(input.held.is_empty());
        }
        let mut input = InputState {
            modifiers: egui::Modifiers::CTRL | egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            ..Default::default()
        };
        for key in [Key::ControlLeft, Key::ShiftLeft] {
            input.key_action(&registry, key, true, false, false, false);
        }
        assert_eq!(
            registry.pointer(1, input.mods(), &input.held),
            Some(Action::ToggleDrag)
        );
        input.key_action(&registry, Key::A, true, false, false, false);
        assert_eq!(registry.pointer(1, input.mods(), &input.held), None);
    }
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
    fn flying_pan_uses_elapsed_time_and_normalizes_diagonals() {
        use std::time::{Duration, Instant};
        let registry = Registry::default();
        let mut input = InputState::default();
        let now = Instant::now();
        input.key_action(&registry, Key::D, true, false, false, false);
        assert_eq!(
            input.flying_pan(&registry, now, 800., false),
            Some([0., 0.])
        );
        let delta = input
            .flying_pan(&registry, now + Duration::from_millis(25), 800., false)
            .unwrap();
        assert!((delta[0] - 20.).abs() < 0.001);
        assert_eq!(delta[1], 0.);
        input.key_action(&registry, Key::W, true, false, false, false);
        let delta = input
            .flying_pan(&registry, now + Duration::from_millis(50), 800., false)
            .unwrap();
        assert!((delta[0].hypot(delta[1]) - 20.).abs() < 0.001);
        assert!(delta[0] > 0. && delta[1] < 0.);
        input.key_action(&registry, Key::D, false, false, false, false);
        input.key_action(&registry, Key::W, false, false, false, false);
        assert_eq!(
            input.flying_pan(&registry, now + Duration::from_millis(75), 800., false),
            None
        );
        assert!(input.pan_time.is_none());
    }
    #[test]
    fn flying_pan_respects_rebinding_focus_and_stalled_frames() {
        use std::time::{Duration, Instant};
        let mut registry = Registry::default();
        registry.set(Action::PanRight, "8");
        let mut input = InputState::default();
        let now = Instant::now();
        input.key_action(&registry, Key::D, true, false, false, false);
        assert_eq!(input.flying_pan(&registry, now, 800., false), None);
        input.key_action(&registry, Key::Num8, true, false, false, false);
        assert!(input.flying_pan(&registry, now, 800., false).is_some());
        let delta = input
            .flying_pan(&registry, now + Duration::from_secs(10), 800., false)
            .unwrap();
        assert!((delta[0] - 40.).abs() < 0.001);
        assert_eq!(
            input.flying_pan(&registry, now + Duration::from_secs(11), 800., true),
            None
        );
        assert!(input.held.is_empty());
        assert_eq!(
            input.flying_pan(&registry, now + Duration::from_secs(12), 800., false),
            None
        );
    }
    #[test]
    fn canvas_pan_repeats_rebinds_and_stops_at_text_fields() {
        let mut registry = Registry::default();
        let mut input = InputState::default();
        for (key, action) in [
            (Key::W, Action::PanUp),
            (Key::A, Action::PanLeft),
            (Key::S, Action::PanDown),
            (Key::D, Action::PanRight),
        ] {
            assert_eq!(
                input.key_action(&registry, key, true, false, false, false),
                Some(action)
            );
            assert_eq!(
                input.key_action(&registry, key, true, true, false, false),
                Some(action)
            );
            assert_eq!(
                input.key_action(&registry, key, true, true, true, false),
                None
            );
            assert!(!input.held.contains(&key));
        }
        registry.set(Action::PanUp, "Alt+W");
        assert_eq!(registry.key(Key::W, Default::default()), None);
        let mut restored = Registry::default();
        assert!(restored.load(&registry.preferences()));
        assert_eq!(restored, registry);
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
